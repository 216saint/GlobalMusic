//! Conta do Spotify conectada **pelo site** (como YouTube Music e SoundCloud): a pessoa entra em
//! accounts.spotify.com numa janela do app, e o cookie de sessão fica no perfil da WebView.
//!
//! **Por quê.** Pela Web API "de app" (OAuth + client ID), contas Free não têm saída: o client ID
//! próprio exige Premium desde fev/2026 e o compartilhado vive estourando a cota (429). O player web
//! do Spotify usa um token do próprio site, com cota própria. Então abrimos o player numa janela
//! **invisível**, deixamos o site gerar o token como sempre faz e só lemos o resultado: um script
//! injetado observa a resposta de `/api/token` e devolve o token pelo título da janela (o único
//! canal de uma página externa para o app, sem dar a ela acesso aos comandos do Tauri).
//!
//! **Atenção:** é uma API interna do Spotify, fora dos termos de uso. Serve ao teste com colegas e
//! pode quebrar quando o site mudar; nesse caso a conta continua funcionando pelo OAuth (Premium).

use std::sync::Mutex as StdMutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::{oneshot, Mutex};

use crate::auth::web_login::{self, WebLogin};
use crate::error::{Error, Result};
use crate::models::Source;

pub const LOGIN: WebLogin = WebLogin {
    label: "spotify-login",
    title: "Entrar no Spotify",
    start_url: "https://accounts.spotify.com/login?continue=https%3A%2F%2Fopen.spotify.com%2F",
    cookie_url: "https://open.spotify.com",
    session_cookie: "sp_dc",
    logout_urls: &["https://open.spotify.com", "https://accounts.spotify.com", "https://www.spotify.com"],
};

const PLAYER_URL: &str = "https://open.spotify.com/";
const WINDOW_LABEL: &str = "spotify-session";
/// O player costuma pedir o token no primeiro segundo; a rede lenta ganha folga.
const TOKEN_TIMEOUT: Duration = Duration::from_secs(25);
/// Renova um pouco antes de vencer (o token do site dura ~1 h).
const REFRESH_MARGIN_MS: u64 = 2 * 60 * 1000;
/// Quando o token veio só do header de uma requisição (sem validade conhecida).
const ASSUMED_LIFETIME_MS: u64 = 30 * 60 * 1000;
/// Prefixo do título que carrega o token (ver o script).
const TITLE_PREFIX: &str = "GMTOKEN ";

/// Roda no player antes dos scripts do site. Observa o `fetch` e o `XMLHttpRequest`: a resposta de
/// `/api/token` (token + validade) e, como reserva, o header `Authorization` das chamadas à API.
const SCRIPT: &str = r#"
(() => {
  if (location.hostname !== "open.spotify.com" || window.__gmToken) return;
  window.__gmToken = true;
  const report = (token, exp, anonymous) => {
    if (!token || anonymous) return;
    const previous = document.title;
    document.title = "GMTOKEN " + JSON.stringify({ token, exp: exp || 0 });
    setTimeout(() => { if (document.title.startsWith("GMTOKEN")) document.title = previous; }, 100);
  };
  const fromBody = (body) => body && report(body.accessToken, body.accessTokenExpirationTimestampMs, body.isAnonymous);
  const fromHeader = (value) => {
    const m = /^Bearer\s+(.+)$/i.exec(value || "");
    if (m) report(m[1], 0, false);
  };
  const isToken = (url) => /\/api\/token\b|\/get_access_token\b/.test(String(url || ""));
  const originalFetch = window.fetch;
  window.fetch = async function (input, init) {
    try {
      const headers = new Headers((init && init.headers) || (input instanceof Request ? input.headers : undefined));
      fromHeader(headers.get("authorization"));
    } catch {}
    const response = await originalFetch.apply(this, arguments);
    try {
      const url = typeof input === "string" ? input : input && input.url;
      if (isToken(url)) response.clone().json().then(fromBody, () => {});
    } catch {}
    return response;
  };
  const open = XMLHttpRequest.prototype.open;
  XMLHttpRequest.prototype.open = function (method, url) {
    this.__gmUrl = url;
    return open.apply(this, arguments);
  };
  const setHeader = XMLHttpRequest.prototype.setRequestHeader;
  XMLHttpRequest.prototype.setRequestHeader = function (name, value) {
    if (/^authorization$/i.test(name)) fromHeader(value);
    return setHeader.apply(this, arguments);
  };
  const send = XMLHttpRequest.prototype.send;
  XMLHttpRequest.prototype.send = function () {
    if (isToken(this.__gmUrl)) {
      this.addEventListener("load", () => { try { fromBody(JSON.parse(this.responseText)); } catch {} });
    }
    return send.apply(this, arguments);
  };
})();
"#;

#[derive(Deserialize)]
struct Reported {
    token: String,
    /// Unix em ms; 0 = desconhecida.
    exp: u64,
}

pub struct WebSession {
    app: AppHandle,
    /// Token atual e até quando vale (unix ms).
    token: StdMutex<Option<(String, u64)>>,
    /// Só uma janela invisível por vez, mesmo com várias chamadas pedindo token juntas.
    fetching: Mutex<()>,
}

impl WebSession {
    pub fn new(app: AppHandle) -> Self {
        Self { app, token: StdMutex::new(None), fetching: Mutex::new(()) }
    }

    /// Há alguém logado no site (cookie `sp_dc`)?
    pub fn connected(&self) -> bool {
        matches!(web_login::session_cookie(&self.app, &LOGIN), Ok(Some(_)))
    }

    /// Abre a janela de login do site.
    pub async fn login(&self) -> Result<()> {
        self.forget();
        web_login::login(&self.app, &LOGIN).await
    }

    pub fn logout(&self) -> Result<()> {
        self.forget();
        web_login::logout(&self.app, &LOGIN)
    }

    /// Descarta o token guardado (ex.: o Spotify recusou com 401).
    pub fn forget(&self) {
        if let Ok(mut token) = self.token.lock() {
            *token = None;
        }
    }

    fn cached(&self) -> Option<String> {
        let token = self.token.lock().ok()?;
        let (value, until) = token.as_ref()?;
        (*until > now_ms() + REFRESH_MARGIN_MS).then(|| value.clone())
    }

    /// Token do site, renovado pela janela invisível quando perto de vencer.
    pub async fn token(&self) -> Result<String> {
        if let Some(token) = self.cached() {
            return Ok(token);
        }
        if !self.connected() {
            return Err(Error::NotConnected(Source::Spotify));
        }
        let _guard = self.fetching.lock().await;
        // Outra chamada pode ter renovado enquanto esperávamos a vez.
        if let Some(token) = self.cached() {
            return Ok(token);
        }
        let reported = self.read_from_player().await?;
        let until = if reported.exp > now_ms() { reported.exp } else { now_ms() + ASSUMED_LIFETIME_MS };
        if let Ok(mut token) = self.token.lock() {
            *token = Some((reported.token.clone(), until));
        }
        log::info!("Spotify: token do site renovado");
        Ok(reported.token)
    }

    /// Abre o player numa janela invisível e espera o script devolver o token.
    async fn read_from_player(&self) -> Result<Reported> {
        if let Some(stale) = self.app.get_webview_window(WINDOW_LABEL) {
            let _ = stale.close();
        }
        let (sender, receiver) = oneshot::channel::<Reported>();
        let sender = StdMutex::new(Some(sender));
        let url = Url::parse(PLAYER_URL).map_err(|e| Error::Other(e.to_string()))?;
        let builder = WebviewWindowBuilder::new(&self.app, WINDOW_LABEL, WebviewUrl::External(url))
            .title("Spotify")
            .visible(false)
            .skip_taskbar(true)
            .initialization_script(SCRIPT)
            .on_document_title_changed(move |_, title| {
                let Some(json) = title.strip_prefix(TITLE_PREFIX) else { return };
                let Ok(reported) = serde_json::from_str::<Reported>(json) else { return };
                if let Some(sender) = sender.lock().ok().and_then(|mut s| s.take()) {
                    let _ = sender.send(reported);
                }
            });
        // Mesmas flags da janela principal (ver `window::WEBVIEW2_ARGS`).
        #[cfg(target_os = "windows")]
        let builder = builder.additional_browser_args(crate::window::WEBVIEW2_ARGS);
        let window = builder.build().map_err(|e| Error::Other(format!("não abri o player do Spotify: {e}")))?;

        let result = tokio::time::timeout(TOKEN_TIMEOUT, receiver).await;
        let _ = window.close();
        match result {
            Ok(Ok(reported)) => Ok(reported),
            _ => Err(Error::Other("o site do Spotify não respondeu; tente de novo em instantes".into())),
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_what_the_script_reports() {
        let title = r#"GMTOKEN {"token":"BQabc","exp":1800000000000}"#;
        let reported: Reported = serde_json::from_str(title.strip_prefix(TITLE_PREFIX).unwrap()).unwrap();
        assert_eq!(reported.token, "BQabc");
        assert_eq!(reported.exp, 1_800_000_000_000);
        assert!(SCRIPT.contains(TITLE_PREFIX), "o script e o Rust usam o mesmo prefixo");
    }
}

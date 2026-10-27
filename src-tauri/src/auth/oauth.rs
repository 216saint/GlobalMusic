//! Fluxo OAuth para apps desktop (RFC 8252): navegador do sistema + redirect
//! para um servidor de loopback em `127.0.0.1`, com PKCE (RFC 7636).

use std::sync::Mutex;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngCore;
use sha2::{Digest, Sha256};
use tauri::AppHandle;
use tauri_plugin_oauth::OauthConfig;
use tauri_plugin_opener::OpenerExt;

use crate::error::{Error, Result};

const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);

/// Página que o navegador mostra ao voltar para o app. Neutra de propósito: o resultado
/// (conectou ou o motivo do erro) aparece no próprio GlobalMusic.
const SUCCESS_PAGE: &str = "<!doctype html><meta charset=utf-8><title>GlobalMusic</title>\
<body style=\"font-family:system-ui;background:#111;color:#eee;display:grid;place-items:center;height:100vh\">\
<p>Pronto. Pode fechar esta aba e voltar ao GlobalMusic.</p></body>";

#[derive(Debug, Clone)]
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
    /// Valor anti-CSRF que precisa voltar intacto no redirect.
    pub state: String,
}

impl Pkce {
    pub fn generate() -> Self {
        let verifier = random_token(64);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        Self { verifier, challenge, state: random_token(24) }
    }
}

fn random_token(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::rng().fill_bytes(&mut buf);
    URL_SAFE_NO_PAD.encode(buf)
}

/// Abre `authorize_url` no navegador padrão e espera o redirect no loopback.
/// Devolve o `code` de autorização, pronto para ser trocado por tokens.
pub async fn authorize_via_browser(
    app: &AppHandle,
    authorize_url: &str,
    port: u16,
    expected_state: &str,
) -> Result<String> {
    let (tx, rx) = tokio::sync::oneshot::channel::<String>();
    let tx = Mutex::new(Some(tx));
    let config = OauthConfig { ports: Some(vec![port]), response: Some(SUCCESS_PAGE.into()), ..Default::default() };

    let port = tauri_plugin_oauth::start_with_config(config, move |url| {
        if let Some(tx) = tx.lock().ok().and_then(|mut guard| guard.take()) {
            let _ = tx.send(url);
        }
    })
    .map_err(|e| Error::OAuth(format!("não foi possível abrir a porta {port}: {e}")))?;

    if let Err(e) = app.opener().open_url(authorize_url, None::<&str>) {
        let _ = tauri_plugin_oauth::cancel(port);
        return Err(Error::Other(format!("não foi possível abrir o navegador: {e}")));
    }

    let received = tokio::time::timeout(LOGIN_TIMEOUT, rx).await;
    let _ = tauri_plugin_oauth::cancel(port);

    let url = received
        .map_err(|_| Error::Timeout)?
        .map_err(|_| Error::OAuth("servidor de loopback encerrado".into()))?;
    parse_callback(&url, expected_state)
}

/// Extrai o `code` da URL de redirect validando o `state`.
pub fn parse_callback(url: &str, expected_state: &str) -> Result<String> {
    let url = url::Url::parse(url).map_err(|e| Error::OAuth(format!("URL de retorno inválida: {e}")))?;
    let mut code = None;
    let mut state = None;
    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "code" => code = Some(value.into_owned()),
            "state" => state = Some(value.into_owned()),
            "error" => return Err(Error::OAuth(value.into_owned())),
            _ => {}
        }
    }
    if state.as_deref() != Some(expected_state) {
        return Err(Error::OAuth("state não confere".into()));
    }
    code.ok_or_else(|| Error::OAuth("redirect sem código de autorização".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_is_s256_of_verifier() {
        let pkce = Pkce::generate();
        assert!((43..=128).contains(&pkce.verifier.len()));
        let expected = URL_SAFE_NO_PAD.encode(Sha256::digest(pkce.verifier.as_bytes()));
        assert_eq!(pkce.challenge, expected);
        assert_ne!(Pkce::generate().state, pkce.state);
    }

    #[test]
    fn callback_returns_code_when_state_matches() {
        let code = parse_callback("http://127.0.0.1:47823/callback?code=abc%2F123&state=xyz", "xyz").unwrap();
        assert_eq!(code, "abc/123");
    }

    #[test]
    fn callback_rejects_wrong_state() {
        let err = parse_callback("http://127.0.0.1:47823/callback?code=abc&state=evil", "xyz").unwrap_err();
        assert!(matches!(err, Error::OAuth(_)));
    }

    #[test]
    fn callback_surfaces_provider_error() {
        let err = parse_callback("http://127.0.0.1:47823/callback?error=access_denied&state=xyz", "xyz").unwrap_err();
        assert_eq!(err.to_string(), "autorização recusada ou inválida: access_denied");
    }
}

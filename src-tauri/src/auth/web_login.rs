//! Login "como no navegador": abre a página oficial de login do serviço numa
//! janela do próprio app e espera aparecer o cookie de sessão.
//!
//! Usado por YouTube Music e SoundCloud, que não precisam de app registrado:
//! depois do login, o backend faz as mesmas requisições que o site faz, com
//! os cookies da pessoa. Os cookies ficam guardados no perfil da WebView do
//! sistema (criptografado pelo SO), igual a um navegador comum — não copiamos
//! nada para arquivos nossos.

use std::collections::HashSet;
use std::time::Duration;

use tauri::webview::Cookie;
use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindowBuilder};

use crate::error::{Error, Result};

const POLL_EVERY: Duration = Duration::from_millis(800);
const LOGIN_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, Copy)]
pub struct WebLogin {
    /// Label da janela de login (precisa ser único entre as janelas abertas).
    pub label: &'static str,
    pub title: &'static str,
    pub start_url: &'static str,
    /// Site cujos cookies representam a sessão.
    pub cookie_url: &'static str,
    /// Cookie que só existe com alguém logado.
    pub session_cookie: &'static str,
    /// Sites cujos cookies apagamos ao desconectar.
    pub logout_urls: &'static [&'static str],
}

/// Abre a janela de login e resolve quando o cookie de sessão aparecer.
/// Se a pessoa fechar a janela, devolve erro de login cancelado.
pub async fn login(app: &AppHandle, spec: &WebLogin) -> Result<()> {
    if let Some(open) = app.get_webview_window(spec.label) {
        let _ = open.set_focus();
        return Err(Error::OAuth("a janela de login já está aberta".into()));
    }

    let start = Url::parse(spec.start_url).map_err(|e| Error::Other(e.to_string()))?;
    let builder = WebviewWindowBuilder::new(app, spec.label, WebviewUrl::External(start))
        .title(spec.title)
        .inner_size(480.0, 720.0)
        .center();
    // Mesmas flags da janela principal: o WebView2 recusa (0x8007139F) janelas com
    // opções diferentes no mesmo perfil. Ver `window::WEBVIEW2_ARGS`.
    #[cfg(target_os = "windows")]
    let builder = builder.additional_browser_args(crate::window::WEBVIEW2_ARGS);
    let window = builder
        .build()
        .map_err(|e| Error::Other(format!("não foi possível abrir a janela de login: {e}")))?;

    let deadline = tokio::time::Instant::now() + LOGIN_TIMEOUT;
    loop {
        tokio::time::sleep(POLL_EVERY).await;
        // Janela fechada pela pessoa = desistiu do login.
        if app.get_webview_window(spec.label).is_none() {
            return Err(Error::OAuth("login cancelado".into()));
        }
        if session_cookie(app, spec)?.is_some() {
            let _ = window.close();
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            let _ = window.close();
            return Err(Error::Timeout);
        }
    }
}

/// Valor do cookie de sessão, se houver alguém logado.
/// Atenção: no Windows, ler cookies na thread principal trava (bug do WebView2);
/// chame sempre a partir de comandos `async`.
pub fn session_cookie(app: &AppHandle, spec: &WebLogin) -> Result<Option<String>> {
    Ok(cookies(app, spec.cookie_url)?
        .into_iter()
        .find(|c| c.name() == spec.session_cookie && !c.value().is_empty())
        .map(|c| c.value().to_owned()))
}

/// Monta o header `Cookie` para requisições feitas pelo backend.
pub fn cookie_header(app: &AppHandle, url: &str) -> Result<String> {
    Ok(cookies(app, url)?
        .iter()
        .map(|c| format!("{}={}", c.name(), c.value()))
        .collect::<Vec<_>>()
        .join("; "))
}

/// Exporta os cookies dos sites no formato "Netscape cookies.txt", que o yt-dlp lê com
/// `--cookies`. É assim que o yt-dlp "entra" como a pessoa e passa da checagem de robô do YouTube.
pub fn netscape_cookies(app: &AppHandle, urls: &[&str]) -> Result<String> {
    let mut seen = HashSet::new();
    let mut out = String::from("# Netscape HTTP Cookie File\n");
    for url in urls {
        let host = Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_owned)).unwrap_or_default();
        for cookie in cookies(app, url)? {
            let (domain, subdomains) = match cookie.domain() {
                Some(d) => (format!(".{}", d.trim_start_matches('.')), true),
                None => (host.clone(), false),
            };
            let path = cookie.path().unwrap_or("/").to_owned();
            if !seen.insert((domain.clone(), path.clone(), cookie.name().to_owned())) {
                continue;
            }
            let expires = cookie.expires_datetime().map_or(0, |d| d.unix_timestamp().max(0));
            let secure = cookie.secure().unwrap_or(false);
            out.push_str(&netscape_line(&domain, subdomains, &path, secure, expires, cookie.name(), cookie.value()));
        }
    }
    Ok(out)
}

/// Uma linha do cookies.txt: domínio, vale p/ subdomínios, caminho, só HTTPS, expira (unix; 0 = sessão), nome, valor.
fn netscape_line(domain: &str, subdomains: bool, path: &str, secure: bool, expires: i64, name: &str, value: &str) -> String {
    let flag = |b: bool| if b { "TRUE" } else { "FALSE" };
    format!("{domain}\t{}\t{path}\t{}\t{expires}\t{name}\t{value}\n", flag(subdomains), flag(secure))
}

pub fn logout(app: &AppHandle, spec: &WebLogin) -> Result<()> {
    let window = any_window(app)?;
    for url in spec.logout_urls {
        for cookie in cookies(app, url)? {
            window.delete_cookie(cookie).map_err(|e| Error::Other(e.to_string()))?;
        }
    }
    Ok(())
}

fn cookies(app: &AppHandle, url: &str) -> Result<Vec<Cookie<'static>>> {
    let url = Url::parse(url).map_err(|e| Error::Other(e.to_string()))?;
    any_window(app)?.cookies_for_url(url).map_err(|e| Error::Other(e.to_string()))
}

/// Todas as janelas do app compartilham o mesmo perfil da WebView,
/// então qualquer uma serve para ler os cookies.
fn any_window(app: &AppHandle) -> Result<tauri::WebviewWindow> {
    app.get_webview_window("main")
        .or_else(|| app.webview_windows().into_values().next())
        .ok_or_else(|| Error::Other("nenhuma janela aberta".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn netscape_line_matches_the_cookies_txt_format() {
        assert_eq!(
            netscape_line(".youtube.com", true, "/", true, 1_900_000_000, "SAPISID", "abc"),
            ".youtube.com\tTRUE\t/\tTRUE\t1900000000\tSAPISID\tabc\n"
        );
        assert_eq!(
            netscape_line("music.youtube.com", false, "/", false, 0, "PREF", "x=1"),
            "music.youtube.com\tFALSE\t/\tFALSE\t0\tPREF\tx=1\n",
            "cookie de sessão expira em 0"
        );
    }
}

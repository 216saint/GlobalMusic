//! Cliente do servidor social do GlobalMusic: conta própria, perfil com foto e amizades.
//!
//! O servidor fica na VPS (código em `server/`, desenho em
//! docs/decisoes/0008-servidor-social.md). O app guarda só o **token de sessão**,
//! no cofre de credenciais do sistema, como os tokens do Spotify.
//!
//! Login:
//! - **Spotify:** o app já tem um access token do Spotify; mandamos ao servidor,
//!   que confere com o próprio Spotify de quem ele é.
//! - **Discord:** o navegador vai ao servidor → Discord → servidor → loopback do
//!   app (`http://127.0.0.1:47823/login?code=<sessão>`), o mesmo caminho do OAuth do Spotify.

use std::sync::Mutex;

use reqwest::{Method, RequestBuilder, Response, StatusCode};
use serde::{Deserialize, Serialize};

use crate::config::OAUTH_PORT;
use crate::error::{Error, Result};
use crate::models::Track;

const KEYRING_SERVICE: &str = "com.globalmusic.app";
const KEYRING_SESSION: &str = "globalmusic-session";
const SESSION_EXPIRED: &str = "sua sessão do GlobalMusic expirou; entre de novo";

/// Endereço do servidor. Em desenvolvimento dá para apontar para um servidor local
/// com `GLOBALMUSIC_API_URL=http://127.0.0.1:3480`.
fn base_url() -> String {
    let fallback = || crate::config::SOCIAL_API_URL.to_owned();
    if cfg!(debug_assertions) {
        std::env::var("GLOBALMUSIC_API_URL").ok().filter(|v| !v.is_empty()).unwrap_or_else(fallback)
    } else {
        fallback()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoginProvider {
    Discord,
    Spotify,
}

/// O próprio perfil.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Me {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub discord_linked: bool,
    pub spotify_linked: bool,
    /// Nome e foto de cada login vinculado (para escolher qual usar no perfil).
    #[serde(default)]
    pub linked_profiles: Vec<LinkedProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedProfile {
    pub provider: String,
    pub name: String,
    pub avatar_url: Option<String>,
}

/// O que os outros veem de alguém.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicUser {
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FriendList {
    pub friends: Vec<PublicUser>,
    pub incoming: Vec<PublicUser>,
    pub outgoing: Vec<PublicUser>,
}

/// Playlist do GlobalMusic compartilhada: uma cópia guardada no servidor, aberta por um link.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedPlaylist {
    pub name: String,
    pub tracks: Vec<Track>,
    pub cover: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ShareLink {
    pub url: String,
}

#[derive(Deserialize)]
struct LoginResponse {
    token: String,
    user: Me,
}

#[derive(Deserialize)]
struct ErrorBody {
    error: String,
}

pub struct Social {
    http: reqwest::Client,
    base: String,
    /// `None` = ainda não lido do cofre; `Some(None)` = sem sessão.
    session: Mutex<Option<Option<String>>>,
}

fn keyring_entry() -> Result<keyring::Entry> {
    Ok(keyring::Entry::new(KEYRING_SERVICE, KEYRING_SESSION)?)
}

impl Social {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http, base: base_url(), session: Mutex::new(None) }
    }

    fn token(&self) -> Option<String> {
        let mut guard = self.session.lock().unwrap_or_else(|e| e.into_inner());
        guard
            .get_or_insert_with(|| keyring_entry().ok().and_then(|e| e.get_password().ok()))
            .clone()
    }

    fn set_token(&self, token: Option<&str>) -> Result<()> {
        let entry = keyring_entry()?;
        match token {
            Some(t) => entry.set_password(t)?,
            None => match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => {}
                Err(e) => return Err(e.into()),
            },
        }
        *self.session.lock().unwrap_or_else(|e| e.into_inner()) = Some(token.map(str::to_owned));
        Ok(())
    }

    fn url(&self, segments: &[&str]) -> Result<url::Url> {
        let mut url = url::Url::parse(&self.base).map_err(|e| Error::Other(format!("endereço do servidor inválido: {e}")))?;
        // `push` codifica cada pedaço: um @usuário estranho nunca vira outro caminho.
        url.path_segments_mut().map_err(|_| Error::Other("endereço do servidor inválido".into()))?.extend(segments);
        Ok(url)
    }

    fn authed(&self, method: Method, segments: &[&str]) -> Result<RequestBuilder> {
        let token = self.token().ok_or_else(|| Error::Other("entre na sua conta GlobalMusic primeiro".into()))?;
        Ok(self.http.request(method, self.url(segments)?).bearer_auth(token))
    }

    /// Envia e transforma `{"error": "..."}` do servidor numa mensagem legível.
    /// 401 = a sessão acabou: esquecemos o token para a UI voltar ao "Entrar".
    async fn send(&self, request: RequestBuilder) -> Result<Response> {
        let response = request.send().await?;
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        if status == StatusCode::UNAUTHORIZED {
            self.set_token(None)?;
            return Err(Error::Other(SESSION_EXPIRED.into()));
        }
        let message = match response.json::<ErrorBody>().await {
            Ok(body) => body.error,
            Err(_) => format!("o servidor do GlobalMusic respondeu {status}"),
        };
        Err(Error::Other(message))
    }

    /// Perfil da sessão salva; `None` se não há sessão (ou ela expirou).
    pub async fn me(&self) -> Result<Option<Me>> {
        if self.token().is_none() {
            return Ok(None);
        }
        match self.send(self.authed(Method::GET, &["me"])?).await {
            Ok(response) => Ok(Some(response.json().await?)),
            Err(_) if self.token().is_none() => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub async fn login_spotify(&self, access_token: &str) -> Result<Me> {
        let body = serde_json::json!({ "accessToken": access_token });
        let response = self.send(self.http.post(self.url(&["auth", "spotify"])?).json(&body)).await?;
        let login: LoginResponse = response.json().await?;
        self.set_token(Some(&login.token))?;
        Ok(login.user)
    }

    pub async fn link_spotify(&self, access_token: &str) -> Result<Me> {
        let body = serde_json::json!({ "accessToken": access_token });
        Ok(self.send(self.authed(Method::POST, &["me", "link", "spotify"])?.json(&body)).await?.json().await?)
    }

    /// Página que o navegador abre para entrar (ou vincular, com `link_ticket`) pelo Discord.
    pub fn discord_start_url(&self, app_state: &str, link_ticket: Option<&str>) -> Result<String> {
        let mut url = self.url(&["auth", "discord", "start"])?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("state", app_state).append_pair("port", &OAUTH_PORT.to_string());
            if let Some(ticket) = link_ticket {
                query.append_pair("link", ticket);
            }
        }
        Ok(url.into())
    }

    /// Pede um tíquete de uso único para vincular o Discord à conta logada.
    pub async fn discord_link_ticket(&self) -> Result<String> {
        #[derive(Deserialize)]
        struct Ticket {
            ticket: String,
        }
        let ticket: Ticket = self.send(self.authed(Method::POST, &["me", "link", "discord"])?).await?.json().await?;
        Ok(ticket.ticket)
    }

    /// Guarda a sessão que voltou do login pelo navegador e devolve o perfil.
    pub async fn adopt_session(&self, token: &str) -> Result<Me> {
        self.set_token(Some(token))?;
        self.me().await?.ok_or_else(|| Error::Other(SESSION_EXPIRED.into()))
    }

    /// Sai: avisa o servidor (para invalidar o token) e apaga do cofre mesmo se offline.
    pub async fn logout(&self) -> Result<()> {
        if let Ok(request) = self.authed(Method::POST, &["auth", "logout"]) {
            let _ = request.send().await;
        }
        self.set_token(None)
    }

    pub async fn update_profile(&self, display_name: Option<String>, username: Option<String>) -> Result<Me> {
        let body = serde_json::json!({ "displayName": display_name, "username": username });
        Ok(self.send(self.authed(Method::PATCH, &["me"])?.json(&body)).await?.json().await?)
    }

    /// Usa o nome e a foto que a pessoa tem no Discord ou no Spotify.
    pub async fn use_linked_profile(&self, provider: LoginProvider) -> Result<Me> {
        let provider = match provider {
            LoginProvider::Discord => "discord",
            LoginProvider::Spotify => "spotify",
        };
        let body = serde_json::json!({ "provider": provider });
        Ok(self.send(self.authed(Method::POST, &["me", "profile-from"])?.json(&body)).await?.json().await?)
    }

    pub async fn upload_avatar(&self, image: Vec<u8>) -> Result<Me> {
        let request = self.authed(Method::PUT, &["me", "avatar"])?.header("content-type", "application/octet-stream").body(image);
        Ok(self.send(request).await?.json().await?)
    }

    /// Guarda uma cópia da playlist no servidor e devolve o link para mandar a quem quiser.
    pub async fn share_playlist(&self, playlist: &SharedPlaylist) -> Result<ShareLink> {
        Ok(self.send(self.authed(Method::POST, &["playlists"])?.json(playlist)).await?.json().await?)
    }

    /// Abre uma playlist compartilhada (não precisa de conta).
    pub async fn shared_playlist(&self, code: &str) -> Result<SharedPlaylist> {
        Ok(self.send(self.http.get(self.url(&["playlists", code])?)).await?.json().await?)
    }

    pub async fn search(&self, query: &str) -> Result<Vec<PublicUser>> {
        let request = self.authed(Method::GET, &["users", "search"])?.query(&[("q", query)]);
        Ok(self.send(request).await?.json().await?)
    }

    pub async fn friends(&self) -> Result<FriendList> {
        Ok(self.send(self.authed(Method::GET, &["friends"])?).await?.json().await?)
    }

    /// Pede amizade; se a outra pessoa já tinha pedido, aceita.
    pub async fn add_friend(&self, username: &str) -> Result<FriendList> {
        Ok(self.send(self.authed(Method::POST, &["friends", username])?).await?.json().await?)
    }

    /// Desfaz amizade, recusa um pedido recebido ou cancela um enviado.
    pub async fn remove_friend(&self, username: &str) -> Result<FriendList> {
        Ok(self.send(self.authed(Method::DELETE, &["friends", username])?).await?.json().await?)
    }
}

// ------------------------------------------------------------------ comandos (src/lib/api.ts)

use tauri::ipc::{InvokeBody, Request};
use tauri::{AppHandle, State};

use crate::auth::oauth::{authorize_via_browser, Pkce};
use crate::state::AppState;

/// Login do Discord pelo navegador: devolve a sessão que o servidor mandou ao loopback.
async fn discord_session(app: &AppHandle, social: &Social, link_ticket: Option<&str>) -> Result<String> {
    let app_state = Pkce::generate().state;
    let url = social.discord_start_url(&app_state, link_ticket)?;
    authorize_via_browser(app, &url, OAUTH_PORT, &app_state).await
}

#[tauri::command]
pub async fn social_me(state: State<'_, AppState>) -> Result<Option<Me>> {
    state.social.me().await
}

#[tauri::command]
pub async fn social_login(app: AppHandle, state: State<'_, AppState>, provider: LoginProvider) -> Result<Me> {
    match provider {
        LoginProvider::Spotify => {
            let token = state.providers.spotify.access_token().await?;
            state.social.login_spotify(&token).await
        }
        LoginProvider::Discord => {
            let session = discord_session(&app, &state.social, None).await?;
            state.social.adopt_session(&session).await
        }
    }
}

/// Vincula mais um login à conta já aberta (entrar depois por qualquer um dos dois).
#[tauri::command]
pub async fn social_link(app: AppHandle, state: State<'_, AppState>, provider: LoginProvider) -> Result<Me> {
    match provider {
        LoginProvider::Spotify => {
            let token = state.providers.spotify.access_token().await?;
            state.social.link_spotify(&token).await
        }
        LoginProvider::Discord => {
            let ticket = state.social.discord_link_ticket().await?;
            let session = discord_session(&app, &state.social, Some(&ticket)).await?;
            state.social.adopt_session(&session).await
        }
    }
}

#[tauri::command]
pub async fn social_logout(state: State<'_, AppState>) -> Result<()> {
    state.social.logout().await
}

#[tauri::command]
pub async fn social_update_profile(
    state: State<'_, AppState>,
    display_name: Option<String>,
    username: Option<String>,
) -> Result<Me> {
    state.social.update_profile(display_name, username).await
}

#[tauri::command]
pub async fn social_use_profile(state: State<'_, AppState>, provider: LoginProvider) -> Result<Me> {
    state.social.use_linked_profile(provider).await
}

/// A foto chega crua (bytes, não JSON): o frontend já reduziu e converteu para JPEG.
#[tauri::command]
pub async fn social_upload_avatar(state: State<'_, AppState>, request: Request<'_>) -> Result<Me> {
    let InvokeBody::Raw(image) = request.body() else {
        return Err(Error::Other("a foto precisa vir como bytes".into()));
    };
    state.social.upload_avatar(image.clone()).await
}

#[tauri::command]
pub async fn social_search(state: State<'_, AppState>, query: String) -> Result<Vec<PublicUser>> {
    state.social.search(&query).await
}

#[tauri::command]
pub async fn social_friends(state: State<'_, AppState>) -> Result<FriendList> {
    state.social.friends().await
}

#[tauri::command]
pub async fn social_add_friend(state: State<'_, AppState>, username: String) -> Result<FriendList> {
    state.social.add_friend(&username).await
}

#[tauri::command]
pub async fn social_remove_friend(state: State<'_, AppState>, username: String) -> Result<FriendList> {
    state.social.remove_friend(&username).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn social() -> Social {
        Social { http: reqwest::Client::new(), base: "https://api.example.com".into(), session: Mutex::new(Some(None)) }
    }

    #[test]
    fn path_segments_are_encoded() {
        let url = social().url(&["friends", "../me?x=1"]).unwrap();
        assert_eq!(url.as_str(), "https://api.example.com/friends/..%2Fme%3Fx=1");
    }

    #[test]
    fn discord_start_url_carries_state_port_and_ticket() {
        let url = social().discord_start_url("abc", Some("t1")).unwrap();
        assert_eq!(url, format!("https://api.example.com/auth/discord/start?state=abc&port={OAUTH_PORT}&link=t1"));
    }
}

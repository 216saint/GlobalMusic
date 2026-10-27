//! Login e sessões.
//!
//! - **Spotify:** o app já está logado no Spotify; ele manda o access token e
//!   nós perguntamos ao próprio Spotify de quem é (`GET /v1/me`).
//! - **Discord:** OAuth2 padrão. O navegador vai ao Discord, volta para
//!   `/auth/discord/callback` e nós redirecionamos ao app no loopback
//!   (`http://127.0.0.1:<porta>/login?code=<sessão>&state=<state do app>`).
//!   O nome `code` é proposital: o app reaproveita o mesmo leitor de redirect
//!   do OAuth do Spotify.
//! - **Sessão:** token aleatório de 256 bits. No banco vai só o SHA-256 dele.

use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::extract::{FromRequestParts, Query, State};
use axum::http::request::Parts;
use axum::response::Redirect;
use axum::Json;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::RngCore;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::error::{ApiError, ApiResult};
use crate::users::{self, Identity, Me};
use crate::AppState;

const SESSION_DAYS: i64 = 90;
/// Renovamos a validade quando faltar menos que isto (sessão "rolante").
const RENEW_BELOW_DAYS: i64 = 60;
const PENDING_TTL: Duration = Duration::from_secs(10 * 60);

pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

pub fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn hash_token(token: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(token.as_bytes()))
}

async fn create_session(state: &AppState, user_id: i64) -> ApiResult<String> {
    let token = random_token();
    sqlx::query("INSERT INTO sessions (token_hash, user_id, expires_at) VALUES (?, ?, ?)")
        .bind(hash_token(&token))
        .bind(user_id)
        .bind(now() + SESSION_DAYS * 86_400)
        .execute(&state.db)
        .await?;
    Ok(token)
}

/// Extrator: `AuthUser(id)` em qualquer handler exige `Authorization: Bearer <sessão>`.
pub struct AuthUser(pub i64);

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> ApiResult<Self> {
        let token = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(ApiError::Unauthorized)?;
        let hash = hash_token(token.trim());
        let row: Option<(i64, i64)> =
            sqlx::query_as("SELECT user_id, expires_at FROM sessions WHERE token_hash = ? AND expires_at > ?")
                .bind(&hash)
                .bind(now())
                .fetch_optional(&state.db)
                .await?;
        let (user_id, expires_at) = row.ok_or(ApiError::Unauthorized)?;
        if expires_at - now() < RENEW_BELOW_DAYS * 86_400 {
            sqlx::query("UPDATE sessions SET expires_at = ? WHERE token_hash = ?")
                .bind(now() + SESSION_DAYS * 86_400)
                .bind(&hash)
                .execute(&state.db)
                .await?;
        }
        Ok(AuthUser(user_id))
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
    pub token: String,
    pub user: Me,
}

// ------------------------------------------------------------------ Spotify

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotifyLogin {
    access_token: String,
}

#[derive(Deserialize)]
struct SpotifyMe {
    id: String,
    display_name: Option<String>,
    #[serde(default)]
    images: Vec<SpotifyImage>,
}

#[derive(Deserialize)]
struct SpotifyImage {
    url: String,
}

/// Confirma com o próprio Spotify de quem é o token.
async fn spotify_identity(state: &AppState, access_token: &str) -> ApiResult<Identity> {
    let response = state.http.get("https://api.spotify.com/v1/me").bearer_auth(access_token).send().await?;
    match response.status().as_u16() {
        200 => {}
        // 400, não 401: o 401 do app significa "sua sessão GlobalMusic expirou".
        401 => return Err(ApiError::BadRequest("token do Spotify inválido ou expirado; reconecte o Spotify".into())),
        429 => return Err(ApiError::Upstream("o Spotify está limitando agora; tente de novo ou entre com o Discord".into())),
        code => return Err(ApiError::Upstream(format!("Spotify respondeu {code}"))),
    }
    let me: SpotifyMe = response.json().await?;
    Ok(Identity::Spotify {
        display_name: me.display_name.unwrap_or_else(|| me.id.clone()),
        avatar_url: me.images.into_iter().next().map(|i| i.url),
        id: me.id,
    })
}

pub async fn login_spotify(State(state): State<AppState>, Json(body): Json<SpotifyLogin>) -> ApiResult<Json<LoginResponse>> {
    let identity = spotify_identity(&state, &body.access_token).await?;
    let user_id = users::find_or_create(&state, &identity).await?;
    let token = create_session(&state, user_id).await?;
    Ok(Json(LoginResponse { token, user: users::me(&state, user_id).await? }))
}

pub async fn link_spotify(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Json(body): Json<SpotifyLogin>,
) -> ApiResult<Json<Me>> {
    let identity = spotify_identity(&state, &body.access_token).await?;
    users::link(&state, user_id, &identity).await?;
    Ok(Json(users::me(&state, user_id).await?))
}

// ------------------------------------------------------------------ Discord

/// Login do Discord em andamento (entre o "start" e o "callback").
pub struct PendingDiscord {
    app_state: String,
    port: u16,
    /// Vincular a uma conta existente (em vez de entrar).
    link_user: Option<i64>,
    created: Instant,
}

/// Tickets de vínculo: o app (já logado) pede um, e o navegador o leva ao Discord.
pub struct LinkTicket {
    user_id: i64,
    created: Instant,
}

#[derive(Default)]
pub struct PendingLogins {
    pub discord: HashMap<String, PendingDiscord>,
    pub link_tickets: HashMap<String, LinkTicket>,
}

impl PendingLogins {
    fn sweep(&mut self) {
        self.discord.retain(|_, p| p.created.elapsed() < PENDING_TTL);
        self.link_tickets.retain(|_, t| t.created.elapsed() < PENDING_TTL);
    }
}

#[derive(Deserialize)]
pub struct DiscordStart {
    state: String,
    port: u16,
    link: Option<String>,
}

/// Volta ao app (loopback) com um erro: o app mostra o motivo no Perfil na hora, em vez de
/// a pessoa ver um JSON no navegador e o app esperar 5 minutos por um retorno que não vem.
fn back_to_app(port: u16, app_state: &str, error: &str) -> Redirect {
    let mut url = url::Url::parse(&format!("http://127.0.0.1:{port}/login")).expect("URL fixa");
    url.query_pairs_mut().append_pair("error", error).append_pair("state", app_state);
    Redirect::to(url.as_str())
}

pub async fn discord_start(State(state): State<AppState>, Query(q): Query<DiscordStart>) -> ApiResult<Redirect> {
    // Só aceitamos voltar para o loopback do app em portas altas: nunca para um site qualquer.
    if q.port < 1024 || q.state.is_empty() || q.state.len() > 128 {
        return Err(ApiError::BadRequest("parâmetros inválidos".into()));
    }
    let Some(config) = state.config.discord.as_ref() else {
        return Ok(back_to_app(q.port, &q.state, "o login com Discord ainda não foi ativado no servidor"));
    };
    let mut pending = state.pending.lock().await;
    pending.sweep();
    let link_user = match q.link {
        Some(ticket) => match pending.link_tickets.remove(&ticket) {
            Some(t) => Some(t.user_id),
            None => return Ok(back_to_app(q.port, &q.state, "o vínculo expirou; tente de novo")),
        },
        None => None,
    };
    let server_state = random_token();
    pending
        .discord
        .insert(server_state.clone(), PendingDiscord { app_state: q.state, port: q.port, link_user, created: Instant::now() });

    let mut url = url::Url::parse("https://discord.com/oauth2/authorize").expect("URL fixa");
    url.query_pairs_mut()
        .append_pair("client_id", &config.client_id)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", &config.redirect_uri)
        .append_pair("scope", "identify")
        .append_pair("state", &server_state)
        .append_pair("prompt", "none");
    Ok(Redirect::to(url.as_str()))
}

#[derive(Deserialize)]
pub struct DiscordCallback {
    code: Option<String>,
    state: String,
    error: Option<String>,
}

#[derive(Deserialize)]
struct DiscordToken {
    access_token: String,
}

#[derive(Deserialize)]
struct DiscordUser {
    id: String,
    username: String,
    global_name: Option<String>,
    avatar: Option<String>,
}

pub async fn discord_callback(State(state): State<AppState>, Query(q): Query<DiscordCallback>) -> ApiResult<Redirect> {
    let pending = state
        .pending
        .lock()
        .await
        .discord
        .remove(&q.state)
        .filter(|p| p.created.elapsed() < PENDING_TTL)
        .ok_or_else(|| ApiError::BadRequest("login expirado; volte ao GlobalMusic e tente de novo".into()))?;
    let Some(code) = q.code else {
        let reason = match q.error.as_deref() {
            Some("access_denied") | None => "você cancelou a autorização no Discord",
            Some(other) => other,
        };
        return Ok(back_to_app(pending.port, &pending.app_state, reason));
    };

    let result: ApiResult<String> = async {
        let config = state.config.discord.as_ref().ok_or_else(|| ApiError::Forbidden("Discord não configurado".into()))?;
        let token: DiscordToken = state
            .http
            .post("https://discord.com/api/oauth2/token")
            .basic_auth(&config.client_id, Some(&config.client_secret))
            .form(&[("grant_type", "authorization_code"), ("code", &code), ("redirect_uri", &config.redirect_uri)])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let user: DiscordUser = state
            .http
            .get("https://discord.com/api/users/@me")
            .bearer_auth(&token.access_token)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let identity = Identity::Discord {
            avatar_url: user.avatar.map(|hash| format!("https://cdn.discordapp.com/avatars/{}/{hash}.png?size=256", user.id)),
            display_name: user.global_name.unwrap_or_else(|| user.username.clone()),
            id: user.id,
        };
        let user_id = match pending.link_user {
            Some(user_id) => {
                users::link(&state, user_id, &identity).await?;
                user_id
            }
            None => users::find_or_create(&state, &identity).await?,
        };
        create_session(&state, user_id).await
    }
    .await;

    Ok(match result {
        Ok(session) => {
            let mut url = url::Url::parse(&format!("http://127.0.0.1:{}/login", pending.port)).expect("URL fixa");
            url.query_pairs_mut().append_pair("code", &session).append_pair("state", &pending.app_state);
            Redirect::to(url.as_str())
        }
        Err(e) => back_to_app(pending.port, &pending.app_state, &e.to_string()),
    })
}

pub async fn link_discord_ticket(State(state): State<AppState>, AuthUser(user_id): AuthUser) -> ApiResult<Json<serde_json::Value>> {
    let ticket = random_token();
    let mut pending = state.pending.lock().await;
    pending.sweep();
    pending.link_tickets.insert(ticket.clone(), LinkTicket { user_id, created: Instant::now() });
    Ok(Json(serde_json::json!({ "ticket": ticket })))
}

pub async fn logout(State(state): State<AppState>, parts: axum::http::HeaderMap) -> ApiResult<()> {
    if let Some(token) = parts.get("authorization").and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer ")) {
        sqlx::query("DELETE FROM sessions WHERE token_hash = ?").bind(hash_token(token.trim())).execute(&state.db).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_random_and_only_the_hash_is_stored() {
        let a = random_token();
        let b = random_token();
        assert_ne!(a, b);
        assert_eq!(a.len(), 43, "32 bytes em base64url sem padding");
        assert_ne!(hash_token(&a), a);
        assert_eq!(hash_token(&a), hash_token(&a), "hash determinístico para a busca");
    }
}

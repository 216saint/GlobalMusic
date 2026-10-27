//! Playlists do GlobalMusic compartilhadas por link.
//!
//! Quem compartilha (com conta) manda uma cópia da playlist e recebe `PUBLIC_URL/p/<código>`. Quem
//! recebe cola o link no app ("Nova playlist" → "Importar por link"), que busca o JSON em
//! `/playlists/<código>` e cria uma cópia própria. Aberto no navegador, o link mostra uma página simples
//! com as músicas e como abrir no GlobalMusic.

use axum::extract::{Path, State};
use axum::response::Html;
use axum::Json;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

use crate::auth::{now, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::users::PublicUser;
use crate::AppState;

/// Tamanho máximo do pedido de compartilhamento (faixas + foto).
pub const SHARE_MAX_BYTES: usize = 1024 * 1024;
const MAX_TRACKS: usize = 1000;
const MAX_NAME: usize = 100;
const MAX_COVER_BYTES: usize = 400 * 1024;
const COVER_PREFIX: &str = "data:image/jpeg;base64,";
/// Por pessoa: evita que alguém use o servidor como depósito.
const MAX_PER_OWNER: i64 = 300;
const CODE_LEN: usize = 10;
const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";
const DOWNLOAD_URL: &str = "https://github.com/216saint/GlobalMusic";

#[derive(Deserialize)]
pub struct ShareRequest {
    name: String,
    tracks: Box<RawValue>,
    cover: Option<String>,
}

#[derive(Serialize)]
pub struct ShareLink {
    code: String,
    url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedPlaylist {
    name: String,
    tracks: Box<RawValue>,
    cover: Option<String>,
    owner: PublicUser,
    created_at: i64,
}

#[derive(sqlx::FromRow)]
struct Row {
    name: String,
    tracks: String,
    cover: Option<String>,
    username: String,
    display_name: String,
    avatar_url: Option<String>,
    created_at: i64,
}

fn new_code() -> String {
    let mut rng = rand::rng();
    (0..CODE_LEN).map(|_| CODE_ALPHABET[rng.random_range(0..CODE_ALPHABET.len())] as char).collect()
}

/// Confere o que o app mandou: nome, uma lista de faixas de tamanho razoável e foto JPEG.
fn validate(request: &ShareRequest) -> ApiResult<String> {
    let name: String = request.name.trim().chars().take(MAX_NAME).collect();
    if name.is_empty() {
        return Err(ApiError::BadRequest("a playlist precisa de um nome".into()));
    }
    let tracks: Vec<&RawValue> = serde_json::from_str(request.tracks.get())
        .map_err(|_| ApiError::BadRequest("lista de músicas inválida".into()))?;
    if tracks.is_empty() || tracks.len() > MAX_TRACKS {
        return Err(ApiError::BadRequest(format!("a playlist precisa ter de 1 a {MAX_TRACKS} músicas")));
    }
    if let Some(cover) = &request.cover {
        if !cover.starts_with(COVER_PREFIX) || cover.len() > MAX_COVER_BYTES {
            return Err(ApiError::BadRequest("foto da playlist inválida".into()));
        }
    }
    Ok(name)
}

pub async fn share(
    State(state): State<AppState>,
    AuthUser(me): AuthUser,
    Json(request): Json<ShareRequest>,
) -> ApiResult<Json<ShareLink>> {
    let name = validate(&request)?;
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM shared_playlists WHERE owner_id = ?")
        .bind(me)
        .fetch_one(&state.db)
        .await?;
    if count >= MAX_PER_OWNER {
        return Err(ApiError::Conflict("você já compartilhou playlists demais".into()));
    }
    let code = new_code();
    sqlx::query("INSERT INTO shared_playlists (code, owner_id, name, tracks, cover, created_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(&code)
        .bind(me)
        .bind(&name)
        .bind(request.tracks.get())
        .bind(&request.cover)
        .bind(now())
        .execute(&state.db)
        .await?;
    tracing::info!("playlist compartilhada: {code} ({name})");
    let url = format!("{}/p/{code}", state.config.public_url);
    Ok(Json(ShareLink { code, url }))
}

async fn find(state: &AppState, code: &str) -> ApiResult<Row> {
    sqlx::query_as::<_, Row>(
        "SELECT p.name, p.tracks, p.cover, p.created_at, u.username, u.display_name, u.avatar_url
           FROM shared_playlists p JOIN users u ON u.id = p.owner_id WHERE p.code = ?",
    )
    .bind(code)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::NotFound("essa playlist compartilhada não existe (ou foi apagada)".into()))
}

/// O JSON que o app importa.
pub async fn get(State(state): State<AppState>, Path(code): Path<String>) -> ApiResult<Json<SharedPlaylist>> {
    let row = find(&state, &code).await?;
    let tracks = RawValue::from_string(row.tracks).map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(SharedPlaylist {
        name: row.name,
        tracks,
        cover: row.cover,
        owner: PublicUser { username: row.username, display_name: row.display_name, avatar_url: row.avatar_url },
        created_at: row.created_at,
    }))
}

#[derive(Deserialize)]
struct PageTrack {
    title: String,
    #[serde(default)]
    artists: Vec<String>,
}

/// O link aberto no navegador: o nome, as músicas e como abrir no GlobalMusic.
pub async fn page(State(state): State<AppState>, Path(code): Path<String>) -> ApiResult<Html<String>> {
    let row = find(&state, &code).await?;
    let tracks: Vec<PageTrack> = serde_json::from_str(&row.tracks).unwrap_or_default();
    let items: String = tracks
        .iter()
        .take(200)
        .map(|t| format!("<li><b>{}</b><span>{}</span></li>", escape(&t.title), escape(&t.artists.join(", "))))
        .collect();
    let cover = row
        .cover
        .as_deref()
        .map(|c| format!(r#"<img class="cover" src="{}" alt="">"#, escape(c)))
        .unwrap_or_default();
    let link = format!("{}/p/{code}", state.config.public_url);
    Ok(Html(format!(
        include_str!("share_page.html"),
        name = escape(&row.name),
        owner = escape(&row.display_name),
        count = tracks.len(),
        cover = cover,
        items = items,
        link = escape(&link),
        download = DOWNLOAD_URL,
    )))
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(name: &str, tracks: &str, cover: Option<&str>) -> ShareRequest {
        ShareRequest {
            name: name.into(),
            tracks: RawValue::from_string(tracks.into()).unwrap(),
            cover: cover.map(str::to_owned),
        }
    }

    #[test]
    fn accepts_only_sane_playlists() {
        assert_eq!(validate(&request("  Treino ", r#"[{"title":"a"}]"#, None)).unwrap(), "Treino");
        assert!(validate(&request(" ", r#"[{"title":"a"}]"#, None)).is_err(), "sem nome");
        assert!(validate(&request("x", "[]", None)).is_err(), "sem músicas");
        assert!(validate(&request("x", r#"{"a":1}"#, None)).is_err(), "não é lista");
        assert!(validate(&request("x", r#"[1]"#, Some("data:text/html,oi"))).is_err(), "foto que não é JPEG");
    }

    #[test]
    fn codes_are_unambiguous_and_html_is_escaped() {
        let code = new_code();
        assert_eq!(code.len(), CODE_LEN);
        assert!(code.bytes().all(|b| CODE_ALPHABET.contains(&b)));
        assert_eq!(escape(r#"<b a="1">&'"#), "&lt;b a=&quot;1&quot;&gt;&amp;&#39;");
    }
}

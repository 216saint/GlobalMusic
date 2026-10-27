//! Contas e perfis: criar/vincular pelo Discord ou Spotify, editar nome, @usuário e foto.

use std::io::Cursor;

use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::Json;
use image::imageops::FilterType;
use image::ImageFormat;
use serde::{Deserialize, Serialize};

use crate::auth::{now, random_token, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::AppState;

pub const AVATAR_MAX_BYTES: usize = 2 * 1024 * 1024;
const AVATAR_PX: u32 = 256;
const USERNAME_MIN: usize = 3;
const USERNAME_MAX: usize = 20;
const DISPLAY_NAME_MAX: usize = 40;

/// Quem a pessoa provou ser, num dos serviços de login.
pub enum Identity {
    Discord { id: String, display_name: String, avatar_url: Option<String> },
    Spotify { id: String, display_name: String, avatar_url: Option<String> },
}

impl Identity {
    fn column(&self) -> &'static str {
        match self {
            Identity::Discord { .. } => "discord_id",
            Identity::Spotify { .. } => "spotify_id",
        }
    }
    fn id(&self) -> &str {
        match self {
            Identity::Discord { id, .. } | Identity::Spotify { id, .. } => id,
        }
    }
    fn display_name(&self) -> &str {
        match self {
            Identity::Discord { display_name, .. } | Identity::Spotify { display_name, .. } => display_name,
        }
    }
    fn avatar_url(&self) -> Option<&str> {
        match self {
            Identity::Discord { avatar_url, .. } | Identity::Spotify { avatar_url, .. } => avatar_url.as_deref(),
        }
    }
    fn label(&self) -> &'static str {
        match self {
            Identity::Discord { .. } => "Discord",
            Identity::Spotify { .. } => "Spotify",
        }
    }
}

/// O próprio perfil (vê quais logins estão vinculados).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Me {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub discord_linked: bool,
    pub spotify_linked: bool,
}

/// O que os outros veem.
#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PublicUser {
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
}

pub async fn me(state: &AppState, user_id: i64) -> ApiResult<Me> {
    let row: (i64, String, String, Option<String>, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT id, username, display_name, avatar_url, discord_id, spotify_id FROM users WHERE id = ?",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(ApiError::Unauthorized)?;
    Ok(Me {
        id: row.0,
        username: row.1,
        display_name: row.2,
        avatar_url: row.3,
        discord_linked: row.4.is_some(),
        spotify_linked: row.5.is_some(),
    })
}

/// Acha a conta ligada a esta identidade ou cria uma nova.
pub async fn find_or_create(state: &AppState, identity: &Identity) -> ApiResult<i64> {
    let existing: Option<(i64,)> = sqlx::query_as(&format!("SELECT id FROM users WHERE {} = ?", identity.column()))
        .bind(identity.id())
        .fetch_optional(&state.db)
        .await?;
    if let Some((id,)) = existing {
        return Ok(id);
    }
    let username = free_username(state, &slugify(identity.display_name())).await?;
    let display_name: String = identity.display_name().chars().take(DISPLAY_NAME_MAX).collect();
    let result = sqlx::query(&format!(
        "INSERT INTO users (username, display_name, avatar_url, {}, created_at) VALUES (?, ?, ?, ?, ?)",
        identity.column()
    ))
    .bind(&username)
    .bind(display_name)
    .bind(identity.avatar_url())
    .bind(identity.id())
    .bind(now())
    .execute(&state.db)
    .await?;
    tracing::info!("nova conta @{username} via {}", identity.label());
    Ok(result.last_insert_rowid())
}

/// Vincula outro login à conta (ex.: entrou pelo Spotify e liga o Discord).
pub async fn link(state: &AppState, user_id: i64, identity: &Identity) -> ApiResult<()> {
    let owner: Option<(i64,)> = sqlx::query_as(&format!("SELECT id FROM users WHERE {} = ?", identity.column()))
        .bind(identity.id())
        .fetch_optional(&state.db)
        .await?;
    match owner {
        Some((id,)) if id == user_id => Ok(()),
        Some(_) => Err(ApiError::Conflict(format!("esta conta do {} já está ligada a outro perfil", identity.label()))),
        None => {
            sqlx::query(&format!("UPDATE users SET {} = ? WHERE id = ?", identity.column()))
                .bind(identity.id())
                .bind(user_id)
                .execute(&state.db)
                .await?;
            Ok(())
        }
    }
}

/// "Ana Clara!" → "ana_clara"; garante o tamanho mínimo/máximo.
pub fn slugify(name: &str) -> String {
    let mut slug = String::new();
    for c in name.to_lowercase().chars() {
        let c = match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if (c == '_' || c == '.' || c.is_whitespace() || c == '-') && !slug.ends_with('_') && !slug.is_empty() {
            slug.push('_');
        }
    }
    let mut slug: String = slug.trim_end_matches('_').chars().take(USERNAME_MAX - 4).collect();
    while slug.len() < USERNAME_MIN {
        slug.push('x');
    }
    slug
}

/// Acrescenta um número se o @usuário já existir ("ana", "ana2", "ana7341"...).
async fn free_username(state: &AppState, base: &str) -> ApiResult<String> {
    for attempt in 0..20 {
        let candidate = match attempt {
            0 => base.to_owned(),
            1..=9 => format!("{base}{}", attempt + 1),
            _ => format!("{base}{}", rand::random_range(1000..9999)),
        };
        let taken: Option<(i64,)> = sqlx::query_as("SELECT id FROM users WHERE username = ?")
            .bind(&candidate)
            .fetch_optional(&state.db)
            .await?;
        if taken.is_none() {
            return Ok(candidate);
        }
    }
    Ok(format!("user{}", &random_token()[..8].to_lowercase()))
}

pub fn valid_username(username: &str) -> bool {
    (USERNAME_MIN..=USERNAME_MAX).contains(&username.len())
        && username.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.')
}

// ------------------------------------------------------------------ handlers

pub async fn get_me(State(state): State<AppState>, AuthUser(user_id): AuthUser) -> ApiResult<Json<Me>> {
    Ok(Json(me(&state, user_id).await?))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileUpdate {
    display_name: Option<String>,
    username: Option<String>,
}

pub async fn update_me(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Json(update): Json<ProfileUpdate>,
) -> ApiResult<Json<Me>> {
    if let Some(name) = update.display_name {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > DISPLAY_NAME_MAX {
            return Err(ApiError::BadRequest(format!("o nome precisa ter de 1 a {DISPLAY_NAME_MAX} caracteres")));
        }
        sqlx::query("UPDATE users SET display_name = ? WHERE id = ?").bind(name).bind(user_id).execute(&state.db).await?;
    }
    if let Some(username) = update.username {
        let username = username.trim().trim_start_matches('@').to_lowercase();
        if !valid_username(&username) {
            return Err(ApiError::BadRequest(format!(
                "o @usuário precisa ter de {USERNAME_MIN} a {USERNAME_MAX} caracteres: letras minúsculas, números, _ ou ."
            )));
        }
        let result = sqlx::query("UPDATE users SET username = ? WHERE id = ?").bind(&username).bind(user_id).execute(&state.db).await;
        if let Err(sqlx::Error::Database(e)) = &result {
            if e.is_unique_violation() {
                return Err(ApiError::Conflict(format!("@{username} já está em uso")));
            }
        }
        result?;
    }
    Ok(Json(me(&state, user_id).await?))
}

/// Recebe a foto crua no corpo da requisição. Decodifica (o que recusa qualquer
/// coisa que não seja imagem de verdade), recorta quadrado, reduz para 256 px e
/// grava como JPEG novo — sem os metadados do arquivo original.
pub async fn upload_avatar(State(state): State<AppState>, AuthUser(user_id): AuthUser, body: Bytes) -> ApiResult<Json<Me>> {
    if body.len() > AVATAR_MAX_BYTES {
        return Err(ApiError::BadRequest("a foto pode ter no máximo 2 MB".into()));
    }
    let jpeg = tokio::task::spawn_blocking(move || reencode_avatar(&body))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))??;

    let file = format!("{user_id}-{}.jpg", &random_token()[..12]);
    tokio::fs::write(state.config.avatar_dir.join(&file), jpeg).await.map_err(|e| ApiError::Internal(e.to_string()))?;

    let previous: Option<(Option<String>,)> = sqlx::query_as("SELECT avatar_url FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(&state.db)
        .await?;
    let url = format!("{}/avatars/{file}", state.config.public_url);
    sqlx::query("UPDATE users SET avatar_url = ? WHERE id = ?").bind(&url).bind(user_id).execute(&state.db).await?;

    // Apaga a foto anterior enviada (fotos do Discord/Spotify são externas e ficam lá).
    if let Some((Some(old),)) = previous {
        if let Some(old_file) = old.strip_prefix(&format!("{}/avatars/", state.config.public_url)) {
            if !old_file.contains('/') && !old_file.contains("..") {
                let _ = tokio::fs::remove_file(state.config.avatar_dir.join(old_file)).await;
            }
        }
    }
    Ok(Json(me(&state, user_id).await?))
}

pub fn reencode_avatar(bytes: &[u8]) -> ApiResult<Vec<u8>> {
    let format = image::guess_format(bytes).map_err(|_| ApiError::BadRequest("envie uma imagem PNG, JPEG, WebP ou GIF".into()))?;
    if !matches!(format, ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP | ImageFormat::Gif) {
        return Err(ApiError::BadRequest("envie uma imagem PNG, JPEG, WebP ou GIF".into()));
    }
    let img = image::load_from_memory_with_format(bytes, format).map_err(|_| ApiError::BadRequest("a imagem está corrompida".into()))?;
    let side = img.width().min(img.height());
    let square = img.crop_imm((img.width() - side) / 2, (img.height() - side) / 2, side, side);
    let small = square.resize_exact(AVATAR_PX, AVATAR_PX, FilterType::Lanczos3).to_rgb8();
    let mut out = Cursor::new(Vec::new());
    small.write_to(&mut out, ImageFormat::Jpeg).map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(out.into_inner())
}

#[derive(Deserialize)]
pub struct Search {
    q: String,
}

/// Busca por @usuário (prefixo) para adicionar amigos.
pub async fn search(State(state): State<AppState>, AuthUser(user_id): AuthUser, Query(search): Query<Search>) -> ApiResult<Json<Vec<PublicUser>>> {
    let q = search.q.trim().trim_start_matches('@').to_lowercase();
    if q.len() < 2 {
        return Ok(Json(Vec::new()));
    }
    // Escapa os curingas do LIKE para a busca ser literal.
    let pattern = format!("{}%", q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
    let users = sqlx::query_as::<_, PublicUser>(
        "SELECT username, display_name, avatar_url FROM users WHERE username LIKE ? ESCAPE '\\' AND id <> ? ORDER BY length(username) LIMIT 10",
    )
    .bind(pattern)
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(users))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_makes_valid_usernames() {
        assert_eq!(slugify("Ana Clara!"), "ana_clara");
        assert_eq!(slugify("Ção-Ê"), "cao_e");
        assert_eq!(slugify("🎧"), "xxx", "nome sem letras ainda vira algo válido");
        assert!(valid_username(&slugify("216saint")));
        assert!(slugify("um nome muito muito comprido demais").len() <= USERNAME_MAX - 4);
    }

    #[test]
    fn username_rules() {
        assert!(valid_username("ana_clara.2"));
        assert!(!valid_username("ab"), "curto demais");
        assert!(!valid_username("Ana"), "maiúscula não");
        assert!(!valid_username("ana clara"), "espaço não");
    }

    #[test]
    fn avatar_rejects_non_images_and_reencodes_images() {
        assert!(reencode_avatar(b"<script>alert(1)</script>").is_err());

        let mut png = Cursor::new(Vec::new());
        image::RgbImage::new(400, 300).write_to(&mut png, ImageFormat::Png).unwrap();
        let jpeg = reencode_avatar(png.get_ref()).unwrap();
        let out = image::load_from_memory(&jpeg).unwrap();
        assert_eq!((out.width(), out.height()), (AVATAR_PX, AVATAR_PX), "recortada quadrada e reduzida");
        assert_eq!(image::guess_format(&jpeg).unwrap(), ImageFormat::Jpeg);
    }
}

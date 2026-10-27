//! YouTube Music, sem nenhum app registrado no Google Cloud:
//! - login: página oficial do Google numa janela do app (`auth::web_login`);
//! - dados: API interna do site (InnerTube, ver `innertube.rs`) — busca funciona até sem login;
//! - requisições logadas: cookies da sessão + header `Authorization: SAPISIDHASH ...`;
//! - áudio: sidecar `yt-dlp` resolve o stream m4a (ver `audio::ytdlp`).

mod innertube;

use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use tauri::AppHandle;

use super::MusicProvider;
use crate::auth::web_login::{self, WebLogin};
use crate::error::{Error, Result};
use crate::models::{AccountStatus, Playlist, Source, Track};

pub const ORIGIN: &str = "https://music.youtube.com";
/// Playlist automática de "Músicas curtidas" de toda conta.
const LIKED_ID: &str = "LM";

pub const LOGIN: WebLogin = WebLogin {
    label: "login-youtube-music",
    title: "Entrar no YouTube Music",
    start_url: "https://accounts.google.com/ServiceLogin?service=youtube&continue=https%3A%2F%2Fmusic.youtube.com%2F",
    cookie_url: ORIGIN,
    // Cookie que o Google só define com sessão ativa; também é a chave do SAPISIDHASH.
    session_cookie: "SAPISID",
    logout_urls: &[ORIGIN, "https://www.youtube.com", "https://accounts.google.com"],
};

pub struct YoutubeMusicProvider {
    app: AppHandle,
    http: reqwest::Client,
}

impl YoutubeMusicProvider {
    pub fn new(app: AppHandle, http: reqwest::Client) -> Self {
        Self { app, http }
    }

    /// POST num endpoint InnerTube. Com `logged_in`, assina como a conta da pessoa.
    async fn call(&self, endpoint: &str, payload: Value, logged_in: bool) -> Result<Value> {
        let mut request = self
            .http
            .post(format!("{}/{endpoint}?prettyPrint=false", innertube::API_URL))
            .header("Origin", ORIGIN)
            .header("User-Agent", innertube::USER_AGENT)
            .json(&innertube::body(payload));
        if logged_in {
            let (authorization, cookies) = self.auth_headers()?;
            request = request
                .header("Authorization", authorization)
                .header("Cookie", cookies)
                .header("X-Origin", ORIGIN)
                .header("X-Goog-AuthUser", "0");
        }
        Ok(request.send().await?.error_for_status()?.json().await?)
    }

    /// Headers de autenticação para chamadas InnerTube logadas.
    fn auth_headers(&self) -> Result<(String, String)> {
        let sapisid = web_login::session_cookie(&self.app, &LOGIN)?.ok_or(Error::NotConnected(Source::YoutubeMusic))?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        Ok((sapisid_hash(&sapisid, ORIGIN, now), web_login::cookie_header(&self.app, ORIGIN)?))
    }

    fn is_logged_in(&self) -> bool {
        matches!(web_login::session_cookie(&self.app, &LOGIN), Ok(Some(_)))
    }

    /// Cookies da sessão no formato cookies.txt, para o yt-dlp tocar como a pessoa logada.
    /// `None` sem login (o YouTube tende a recusar com "confirm you're not a bot").
    pub fn ytdlp_cookies(&self) -> Option<String> {
        if !self.is_logged_in() {
            return None;
        }
        web_login::netscape_cookies(&self.app, &["https://www.youtube.com", ORIGIN, "https://accounts.google.com"])
            .map_err(|e| log::warn!("não exportei os cookies do YouTube: {e}"))
            .ok()
    }
}

/// Assinatura que o YouTube exige em requisições logadas:
/// `SAPISIDHASH <ts>_<sha1("<ts> <SAPISID> <origin>")>`.
pub fn sapisid_hash(sapisid: &str, origin: &str, unix_secs: u64) -> String {
    let digest = Sha1::digest(format!("{unix_secs} {sapisid} {origin}"));
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("SAPISIDHASH {unix_secs}_{hex}")
}

#[async_trait]
impl MusicProvider for YoutubeMusicProvider {
    fn source(&self) -> Source {
        Source::YoutubeMusic
    }

    async fn account(&self) -> AccountStatus {
        if !self.is_logged_in() {
            return AccountStatus::disconnected(Source::YoutubeMusic);
        }
        let mut status = AccountStatus { connected: true, ..AccountStatus::disconnected(Source::YoutubeMusic) };
        // Nome e foto: o mesmo menu de conta que o site abre ao clicar no avatar.
        if let Ok(menu) = self.call("account/account_menu", json!({}), true).await {
            let header = menu.pointer("/actions/0/openPopupAction/popup/multiPageMenuRenderer/header/activeAccountHeaderRenderer");
            if let Some(header) = header {
                status.display_name = header.pointer("/accountName/runs/0/text").and_then(Value::as_str).map(str::to_owned);
                status.avatar_url = header
                    .pointer("/accountPhoto/thumbnails")
                    .and_then(Value::as_array)
                    .and_then(|t| t.last())
                    .and_then(|t| t["url"].as_str())
                    .map(str::to_owned);
            }
        }
        status
    }

    async fn connect(&self, app: &AppHandle) -> Result<AccountStatus> {
        web_login::login(app, &LOGIN).await?;
        Ok(self.account().await)
    }

    async fn disconnect(&self) -> Result<()> {
        web_login::logout(&self.app, &LOGIN)
    }

    async fn search_tracks(&self, query: &str, limit: u32) -> Result<Vec<Track>> {
        // Busca pública: funciona sem conta. Logado, os resultados levam em conta o gosto da pessoa.
        let payload = json!({ "query": query, "params": innertube::SONGS_FILTER });
        let response = self.call("search", payload, self.is_logged_in()).await?;
        let mut tracks = innertube::parse_tracks(&response);
        tracks.truncate(limit as usize);
        Ok(tracks)
    }

    async fn playlists(&self) -> Result<Vec<Playlist>> {
        if !self.is_logged_in() {
            return Err(Error::NotConnected(Source::YoutubeMusic));
        }
        let response = self.call("browse", json!({ "browseId": "FEmusic_liked_playlists" }), true).await?;
        let mut playlists = innertube::parse_playlists(&response);
        if !playlists.iter().any(|p| p.id == LIKED_ID) {
            playlists.insert(
                0,
                Playlist {
                    id: LIKED_ID.into(),
                    source: Source::YoutubeMusic,
                    name: "Músicas curtidas".into(),
                    owner: None,
                    track_count: None,
                    artwork: None,
                },
            );
        }
        Ok(playlists)
    }

    async fn playlist_tracks(&self, playlist_id: &str) -> Result<Vec<Track>> {
        // TODO: seguir `continuations` para playlists com mais de ~100 faixas.
        let response = self
            .call("browse", json!({ "browseId": format!("VL{playlist_id}") }), self.is_logged_in())
            .await?;
        Ok(innertube::parse_tracks(&response))
    }

    async fn set_liked(&self, track: &Track, liked: bool) -> Result<()> {
        if !self.is_logged_in() {
            return Err(Error::NotConnected(Source::YoutubeMusic));
        }
        // O mesmo "joinha" do site: entra (ou sai) da playlist "Músicas curtidas".
        let endpoint = if liked { "like/like" } else { "like/removelike" };
        self.call(endpoint, json!({ "target": { "videoId": track.id } }), true).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sapisid_hash_matches_reference() {
        // Referência gerada com: node -e 'crypto.createHash("sha1").update("1700000000 abc123SAPISID https://music.youtube.com")'
        assert_eq!(
            sapisid_hash("abc123SAPISID", ORIGIN, 1_700_000_000),
            "SAPISIDHASH 1700000000_9ad4ca9eff164ae164030177bf1dcba01962b387"
        );
    }
}

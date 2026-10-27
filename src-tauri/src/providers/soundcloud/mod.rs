//! SoundCloud, sem app registrado:
//! - login: página oficial numa janela do app; a sessão é o cookie `oauth_token`;
//! - dados: API do próprio site (`api-v2.soundcloud.com`), com o `client_id`
//!   público que o site embute nos seus bundles JavaScript;
//! - requisições logadas: header `Authorization: OAuth <oauth_token>`;
//! - áudio: `media.transcodings` da faixa → MP3 progressivo ou HLS de MP3 (`stream`).

mod api;

pub use api::StreamKind;

use async_trait::async_trait;
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use tauri::AppHandle;
use tokio::sync::Mutex;

use super::MusicProvider;
use crate::audio::fetch;
use crate::auth::web_login::{self, WebLogin};
use crate::error::{Error, Result};
use crate::models::{AccountStatus, Playlist, Source, Track};
use api::{Collection, Like, ScPlaylist, ScTrack, StreamLocation, User};

pub const API_URL: &str = "https://api-v2.soundcloud.com";
const HOME_URL: &str = "https://soundcloud.com";
/// Playlist virtual com as faixas curtidas.
const LIKES_ID: &str = "likes";
/// A API aceita no máximo 50 ids por chamada em `/tracks?ids=`.
const IDS_PER_CALL: usize = 50;

pub const LOGIN: WebLogin = WebLogin {
    label: "login-soundcloud",
    title: "Entrar no SoundCloud",
    start_url: "https://soundcloud.com/signin",
    cookie_url: HOME_URL,
    session_cookie: "oauth_token",
    logout_urls: &[HOME_URL, "https://secure.soundcloud.com"],
};

/// Onde está o áudio de uma faixa e como lê-lo.
#[derive(Debug, Clone)]
pub struct Stream {
    pub url: String,
    pub kind: StreamKind,
}

pub struct SoundcloudProvider {
    app: AppHandle,
    http: reqwest::Client,
    /// `client_id` descoberto na página inicial; renovado quando a API recusa.
    client_id: Mutex<Option<String>>,
}

impl SoundcloudProvider {
    pub fn new(app: AppHandle, http: reqwest::Client) -> Self {
        Self { app, http, client_id: Mutex::new(None) }
    }

    fn oauth_token(&self) -> Option<String> {
        web_login::session_cookie(&self.app, &LOGIN).ok().flatten()
    }

    async fn client_id(&self, refresh: bool) -> Result<String> {
        let mut cached = self.client_id.lock().await;
        if refresh || cached.is_none() {
            *cached = Some(self.fetch_client_id().await?);
        }
        Ok(cached.clone().unwrap_or_default())
    }

    /// GET na API com `client_id` (+ token da pessoa, se logada).
    /// Se o `client_id` expirou (401/403), descobre um novo e tenta mais uma vez.
    async fn get<T: DeserializeOwned>(&self, path_or_url: &str, query: &[(&str, &str)]) -> Result<T> {
        let url = if path_or_url.starts_with("http") { path_or_url.to_owned() } else { format!("{API_URL}{path_or_url}") };
        for attempt in 0..2 {
            let client_id = self.client_id(attempt > 0).await?;
            let mut request = self.http.get(&url).query(query).query(&[("client_id", client_id.as_str())]);
            if let Some(token) = self.oauth_token() {
                request = request.header("Authorization", format!("OAuth {token}"));
            }
            let response = request.send().await?;
            if matches!(response.status(), StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) && attempt == 0 {
                continue;
            }
            return Ok(response.error_for_status()?.json().await?);
        }
        unreachable!("o laço sempre retorna na segunda tentativa")
    }

    async fn me(&self) -> Result<User> {
        if self.oauth_token().is_none() {
            return Err(Error::NotConnected(Source::Soundcloud));
        }
        self.get("/me", &[]).await
    }

    /// Descobre o `client_id` público lendo os bundles JS da página inicial.
    pub async fn fetch_client_id(&self) -> Result<String> {
        let html = self.http.get(HOME_URL).send().await?.text().await?;
        // O client_id costuma estar num dos últimos bundles.
        for url in script_urls(&html).into_iter().rev() {
            let js = self.http.get(url).send().await?.text().await?;
            if let Some(id) = client_id_in(&js) {
                return Ok(id.to_owned());
            }
        }
        Err(Error::Other("client_id do SoundCloud não encontrado".into()))
    }

    /// Completa faixas "stub" (só id) pedindo os detalhes em lotes.
    async fn hydrate(&self, tracks: Vec<ScTrack>) -> Result<Vec<Track>> {
        let missing: Vec<String> = tracks.iter().filter(|t| t.title.is_none()).map(|t| t.id.to_string()).collect();
        let mut full = Vec::new();
        for chunk in missing.chunks(IDS_PER_CALL) {
            let ids = chunk.join(",");
            full.extend(self.get::<Vec<ScTrack>>("/tracks", &[("ids", &ids)]).await?);
        }
        Ok(tracks
            .into_iter()
            .map(|t| if t.title.is_some() { t } else { full.iter().find(|f| f.id == t.id).cloned().unwrap_or(t) })
            .filter(ScTrack::is_playable)
            .filter_map(ScTrack::into_track)
            .collect())
    }

    /// Resolve onde está o áudio de uma faixa (usado pelo player).
    pub async fn stream(&self, track_id: &str) -> Result<Stream> {
        let track: ScTrack = self.get(&format!("/tracks/{track_id}"), &[]).await?;
        let (transcoding, kind) = track
            .best_transcoding()
            .ok_or_else(|| Error::Other("o SoundCloud não oferece um formato tocável para esta faixa".into()))?;
        let auth = track.track_authorization.clone().unwrap_or_default();
        let location: StreamLocation = self.get(&transcoding.url, &[("track_authorization", &auth)]).await?;
        Ok(Stream { url: location.url, kind })
    }

    /// Os arquivos de uma playlist HLS, na ordem (cabeçalho + pedaços), para o player baixar
    /// em paralelo e já ir tocando (`audio::fetch::stream_parts`).
    pub async fn hls_parts(&self, m3u8_url: &str) -> Result<Vec<fetch::Part>> {
        let playlist = self.http.get(m3u8_url).send().await?.error_for_status()?.text().await?;
        let parts: Vec<_> = api::hls_parts(&playlist).into_iter().map(fetch::Part::whole).collect();
        if parts.is_empty() {
            return Err(Error::Other("playlist do SoundCloud vazia".into()));
        }
        Ok(parts)
    }
}

/// URLs dos bundles JavaScript referenciados no HTML da página inicial.
pub fn script_urls(html: &str) -> Vec<&str> {
    html.match_indices("src=\"")
        .filter_map(|(i, m)| {
            let rest = &html[i + m.len()..];
            let url = &rest[..rest.find('"')?];
            (url.starts_with("https://a-v2.sndcdn.com/assets/") && url.ends_with(".js")).then_some(url)
        })
        .collect()
}

/// Extrai `client_id:"<32 caracteres>"` de um bundle.
pub fn client_id_in(js: &str) -> Option<&str> {
    js.match_indices("client_id:\"").find_map(|(i, m)| {
        let id = js.get(i + m.len()..i + m.len() + 32)?;
        id.chars().all(|c| c.is_ascii_alphanumeric()).then_some(id)
    })
}

#[async_trait]
impl MusicProvider for SoundcloudProvider {
    fn source(&self) -> Source {
        Source::Soundcloud
    }

    async fn account(&self) -> AccountStatus {
        match self.me().await {
            Ok(me) => AccountStatus {
                notice: None,
                source: Source::Soundcloud,
                connected: true,
                display_name: Some(me.username),
                avatar_url: me.avatar_url,
                premium: None,
            },
            Err(Error::NotConnected(_)) => AccountStatus::disconnected(Source::Soundcloud),
            // Logado, mas a API falhou agora: mostra conectado sem detalhes.
            Err(_) => AccountStatus { connected: true, ..AccountStatus::disconnected(Source::Soundcloud) },
        }
    }

    async fn connect(&self, app: &AppHandle) -> Result<AccountStatus> {
        web_login::login(app, &LOGIN).await?;
        Ok(self.account().await)
    }

    async fn disconnect(&self) -> Result<()> {
        web_login::logout(&self.app, &LOGIN)
    }

    async fn search_tracks(&self, query: &str, limit: u32) -> Result<Vec<Track>> {
        // Pedimos o dobro: parte dos resultados é prévia (Go+) ou bloqueada e sai no filtro.
        let fetch = (limit * 2).to_string();
        let found: Collection<ScTrack> = self.get("/search/tracks", &[("q", query), ("limit", &fetch)]).await?;
        Ok(found
            .collection
            .into_iter()
            .filter(ScTrack::is_playable)
            .filter_map(ScTrack::into_track)
            .take(limit as usize)
            .collect())
    }

    async fn playlists(&self) -> Result<Vec<Playlist>> {
        let me = self.me().await?;
        let mine: Collection<ScPlaylist> = self.get(&format!("/users/{}/playlists", me.id), &[("limit", "50")]).await?;

        let mut playlists = vec![Playlist {
            id: LIKES_ID.into(),
            source: Source::Soundcloud,
            name: "Curtidas".into(),
            owner: Some(me.username),
            track_count: me.likes_count,
            artwork: None,
        }];
        playlists.extend(mine.collection.into_iter().map(ScPlaylist::into_playlist));
        Ok(playlists)
    }

    async fn set_liked(&self, track: &Track, liked: bool) -> Result<()> {
        let me = self.me().await?;
        let url = format!("{API_URL}/users/{}/track_likes/{}", me.id, track.id);
        let token = self.oauth_token().ok_or(Error::NotConnected(Source::Soundcloud))?;
        let client_id = self.client_id(false).await?;
        let request = if liked { self.http.put(&url) } else { self.http.delete(&url) };
        request
            .query(&[("client_id", client_id.as_str())])
            .header("Authorization", format!("OAuth {token}"))
            .header(reqwest::header::CONTENT_LENGTH, 0)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn playlist_tracks(&self, playlist_id: &str) -> Result<Vec<Track>> {
        if playlist_id == LIKES_ID {
            let me = self.me().await?;
            let likes: Collection<Like> = self.get(&format!("/users/{}/track_likes", me.id), &[("limit", "200")]).await?;
            return self.hydrate(likes.collection.into_iter().filter_map(|l| l.track).collect()).await;
        }
        let playlist: ScPlaylist = self.get(&format!("/playlists/{playlist_id}"), &[]).await?;
        self.hydrate(playlist.tracks).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_bundle_urls_in_homepage() {
        let html = r#"<script crossorigin src="https://a-v2.sndcdn.com/assets/0-abc.js"></script>
            <script src="https://example.com/x.js"></script>
            <script crossorigin src="https://a-v2.sndcdn.com/assets/49-def.js"></script>"#;
        assert_eq!(
            script_urls(html),
            ["https://a-v2.sndcdn.com/assets/0-abc.js", "https://a-v2.sndcdn.com/assets/49-def.js"]
        );
    }

    #[test]
    fn extracts_client_id_from_bundle() {
        let js = r#"x={a:1},y={client_id:"AbCdEfGhIjKlMnOpQrStUvWxYz012345",env:"production"}"#;
        assert_eq!(client_id_in(js), Some("AbCdEfGhIjKlMnOpQrStUvWxYz012345"));
        assert_eq!(client_id_in(r#"client_id:"curto""#), None);
    }
}

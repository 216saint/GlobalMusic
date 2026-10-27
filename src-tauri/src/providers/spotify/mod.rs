//! Spotify: Web API para busca/biblioteca — funciona com contas Free e Premium.
//! Login: OAuth 2.0 + PKCE pelo navegador do sistema (ver docs/conceitos/oauth-pkce.md).
//! Reprodução: librespot quando a conta é Premium; senão o player usa o
//! fallback do YouTube (`player::resolver`).
//!
//! **Dois tokens quando há client ID próprio.** O Spotify só aceita tocar (o librespot pede os
//! metadados da faixa em `extended-metadata`) com tokens do client ID do app oficial; com um
//! client ID do dashboard ele responde 400 e a faixa caía no YouTube (5–9 s em vez de ~1 s).
//! Então, com client ID próprio, guardamos dois: o da Web API (cota só da pessoa) e o de
//! reprodução (client oficial, `PLAYBACK_KEY`), que não gasta a cota da Web API.
//!
//! - `client_id`: auto-atualização do client ID (ver ADR 0003);
//! - `http`: requisições com tratamento de 429 (Retry-After).

mod api;
mod client_id;
mod http;

pub use client_id::ClientId;

use serde::de::DeserializeOwned;

use api::{Item, Page, SearchResponse, SpPlaylist};

/// Playlist virtual com as "Músicas Curtidas".
const LIKED_ID: &str = "liked";
/// Limites de paginação: o bastante para bibliotecas grandes sem gastar a cota à toa.
const MAX_PLAYLIST_PAGES: usize = 4; // 4 × 50 playlists
const MAX_TRACK_PAGES: usize = 10; // 10 × 100 (playlist) ou 10 × 50 (curtidas)
/// Máximo de ids por chamada de `GET /tracks`.
const SEVERAL_TRACKS: usize = 50;

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::Deserialize;
use tauri::AppHandle;
use tokio::sync::Mutex;

use super::MusicProvider;
use crate::auth::oauth::{authorize_via_browser, Pkce};
use crate::auth::{TokenStore, Tokens};
use crate::config::{spotify, OAUTH_PORT};
use crate::error::{Error, Result};
use crate::models::{AccountStatus, Playlist, Source, Track};

/// Perfil (Free/Premium), playlists, curtidas e `streaming` (exigido pelo librespot).
pub const SCOPES: &[&str] = &[
    "user-read-private",
    "user-read-email",
    "playlist-read-private",
    "playlist-read-collaborative",
    "user-library-read",
    // Curtir/descurtir faixas pelo GlobalMusic (contas conectadas antes da 0.1.3 precisam reconectar).
    "user-library-modify",
    "streaming",
];

/// Nome do token de reprodução no cofre (só existe quando há client ID próprio).
const PLAYBACK_KEY: &str = "spotify_playback";
/// O token de reprodução só precisa tocar e saber o plano.
const PLAYBACK_SCOPES: &[&str] = &["streaming", "user-read-private"];
const RECONNECT_FOR_PLAYBACK: &str = "Reconecte o Spotify para tocar direto pelo Spotify (bem mais rápido que pelo YouTube).";

/// Renova o token um pouco antes de expirar, para não falhar no meio de uma chamada.
const REFRESH_MARGIN_SECS: u64 = 60;
/// Nome/foto/plano mudam raramente: reaproveitar evita gastar a cota a cada abertura do perfil.
const PROFILE_TTL: Duration = Duration::from_secs(15 * 60);

pub struct SpotifyProvider {
    store: TokenStore,
    http: reqwest::Client,
    client_id: Arc<ClientId>,
    /// Último `/me` bem-sucedido e quando foi lido.
    profile: Mutex<Option<(Instant, AccountStatus)>>,
}

impl SpotifyProvider {
    pub fn new(store: TokenStore, http: reqwest::Client, client_id: Arc<ClientId>) -> Self {
        Self { store, http, client_id, profile: Mutex::new(None) }
    }

    pub fn custom_client_id(&self) -> Option<String> {
        self.client_id.custom()
    }

    /// Troca (ou remove) o client ID próprio. Tokens emitidos pelo client anterior
    /// não servem para o novo, então a conta é desconectada e precisa conectar de novo.
    pub async fn set_custom_client_id(&self, id: Option<&str>) -> Result<()> {
        if self.client_id.set_custom(id)? {
            self.store.clear(Source::Spotify)?;
            self.store.clear_as(PLAYBACK_KEY)?;
            *self.profile.lock().await = None;
        }
        Ok(())
    }

    /// GET autenticado na Web API (caminho relativo ou URL completa de paginação).
    async fn get_json<T: DeserializeOwned>(&self, path_or_url: &str) -> Result<T> {
        let token = self.access_token().await?;
        self.get_json_with(&token, path_or_url).await
    }

    async fn get_json_with<T: DeserializeOwned>(&self, token: &str, path_or_url: &str) -> Result<T> {
        let url = if path_or_url.starts_with("http") {
            path_or_url.to_owned()
        } else {
            format!("{}{path_or_url}", spotify::API_URL)
        };
        let response = http::send(|| self.http.get(&url).bearer_auth(token)).await?;
        Ok(response.error_for_status()?.json().await?)
    }

    /// Segue o campo `next` até `max_pages` páginas.
    async fn all_pages<T: DeserializeOwned>(&self, first: &str, max_pages: usize) -> Result<Vec<T>> {
        let token = self.access_token().await?;
        self.all_pages_with(&token, first, max_pages).await
    }

    async fn all_pages_with<T: DeserializeOwned>(&self, token: &str, first: &str, max_pages: usize) -> Result<Vec<T>> {
        let mut items = Vec::new();
        let mut next = Some(first.to_owned());
        for _ in 0..max_pages {
            let Some(url) = next.take() else { break };
            let page: Page<T> = self.get_json_with(token, &url).await?;
            items.extend(page.items);
            next = page.next;
        }
        Ok(items)
    }

    /// Faixas de uma playlist de **outra pessoa** (importada por link).
    ///
    /// Um client ID próprio fica em Development Mode, e lá o Spotify só entrega as faixas de
    /// playlists que a pessoa criou ou colabora: as dos outros vêm vazias. O token de reprodução
    /// (client oficial) não tem essa restrição, então é ele que lê. Sem client ID próprio, o token
    /// de sempre já é do client oficial.
    pub async fn shared_playlist_tracks(&self, playlist_id: &str) -> Result<Vec<Track>> {
        if self.client_id.custom().is_none() {
            return self.playlist_tracks(playlist_id).await;
        }
        match self.valid_token(PLAYBACK_KEY, self.client_id.official()).await {
            Ok((token, _)) => self.playlist_items(&token, playlist_id).await,
            // Sem o token de reprodução: o da pessoa ainda serve se a playlist for dela.
            Err(Error::NotConnected(_)) => match self.playlist_tracks(playlist_id).await {
                Ok(tracks) if !tracks.is_empty() => Ok(tracks),
                _ => Err(Error::Other(
                    "reconecte o Spotify no Perfil para abrir playlists de outras pessoas".into(),
                )),
            },
            Err(e) => Err(e),
        }
    }

    /// Faixas de uma playlist (não as curtidas), lidas com `token`. Se o Spotify recusar (403), tenta
    /// na ordem: o token do client oficial (quando a pessoa usa Client ID próprio, que fica em
    /// Development Mode e não lê playlists dos outros) e a página pública da playlist.
    async fn playlist_items(&self, token: &str, playlist_id: &str) -> Result<Vec<Track>> {
        let refused = match self.read_items(token, playlist_id).await {
            Ok(tracks) => return Ok(tracks),
            Err(e) if http_status(&e) == Some(403) => e,
            Err(e) => return Err(e),
        };
        let official = self.official_token().await;
        if let Some(official) = official.as_deref().filter(|o| *o != token) {
            match self.read_items(official, playlist_id).await {
                Ok(tracks) => return Ok(tracks),
                Err(e) => log::info!("Spotify: client oficial também recusou a playlist {playlist_id} ({e})"),
            }
        }
        // Desde nov/2024 a Web API não entrega a apps de terceiros as playlists feitas pelo próprio
        // Spotify ("This Is", Daily Mix…). A página pública do player incorporável entrega.
        log::info!("Spotify: playlist {playlist_id} restrita na Web API ({refused}); lendo a página pública");
        self.public_playlist_tracks(official.as_deref().unwrap_or(token), playlist_id).await
    }

    /// `/items` (o endpoint de 2026) e, se ele não existir para este cliente, o antigo `/tracks`.
    async fn read_items(&self, token: &str, playlist_id: &str) -> Result<Vec<Track>> {
        let items_url = format!("/playlists/{playlist_id}/items?limit=100");
        let items: Vec<Item> = match self.all_pages_with(token, &items_url, MAX_TRACK_PAGES).await {
            Ok(items) => items,
            Err(e) if matches!(http_status(&e), Some(403 | 404)) => {
                log::info!("Spotify: /items recusado ({e}); tentando /tracks");
                let tracks_url = format!("/playlists/{playlist_id}/tracks?limit=100");
                self.all_pages_with(token, &tracks_url, MAX_TRACK_PAGES).await?
            }
            Err(e) => return Err(e),
        };
        Ok(items.into_iter().filter_map(|i| i.track?.into_track()).collect())
    }

    /// Token do client oficial, só quando a pessoa usa Client ID próprio (senão o de sempre já é ele).
    async fn official_token(&self) -> Option<String> {
        self.client_id.custom()?;
        self.valid_token(PLAYBACK_KEY, self.client_id.official()).await.ok().map(|(token, _)| token)
    }

    /// Faixas pela página pública da playlist, completadas pela Web API (capa, álbum, ISRC) em lotes
    /// de 50. Se a Web API recusar, as capas vêm da prévia pública de cada música (`imports`).
    async fn public_playlist_tracks(&self, token: &str, playlist_id: &str) -> Result<Vec<Track>> {
        let tracks = crate::imports::spotify_embed_tracks(&self.http, playlist_id).await.map_err(|e| {
            log::warn!("Spotify: página pública da playlist {playlist_id} falhou: {e}");
            Error::Other("o Spotify não liberou as faixas desta playlist agora. Tente de novo em instantes".into())
        })?;
        let mut detailed = Vec::with_capacity(tracks.len());
        for chunk in tracks.chunks(SEVERAL_TRACKS) {
            let ids = chunk.iter().map(|t| t.id.as_str()).collect::<Vec<_>>().join(",");
            match self.get_json_with::<api::SeveralTracks>(token, &format!("/tracks?ids={ids}")).await {
                Ok(found) => detailed.extend(found.tracks.into_iter().flatten().filter_map(api::SpTrack::into_track)),
                Err(e) => {
                    log::info!("Spotify: sem os detalhes das faixas ({e}); capas pela prévia pública");
                    return Ok(crate::imports::spotify_artwork(&self.http, tracks).await);
                }
            }
        }
        Ok(if detailed.len() == tracks.len() { detailed } else { tracks })
    }

    /// Access token da Web API, renovando com o refresh token se precisar.
    pub async fn access_token(&self) -> Result<String> {
        Ok(self.valid_token(Source::Spotify.key(), self.client_id.get()).await?.0)
    }

    /// Token e client ID para **tocar** (librespot). Sem client ID próprio, é o mesmo da Web API.
    pub async fn playback_token(&self) -> Result<(String, String)> {
        if self.client_id.custom().is_none() {
            return self.valid_token(Source::Spotify.key(), self.client_id.get()).await;
        }
        match self.valid_token(PLAYBACK_KEY, self.client_id.official()).await {
            Err(Error::NotConnected(_)) => Err(Error::Other(RECONNECT_FOR_PLAYBACK.into())),
            other => other,
        }
    }

    /// Token guardado em `key`, renovado se perto de expirar. Devolve (token, client ID que o emitiu).
    async fn valid_token(&self, key: &str, default_client: String) -> Result<(String, String)> {
        let tokens = self.store.load_as(key)?.ok_or(Error::NotConnected(Source::Spotify))?;
        // A renovação precisa do mesmo client ID que emitiu o token.
        let client_id = tokens.client_id.clone().unwrap_or(default_client);
        if !needs_refresh(tokens.expires_at, now()) {
            return Ok((tokens.access_token, client_id));
        }
        let Some(refresh) = tokens.refresh_token.clone() else {
            return Err(Error::NotConnected(Source::Spotify));
        };
        let renewed = self
            .request_token(&[("grant_type", "refresh_token"), ("refresh_token", &refresh), ("client_id", &client_id)])
            .await;
        match renewed {
            Ok(response) => {
                let fresh = response.into_tokens(Some(refresh), &client_id, now());
                self.store.save_as(key, &fresh)?;
                Ok((fresh.access_token, client_id))
            }
            // Token revogado ou client ID trocado pelo Spotify: só um novo login resolve.
            Err(Error::OAuth(_)) => {
                self.store.clear_as(key)?;
                Err(Error::NotConnected(Source::Spotify))
            }
            Err(e) => Err(e),
        }
    }

    /// Login pelo navegador (PKCE) com `client_id` e `scopes`; devolve os tokens.
    async fn authorize(&self, app: &AppHandle, client_id: &str, scopes: &[&str]) -> Result<Tokens> {
        let pkce = Pkce::generate();
        let code = authorize_via_browser(app, &authorize_url(&pkce, client_id, scopes), OAUTH_PORT, &pkce.state).await?;
        let redirect = spotify::redirect_uri();
        Ok(self
            .request_token(&[
                ("grant_type", "authorization_code"),
                ("code", &code),
                ("redirect_uri", &redirect),
                ("client_id", client_id),
                ("code_verifier", &pkce.verifier),
            ])
            .await?
            .into_tokens(None, client_id, now()))
    }

    /// Com client ID próprio e conta Premium, falta o token de reprodução?
    fn needs_playback_login(&self, premium: Option<bool>) -> bool {
        premium == Some(true)
            && self.client_id.custom().is_some()
            && matches!(self.store.load_as(PLAYBACK_KEY), Ok(None))
    }

    async fn request_token(&self, form: &[(&str, &str)]) -> Result<TokenResponse> {
        let response = http::send(|| self.http.post(spotify::TOKEN_URL).form(form)).await?;
        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(Error::OAuth(format!("Spotify recusou o token: {body}")));
        }
        Ok(response.json().await?)
    }

    async fn me(&self) -> Result<Me> {
        let token = self.access_token().await?;
        let url = format!("{}/me", spotify::API_URL);
        let response = http::send(|| self.http.get(&url).bearer_auth(&token)).await?;
        Ok(response.error_for_status()?.json().await?)
    }
}

pub fn authorize_url(pkce: &Pkce, client_id: &str, scopes: &[&str]) -> String {
    let scope = scopes.join(" ");
    let redirect = spotify::redirect_uri();
    let params = [
        ("client_id", client_id),
        ("response_type", "code"),
        ("redirect_uri", redirect.as_str()),
        ("code_challenge_method", "S256"),
        ("code_challenge", pkce.challenge.as_str()),
        ("scope", scope.as_str()),
        ("state", pkce.state.as_str()),
    ];
    url::Url::parse_with_params(spotify::AUTHORIZE_URL, params)
        .expect("AUTHORIZE_URL é uma URL válida")
        .into()
}

/// Código HTTP de um erro de rede, se houver (ex.: 403).
fn http_status(error: &Error) -> Option<u16> {
    match error {
        Error::Http(e) => e.status().map(|s| s.as_u16()),
        _ => None,
    }
}

fn needs_refresh(expires_at: Option<u64>, now: u64) -> bool {
    expires_at.is_some_and(|at| now + REFRESH_MARGIN_SECS >= at)
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
    refresh_token: Option<String>,
}

impl TokenResponse {
    /// Na renovação o Spotify pode omitir o refresh token: aí mantemos o anterior.
    fn into_tokens(self, previous_refresh: Option<String>, client_id: &str, now: u64) -> Tokens {
        Tokens {
            access_token: self.access_token,
            refresh_token: self.refresh_token.or(previous_refresh),
            expires_at: Some(now + self.expires_in),
            client_id: Some(client_id.to_owned()),
        }
    }
}

#[derive(Debug, Deserialize)]
struct Me {
    display_name: Option<String>,
    #[serde(default)]
    images: Vec<Image>,
    /// "premium", "free" ou "open".
    product: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Image {
    url: String,
}

impl Me {
    fn into_account(self) -> AccountStatus {
        AccountStatus {
            source: Source::Spotify,
            connected: true,
            display_name: self.display_name,
            avatar_url: self.images.into_iter().next().map(|i| i.url),
            premium: self.product.map(|p| p == "premium"),
            notice: None,
        }
    }
}

#[async_trait]
impl MusicProvider for SpotifyProvider {
    fn source(&self) -> Source {
        Source::Spotify
    }

    async fn account(&self) -> AccountStatus {
        let mut profile = self.profile.lock().await;
        if let Some((at, cached)) = profile.as_ref() {
            if at.elapsed() < PROFILE_TTL {
                return cached.clone();
            }
        }
        match self.me().await {
            Ok(me) => {
                let mut account = me.into_account();
                if self.needs_playback_login(account.premium) {
                    account.notice = Some(RECONNECT_FOR_PLAYBACK.into());
                }
                *profile = Some((Instant::now(), account.clone()));
                account
            }
            // Logado, mas sem rede ou limitado agora: usa o último perfil conhecido
            // (mesmo vencido) ou mostra conectado sem detalhes.
            Err(Error::Http(_) | Error::RateLimited(_)) => profile.as_ref().map_or_else(
                || AccountStatus { connected: true, ..AccountStatus::disconnected(Source::Spotify) },
                |(_, cached)| cached.clone(),
            ),
            Err(_) => {
                *profile = None;
                AccountStatus::disconnected(Source::Spotify)
            }
        }
    }

    async fn connect(&self, app: &AppHandle) -> Result<AccountStatus> {
        // Garante o client ID mais recente antes de abrir o navegador (no máximo 1x por dia).
        let _ = self.client_id.refresh_if_stale().await;
        let tokens = self.authorize(app, &self.client_id.get(), SCOPES).await?;
        self.store.save(Source::Spotify, &tokens)?;
        // Client ID próprio: um segundo login, com o client oficial, só para tocar. O navegador
        // já está logado no Spotify, então costuma ser só um "Concordo" (e só na primeira vez).
        if self.client_id.custom().is_some() {
            match self.authorize(app, &self.client_id.official(), PLAYBACK_SCOPES).await {
                Ok(playback) => self.store.save_as(PLAYBACK_KEY, &playback)?,
                // Sem ele, as faixas ainda tocam (pelo YouTube); o Perfil avisa para reconectar.
                Err(e) => log::warn!("Spotify: login de reprodução falhou: {e}"),
            }
        }
        // Com o token salvo, o login deu certo. Se o perfil não carregar agora (ex.: 429),
        // a conta aparece conectada mesmo assim e os detalhes chegam na próxima leitura.
        *self.profile.lock().await = None;
        Ok(self.account().await)
    }

    async fn disconnect(&self) -> Result<()> {
        *self.profile.lock().await = None;
        self.store.clear_as(PLAYBACK_KEY)?;
        self.store.clear(Source::Spotify)
    }

    async fn search_tracks(&self, query: &str, limit: u32) -> Result<Vec<Track>> {
        let mut url = url::Url::parse(&format!("{}/search", spotify::API_URL)).expect("URL fixa válida");
        url.query_pairs_mut()
            .append_pair("q", query)
            .append_pair("type", "track")
            .append_pair("limit", &limit.min(50).to_string());
        let response: SearchResponse = self.get_json(url.as_str()).await?;
        Ok(response.tracks.items.into_iter().filter_map(api::SpTrack::into_track).collect())
    }

    async fn playlists(&self) -> Result<Vec<Playlist>> {
        // TODO: cache local — a biblioteca muda pouco e cada chamada gasta a cota compartilhada.
        let liked: Page<Item> = self.get_json("/me/tracks?limit=1").await?;
        let mut playlists = vec![Playlist {
            id: LIKED_ID.into(),
            source: Source::Spotify,
            name: "Músicas Curtidas".into(),
            owner: None,
            track_count: liked.total,
            artwork: None,
            url: None,
        }];
        let mine: Vec<SpPlaylist> = self.all_pages("/me/playlists?limit=50", MAX_PLAYLIST_PAGES).await?;
        // As automáticas do Spotify ("This Is", Daily Mix…) ficam de fora: a pessoa não as montou, e
        // a Web API nem entrega as faixas delas.
        playlists.extend(mine.into_iter().filter(|p| !p.by_spotify()).map(SpPlaylist::into_playlist));
        Ok(playlists)
    }

    async fn playlist_tracks(&self, playlist_id: &str) -> Result<Vec<Track>> {
        let token = self.access_token().await?;
        if playlist_id != LIKED_ID {
            return self.playlist_items(&token, playlist_id).await;
        }
        let items: Vec<Item> = self.all_pages_with(&token, "/me/tracks?limit=50", MAX_TRACK_PAGES).await?;
        Ok(items.into_iter().filter_map(|i| i.track?.into_track()).collect())
    }

    async fn set_liked(&self, track: &Track, liked: bool) -> Result<()> {
        let token = self.access_token().await?;
        let url = format!("{}/me/tracks?ids={}", spotify::API_URL, track.id);
        let response = http::send(|| {
            let request = if liked { self.http.put(&url) } else { self.http.delete(&url) };
            request.bearer_auth(&token).header(reqwest::header::CONTENT_LENGTH, 0)
        })
        .await?;
        if response.status() == reqwest::StatusCode::FORBIDDEN {
            // O token antigo não tem o escopo `user-library-modify` (adicionado na 0.1.3).
            return Err(Error::Other("reconecte o Spotify no Perfil para liberar as curtidas".into()));
        }
        response.error_for_status()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorize_url_carries_pkce_and_loopback_redirect() {
        let pkce = Pkce::generate();
        let url = url::Url::parse(&authorize_url(&pkce, spotify::CLIENT_ID, SCOPES)).unwrap();
        let get = |k: &str| url.query_pairs().find(|(key, _)| key == k).map(|(_, v)| v.into_owned());

        assert_eq!(get("client_id").as_deref(), Some(spotify::CLIENT_ID));
        assert_eq!(get("code_challenge_method").as_deref(), Some("S256"));
        assert_eq!(get("code_challenge"), Some(pkce.challenge));
        assert_eq!(get("state"), Some(pkce.state));
        assert_eq!(get("redirect_uri").as_deref(), Some("http://127.0.0.1:47823/login"));
        assert!(get("scope").unwrap().split(' ').any(|s| s == "streaming"));
    }

    #[test]
    fn refreshes_shortly_before_expiry() {
        assert!(!needs_refresh(Some(1_000), 900));
        assert!(needs_refresh(Some(1_000), 950), "dentro da margem de 60s");
        assert!(needs_refresh(Some(1_000), 2_000));
        assert!(!needs_refresh(None, 2_000), "sem expiração conhecida, usa o token");
    }

    #[test]
    fn refresh_keeps_previous_refresh_token_and_records_client() {
        let response = TokenResponse { access_token: "new".into(), expires_in: 3600, refresh_token: None };
        let tokens = response.into_tokens(Some("old-refresh".into()), "abc", 100);
        assert_eq!(tokens.refresh_token.as_deref(), Some("old-refresh"));
        assert_eq!(tokens.expires_at, Some(3_700));
        assert_eq!(tokens.client_id.as_deref(), Some("abc"));
    }

    #[test]
    fn me_maps_plan_to_premium_flag() {
        let me: Me = serde_json::from_str(r#"{"display_name":"Ana","images":[{"url":"https://i/a.jpg"}],"product":"free"}"#).unwrap();
        let account = me.into_account();
        assert_eq!(account.premium, Some(false));
        assert_eq!(account.display_name.as_deref(), Some("Ana"));
        assert_eq!(account.avatar_url.as_deref(), Some("https://i/a.jpg"));
    }
}

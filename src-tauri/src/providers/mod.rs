//! Contrato comum das fontes de música. Cada provedor traduz a API externa
//! para os modelos de `crate::models`; o resto do app nunca fala com a API direto.

mod soundcloud;
mod spotify;
mod youtube_music;

use std::sync::Arc;

use async_trait::async_trait;
use tauri::{AppHandle, Manager};

pub use soundcloud::{SoundcloudProvider, StreamKind as SoundcloudStreamKind};
pub use spotify::{ClientId, SpotifyProvider};
pub use youtube_music::YoutubeMusicProvider;

use crate::auth::TokenStore;
use crate::error::Result;
use crate::models::{AccountStatus, Playlist, Source, Track};

#[async_trait]
pub trait MusicProvider: Send + Sync {
    fn source(&self) -> Source;

    async fn account(&self) -> AccountStatus;

    /// Abre o navegador no consentimento OAuth e guarda o token no cofre do SO.
    async fn connect(&self, app: &AppHandle) -> Result<AccountStatus>;

    async fn disconnect(&self) -> Result<()>;

    async fn search_tracks(&self, query: &str, limit: u32) -> Result<Vec<Track>>;

    /// Playlists da biblioteca do usuário (exige conta conectada).
    async fn playlists(&self) -> Result<Vec<Playlist>>;

    async fn playlist_tracks(&self, playlist_id: &str) -> Result<Vec<Track>>;

    /// Curte (ou descurte) a faixa **na conta da pessoa** no serviço de origem.
    async fn set_liked(&self, track: &Track, liked: bool) -> Result<()>;
}

/// Os três provedores, de dois jeitos:
/// - `all`: lista genérica para busca/biblioteca federadas (só usa o trait);
/// - campos tipados: para o player, que precisa de métodos específicos
///   (token do Spotify para o librespot, stream do SoundCloud...).
#[derive(Clone)]
pub struct Providers {
    pub spotify: Arc<SpotifyProvider>,
    pub youtube: Arc<YoutubeMusicProvider>,
    pub soundcloud: Arc<SoundcloudProvider>,
    pub all: Vec<Arc<dyn MusicProvider>>,
}

impl Providers {
    pub fn new(app: &AppHandle, store: &TokenStore, http: &reqwest::Client) -> Self {
        let client_id = Arc::new(ClientId::load(http.clone(), app.path().app_data_dir().ok()));

        // Checagem diária do client ID em segundo plano, sem atrasar a abertura do app.
        let background = client_id.clone();
        tauri::async_runtime::spawn(async move {
            let _ = background.refresh_if_stale().await;
        });

        let library_cache = app.path().app_data_dir().ok().map(|d| d.join("spotify-library"));
        let spotify = Arc::new(SpotifyProvider::new(store.clone(), http.clone(), client_id, library_cache));
        let youtube = Arc::new(YoutubeMusicProvider::new(app.clone(), http.clone()));
        let soundcloud = Arc::new(SoundcloudProvider::new(app.clone(), http.clone()));
        let all: Vec<Arc<dyn MusicProvider>> = vec![spotify.clone(), youtube.clone(), soundcloud.clone()];
        Self { spotify, youtube, soundcloud, all }
    }

    pub fn get(&self, source: Source) -> &Arc<dyn MusicProvider> {
        self.all
            .iter()
            .find(|p| p.source() == source)
            .expect("todo Source tem um provedor registrado em Providers::new")
    }
}

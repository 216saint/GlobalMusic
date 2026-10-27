use std::sync::Arc;

use tauri::{AppHandle, Manager};

use crate::favorites::Favorites;
use tokio::sync::Mutex;

use crate::audio::{Audio, EventSink};
use crate::auth::TokenStore;
use crate::config::DISCORD_APP_ID;
use crate::player::Player;
use crate::presence::Presence;
use crate::providers::Providers;
use crate::imports::Imports;
use crate::local_playlists::LocalPlaylists;
use crate::lyrics::LyricsClient;
use crate::social::Social;
use crate::sponsorblock::SponsorBlock;

pub struct AppState {
    pub providers: Providers,
    pub player: Mutex<Player>,
    pub presence: Presence,
    pub audio: Arc<Audio>,
    pub favorites: Favorites,
    pub social: Social,
    pub lyrics: LyricsClient,
    pub sponsor: SponsorBlock,
    /// Playlists importadas por link.
    pub imports: Imports,
    /// Playlists criadas no GlobalMusic.
    pub local_playlists: LocalPlaylists,
    /// HTTP genérico (oEmbed das playlists importadas).
    pub http: reqwest::Client,
}

impl AppState {
    pub fn new(app: &AppHandle) -> Self {
        // Um cliente HTTP só, compartilhado: reaproveita conexões e o pool de TLS.
        let http = reqwest::Client::new();
        let providers = Providers::new(app, &TokenStore, &http);

        // Eventos do motor de áudio (fim de faixa, falha) voltam para os comandos do player.
        let handle = app.clone();
        let events: EventSink = Arc::new(move |event| {
            let app = handle.clone();
            tauri::async_runtime::spawn(async move { crate::commands::on_audio_event(&app, event).await });
        });

        Self {
            favorites: Favorites::load(app.path().app_data_dir().ok().map(|d| d.join("favorites.json"))),
            audio: Arc::new(Audio::new(
                providers.clone(),
                events,
                app.path().resource_dir().ok().as_deref(),
                app.path().app_cache_dir().ok(),
                app.path().app_data_dir().ok(),
            )),
            social: Social::new(http.clone()),
            lyrics: LyricsClient::new(http.clone()),
            sponsor: SponsorBlock::new(http.clone()),
            imports: Imports::load(app.path().app_data_dir().ok().map(|d| d.join("imported-playlists.json"))),
            local_playlists: LocalPlaylists::load(app.path().app_data_dir().ok().map(|d| d.join("local-playlists.json"))),
            http: http.clone(),
            providers,
            player: Mutex::new(Player::default()),
            presence: Presence::spawn(DISCORD_APP_ID.map(str::to_owned)),
        }
    }
}

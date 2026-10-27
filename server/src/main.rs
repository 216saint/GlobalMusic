//! Servidor social do GlobalMusic: contas (Discord/Spotify), perfis com foto e amizades.
//! Desenho e motivos: docs/decisoes/0008-servidor-social.md.
//!
//! Configuração por variáveis de ambiente (na VPS: /etc/globalmusic/api.env):
//! - `BIND` (padrão 127.0.0.1:3480) — só local; o Caddy faz o HTTPS na frente.
//! - `DATABASE_URL` (padrão sqlite://./globalmusic.db)
//! - `PUBLIC_URL` (ex.: https://api.globalmusic.216saint.com.br)
//! - `AVATAR_DIR` (onde ficam as fotos enviadas; o Caddy serve em /avatars)
//! - `DISCORD_CLIENT_ID`, `DISCORD_CLIENT_SECRET` (opcionais: sem eles, só login pelo Spotify)

mod auth;
mod error;
mod friends;
mod limit;
mod users;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, post, put};
use axum::{middleware, Router};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use tokio::sync::Mutex;

pub struct DiscordConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

pub struct Config {
    pub public_url: String,
    pub avatar_dir: PathBuf,
    pub discord: Option<DiscordConfig>,
}

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::SqlitePool,
    pub http: reqwest::Client,
    pub config: Arc<Config>,
    pub pending: Arc<Mutex<auth::PendingLogins>>,
    pub limiter: Arc<limit::RateLimiter>,
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let public_url = env("PUBLIC_URL").unwrap_or_else(|| "http://127.0.0.1:3480".into()).trim_end_matches('/').to_owned();
    let avatar_dir = PathBuf::from(env("AVATAR_DIR").unwrap_or_else(|| "./avatars".into()));
    std::fs::create_dir_all(&avatar_dir)?;
    let discord = match (env("DISCORD_CLIENT_ID"), env("DISCORD_CLIENT_SECRET")) {
        (Some(client_id), Some(client_secret)) => Some(DiscordConfig {
            client_id,
            client_secret,
            redirect_uri: format!("{public_url}/auth/discord/callback"),
        }),
        _ => {
            tracing::warn!("DISCORD_CLIENT_ID/SECRET ausentes: login pelo Discord desligado");
            None
        }
    };

    let db_url = env("DATABASE_URL").unwrap_or_else(|| "sqlite://./globalmusic.db".into());
    let options = db_url
        .parse::<SqliteConnectOptions>()?
        .create_if_missing(true)
        .foreign_keys(true)
        // WAL: leituras não esperam escritas; bom para muitos apps consultando ao mesmo tempo.
        .journal_mode(SqliteJournalMode::Wal);
    let db = SqlitePoolOptions::new().max_connections(8).connect_with(options).await?;
    sqlx::migrate!("./migrations").run(&db).await?;

    let state = AppState {
        db,
        http: reqwest::Client::builder().timeout(Duration::from_secs(15)).build()?,
        config: Arc::new(Config { public_url, avatar_dir, discord }),
        pending: Arc::new(Mutex::new(auth::PendingLogins::default())),
        limiter: Arc::new(limit::RateLimiter::default()),
    };

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/auth/spotify", post(auth::login_spotify))
        .route("/auth/discord/start", get(auth::discord_start))
        .route("/auth/discord/callback", get(auth::discord_callback))
        .route("/auth/logout", post(auth::logout))
        .route("/me", get(users::get_me).patch(users::update_me))
        .route("/me/avatar", put(users::upload_avatar).layer(DefaultBodyLimit::max(users::AVATAR_MAX_BYTES)))
        .route("/me/link/spotify", post(auth::link_spotify))
        .route("/me/link/discord", post(auth::link_discord_ticket))
        .route("/users/search", get(users::search))
        .route("/friends", get(friends::list))
        .route("/friends/{username}", post(friends::add).merge(delete(friends::remove)))
        .layer(DefaultBodyLimit::max(64 * 1024))
        .layer(middleware::from_fn_with_state(state.clone(), limit::middleware))
        .with_state(state);

    let bind = env("BIND").unwrap_or_else(|| "127.0.0.1:3480".into());
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!("GlobalMusic API ouvindo em {bind}");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

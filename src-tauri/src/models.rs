//! Modelos compartilhados entre backend e frontend (espelhados em `src/lib/types.ts`).

use std::fmt;

use serde::{Deserialize, Serialize};

pub use crate::artwork::Artwork;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Spotify,
    YoutubeMusic,
    Soundcloud,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Source::Spotify => "Spotify",
            Source::YoutubeMusic => "YouTube Music",
            Source::Soundcloud => "SoundCloud",
        }
    }

    /// Chave estável usada no cofre de credenciais.
    pub fn key(self) -> &'static str {
        match self {
            Source::Spotify => "spotify",
            Source::YoutubeMusic => "youtube_music",
            Source::Soundcloud => "soundcloud",
        }
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    /// ID nativo do provedor (ID base62 do Spotify, videoId do YouTube, id numérico do SoundCloud).
    pub id: String,
    pub source: Source,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: u64,
    pub artwork: Option<Artwork>,
    /// Só o Spotify expõe ISRC; ajuda a achar o equivalente em outra fonte.
    pub isrc: Option<String>,
}

impl Track {
    /// Link público da faixa, quando dá para montar só com o ID.
    pub fn web_url(&self) -> Option<String> {
        match self.source {
            Source::Spotify => Some(format!("https://open.spotify.com/track/{}", self.id)),
            Source::YoutubeMusic => Some(format!("https://music.youtube.com/watch?v={}", self.id)),
            // TODO(fase 2): guardar o `permalink_url` do SoundCloud no Track.
            Source::Soundcloud => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: String,
    pub source: Source,
    pub name: String,
    pub owner: Option<String>,
    pub track_count: Option<u32>,
    pub artwork: Option<Artwork>,
}

/// Resultado de uma operação feita em paralelo nas fontes: cada fonte responde
/// (ou falha) de forma independente, e a UI mostra o que chegou.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceResults<T> {
    pub source: Source,
    pub items: Vec<T>,
    pub error: Option<String>,
}

impl<T> SourceResults<T> {
    pub fn ok(source: Source, items: Vec<T>) -> Self {
        Self { source, items, error: None }
    }

    pub fn failed(source: Source, error: impl Into<String>) -> Self {
        Self { source, items: Vec::new(), error: Some(error.into()) }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountStatus {
    pub source: Source,
    pub connected: bool,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    /// `None` quando a fonte não tem conceito de plano pago relevante para a reprodução.
    pub premium: Option<bool>,
    /// Algo que a pessoa precisa fazer nesta conta (ex.: reconectar para liberar algo).
    pub notice: Option<String>,
}

impl AccountStatus {
    pub fn disconnected(source: Source) -> Self {
        Self { source, connected: false, display_name: None, avatar_url: None, premium: None, notice: None }
    }
}

//! Decide *como* tocar cada faixa. É aqui que contas Spotify Free continuam
//! funcionando: a faixa do Spotify vira metadado e o áudio vem do YouTube.

use serde::Serialize;

use crate::models::{Source, Track};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum PlaybackRoute {
    /// librespot (Spotify Premium).
    SpotifyNative,
    /// Equivalente encontrado no YouTube (Spotify Free ou sem librespot).
    YoutubeFallback,
    /// Stream HTTP direto da própria fonte (YouTube Music, SoundCloud).
    DirectStream,
}

pub fn route_for(track: &Track, spotify_premium: bool) -> PlaybackRoute {
    match track.source {
        Source::Spotify if spotify_premium && cfg!(feature = "spotify-premium") => PlaybackRoute::SpotifyNative,
        Source::Spotify => PlaybackRoute::YoutubeFallback,
        Source::YoutubeMusic | Source::Soundcloud => PlaybackRoute::DirectStream,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(source: Source) -> Track {
        Track {
            id: "x".into(),
            source,
            title: "t".into(),
            artists: vec![],
            album: None,
            duration_ms: 0,
            artwork: None,
            isrc: None,
        }
    }

    #[test]
    fn spotify_free_falls_back_to_youtube() {
        assert_eq!(route_for(&track(Source::Spotify), false), PlaybackRoute::YoutubeFallback);
    }

    #[test]
    #[cfg(feature = "spotify-premium")]
    fn spotify_premium_plays_natively() {
        assert_eq!(route_for(&track(Source::Spotify), true), PlaybackRoute::SpotifyNative);
    }

    #[test]
    fn other_sources_stream_directly_regardless_of_plan() {
        for premium in [true, false] {
            assert_eq!(route_for(&track(Source::YoutubeMusic), premium), PlaybackRoute::DirectStream);
            assert_eq!(route_for(&track(Source::Soundcloud), premium), PlaybackRoute::DirectStream);
        }
    }
}

//! Formato dos dados da Web API do Spotify e conversão para os nossos modelos.
//!
//! O Spotify renomeou alguns campos em 2026 (`tracks` → `items` em playlists,
//! `track` → `item` nos itens). Os `alias` do serde aceitam os dois nomes.

use std::collections::HashMap;

use serde::Deserialize;

use crate::artwork::{self, Sized};
use crate::models::{Playlist, Source, Track};

/// Página de resultados: os itens + a URL da próxima página (se houver).
#[derive(Debug, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next: Option<String>,
    pub total: Option<u32>,
}

/// `GET /tracks?ids=…`: um item por id pedido (`null` para id desconhecido).
#[derive(Debug, Deserialize)]
pub struct SeveralTracks {
    pub tracks: Vec<Option<SpTrack>>,
}

#[derive(Debug, Deserialize)]
pub struct SearchResponse {
    pub tracks: Page<SpTrack>,
}

#[derive(Debug, Deserialize)]
pub struct SpTrack {
    /// `None` para arquivos locais do usuário (não tocáveis por streaming).
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub artists: Vec<Named>,
    pub album: Option<SpAlbum>,
    #[serde(default)]
    pub duration_ms: u64,
    #[serde(default)]
    pub external_ids: HashMap<String, String>,
    #[serde(default)]
    pub is_local: bool,
}

#[derive(Debug, Deserialize)]
pub struct Named {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct SpAlbum {
    pub name: String,
    #[serde(default)]
    pub images: Vec<SpImage>,
}

#[derive(Debug, Deserialize)]
pub struct SpImage {
    pub url: String,
    pub width: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct SpPlaylist {
    pub id: String,
    pub name: String,
    pub owner: Option<Owner>,
    #[serde(default)]
    pub images: Option<Vec<SpImage>>,
    #[serde(alias = "items")]
    pub tracks: Option<Total>,
}

#[derive(Debug, Deserialize)]
pub struct Owner {
    #[serde(default)]
    pub id: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Total {
    pub total: Option<u32>,
}

/// Item de playlist ou de "Músicas Curtidas". Pode ser episódio de podcast (sem `track`).
#[derive(Debug, Deserialize)]
pub struct Item {
    #[serde(alias = "item")]
    pub track: Option<SpTrack>,
}

fn art(images: &[SpImage]) -> Option<artwork::Artwork> {
    let sized: Vec<Sized> = images.iter().map(|i| Sized { url: &i.url, px: i.width }).collect();
    artwork::from_spotify(&sized)
}

impl SpTrack {
    pub fn into_track(self) -> Option<Track> {
        if self.is_local {
            return None;
        }
        Some(Track {
            id: self.id?,
            source: Source::Spotify,
            title: self.name,
            artists: self.artists.into_iter().map(|a| a.name).collect(),
            artwork: self.album.as_ref().and_then(|a| art(&a.images)),
            album: self.album.map(|a| a.name),
            duration_ms: self.duration_ms,
            isrc: self.external_ids.get("isrc").cloned(),
        })
    }
}

impl SpPlaylist {
    /// Feita pelo próprio Spotify ("This Is", Daily Mix, Descobertas da Semana, editoriais).
    pub fn by_spotify(&self) -> bool {
        self.owner.as_ref().is_some_and(|o| o.id == SPOTIFY_OWNER)
    }

    pub fn into_playlist(self) -> Playlist {
        Playlist {
            id: self.id,
            source: Source::Spotify,
            name: self.name,
            owner: self.owner.and_then(|o| o.display_name),
            track_count: self.tracks.and_then(|t| t.total),
            artwork: self.images.as_deref().and_then(art),
            url: None,
        }
    }
}

/// Id do dono das playlists automáticas e editoriais.
const SPOTIFY_OWNER: &str = "spotify";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_playlists_made_by_spotify() {
        let playlist = |owner: &str| SpPlaylist {
            id: "p".into(),
            name: "x".into(),
            owner: Some(Owner { id: owner.into(), display_name: None }),
            images: None,
            tracks: None,
        };
        assert!(playlist("spotify").by_spotify());
        assert!(!playlist("216saint").by_spotify());
    }

    #[test]
    fn maps_search_result_with_isrc_and_two_sized_cover() {
        let json = r#"{ "tracks": { "next": null, "total": 1, "items": [{
            "id": "11dFghVXANMlKmJXsNCbNl", "name": "Cut To The Feeling", "duration_ms": 207959,
            "artists": [{ "name": "Carly Rae Jepsen" }],
            "external_ids": { "isrc": "USUM71703861" },
            "album": { "name": "Cut To The Feeling", "images": [
                { "url": "https://i.scdn.co/640", "width": 640 },
                { "url": "https://i.scdn.co/300", "width": 300 },
                { "url": "https://i.scdn.co/64", "width": 64 } ] }
        }]}}"#;
        let response: SearchResponse = serde_json::from_str(json).unwrap();
        let track = response.tracks.items.into_iter().next().unwrap().into_track().unwrap();
        assert_eq!(track.id, "11dFghVXANMlKmJXsNCbNl");
        assert_eq!(track.isrc.as_deref(), Some("USUM71703861"));
        let art = track.artwork.unwrap();
        assert_eq!((art.thumb.as_str(), art.full.as_str()), ("https://i.scdn.co/300", "https://i.scdn.co/640"));
    }

    #[test]
    fn accepts_old_and_new_playlist_field_names() {
        let old: SpPlaylist = serde_json::from_str(r#"{ "id": "a", "name": "Foco", "tracks": { "total": 120 } }"#).unwrap();
        let new: SpPlaylist = serde_json::from_str(r#"{ "id": "b", "name": "Foco", "items": { "total": 7 } }"#).unwrap();
        assert_eq!(old.into_playlist().track_count, Some(120));
        assert_eq!(new.into_playlist().track_count, Some(7));

        let item: Item = serde_json::from_str(r#"{ "item": { "id": "x", "name": "t" } }"#).unwrap();
        assert!(item.track.is_some());
    }

    #[test]
    fn local_files_and_podcast_episodes_are_skipped() {
        let local: SpTrack = serde_json::from_str(r#"{ "id": null, "name": "minha gravação", "is_local": true }"#).unwrap();
        assert!(local.into_track().is_none());
        let episode: Item = serde_json::from_str(r#"{ "track": null }"#).unwrap();
        assert!(episode.track.is_none());
    }
}

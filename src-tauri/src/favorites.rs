//! Curtidas do próprio GlobalMusic: uma lista única com faixas de qualquer fonte.
//! Ficam num JSON na pasta de dados do app (`favorites.json`), a mais recente primeiro.
//!
//! Curtir no GlobalMusic também curte no serviço de origem (ver `commands::set_favorite`);
//! se o serviço falhar, a curtida local é mantida e a UI avisa.

use std::path::PathBuf;
use std::sync::Mutex;

use crate::models::Track;

pub struct Favorites {
    path: Option<PathBuf>,
    tracks: Mutex<Vec<Track>>,
}

impl Favorites {
    pub fn load(path: Option<PathBuf>) -> Self {
        let tracks = path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        Self { path, tracks: Mutex::new(tracks) }
    }

    pub fn list(&self) -> Vec<Track> {
        self.tracks.lock().map(|t| t.clone()).unwrap_or_default()
    }

    /// Curte/descurte e grava em disco. Devolve a lista atualizada.
    pub fn set(&self, track: &Track, liked: bool) -> Vec<Track> {
        let Ok(mut tracks) = self.tracks.lock() else { return Vec::new() };
        tracks.retain(|t| !(t.source == track.source && t.id == track.id));
        if liked {
            tracks.insert(0, track.clone());
        }
        if let Some(path) = &self.path {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Ok(json) = serde_json::to_string(&*tracks) {
                let _ = std::fs::write(path, json);
            }
        }
        tracks.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Source;

    fn track(id: &str, source: Source) -> Track {
        Track {
            id: id.into(),
            source,
            title: id.into(),
            artists: vec![],
            album: None,
            duration_ms: 0,
            artwork: None,
            isrc: None,
        }
    }

    #[test]
    fn likes_mix_sources_newest_first_and_persist() {
        let dir = std::env::temp_dir().join(format!("gm-fav-{}", std::process::id()));
        let path = dir.join("favorites.json");
        let _ = std::fs::remove_dir_all(&dir);

        let favorites = Favorites::load(Some(path.clone()));
        favorites.set(&track("a", Source::Spotify), true);
        favorites.set(&track("a", Source::YoutubeMusic), true); // mesmo id, outra fonte: outra faixa
        let list = favorites.set(&track("a", Source::Spotify), true); // curtir de novo só move para o topo
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].source, Source::Spotify);

        let reloaded = Favorites::load(Some(path));
        assert_eq!(reloaded.list().len(), 2);
        assert!(reloaded.set(&track("a", Source::Spotify), false).len() == 1);

        let _ = std::fs::remove_dir_all(dir);
    }
}

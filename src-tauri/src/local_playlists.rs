//! Playlists do próprio GlobalMusic: criadas no app, com faixas de qualquer fonte e sem precisar de
//! conta conectada. Ficam num JSON na pasta de dados (`local-playlists.json`), a mais nova primeiro.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::models::Track;

const MAX_NAME: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalPlaylist {
    pub id: String,
    pub name: String,
    pub tracks: Vec<Track>,
}

pub struct LocalPlaylists {
    path: Option<PathBuf>,
    list: Mutex<Vec<LocalPlaylist>>,
}

impl LocalPlaylists {
    pub fn load(path: Option<PathBuf>) -> Self {
        let list = path
            .as_deref()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self { path, list: Mutex::new(list) }
    }

    pub fn list(&self) -> Vec<LocalPlaylist> {
        self.list.lock().map(|l| l.clone()).unwrap_or_default()
    }

    pub fn create(&self, name: &str, tracks: Vec<Track>) -> Result<Vec<LocalPlaylist>> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Other("dê um nome para a playlist".into()));
        }
        let playlist = LocalPlaylist {
            id: format!("{:016x}", rand::random::<u64>()),
            name: name.chars().take(MAX_NAME).collect(),
            tracks: without_repeats(tracks),
        };
        self.change(|list| {
            list.insert(0, playlist);
            Ok(())
        })
    }

    /// Adiciona no fim; faixas que já estão na playlist não entram de novo.
    pub fn add(&self, id: &str, tracks: Vec<Track>) -> Result<Vec<LocalPlaylist>> {
        self.change(|list| {
            let playlist = find(list, id)?;
            for track in tracks {
                if !playlist.tracks.iter().any(|t| same(t, &track)) {
                    playlist.tracks.push(track);
                }
            }
            Ok(())
        })
    }

    pub fn remove_track(&self, id: &str, index: usize) -> Result<Vec<LocalPlaylist>> {
        self.change(|list| {
            let playlist = find(list, id)?;
            if index < playlist.tracks.len() {
                playlist.tracks.remove(index);
            }
            Ok(())
        })
    }

    pub fn delete(&self, id: &str) -> Result<Vec<LocalPlaylist>> {
        self.change(|list| {
            list.retain(|p| p.id != id);
            Ok(())
        })
    }

    /// Aplica a mudança, grava em disco e devolve a lista atualizada.
    fn change(&self, edit: impl FnOnce(&mut Vec<LocalPlaylist>) -> Result<()>) -> Result<Vec<LocalPlaylist>> {
        let mut list = self.list.lock().map_err(|_| Error::Other("playlists indisponíveis".into()))?;
        edit(&mut list)?;
        if let Some(path) = &self.path {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Err(e) = serde_json::to_vec(&*list).map_err(|e| e.to_string()).and_then(|json| {
                std::fs::write(path, json).map_err(|e| e.to_string())
            }) {
                log::warn!("não gravei as playlists locais: {e}");
            }
        }
        Ok(list.clone())
    }
}

fn find<'a>(list: &'a mut [LocalPlaylist], id: &str) -> Result<&'a mut LocalPlaylist> {
    list.iter_mut().find(|p| p.id == id).ok_or_else(|| Error::Other("essa playlist não existe mais".into()))
}

fn same(a: &Track, b: &Track) -> bool {
    a.source == b.source && a.id == b.id
}

fn without_repeats(tracks: Vec<Track>) -> Vec<Track> {
    let mut out: Vec<Track> = Vec::with_capacity(tracks.len());
    for track in tracks {
        if !out.iter().any(|t| same(t, &track)) {
            out.push(track);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Source;

    fn track(id: &str) -> Track {
        Track {
            id: id.into(),
            source: Source::YoutubeMusic,
            title: id.into(),
            artists: vec![],
            album: None,
            duration_ms: 0,
            artwork: None,
            isrc: None,
        }
    }

    #[test]
    fn creates_adds_without_repeats_removes_and_deletes() {
        let store = LocalPlaylists::load(None);
        assert!(store.create("  ", vec![]).is_err(), "sem nome não cria");

        let list = store.create("Estudar", vec![track("a"), track("a"), track("b")]).unwrap();
        let id = list[0].id.clone();
        assert_eq!(list[0].tracks.len(), 2, "repetidas entram uma vez só");

        let list = store.add(&id, vec![track("b"), track("c")]).unwrap();
        assert_eq!(list[0].tracks.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);

        let list = store.remove_track(&id, 0).unwrap();
        assert_eq!(list[0].tracks[0].id, "b");

        assert!(store.delete(&id).unwrap().is_empty());
        assert!(store.add(&id, vec![track("x")]).is_err());
    }
}

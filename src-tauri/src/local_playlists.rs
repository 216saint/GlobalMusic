//! Playlists do próprio GlobalMusic: criadas no app, com faixas de qualquer fonte e sem precisar de
//! conta conectada. Ficam num JSON na pasta de dados (`local-playlists.json`), a mais nova primeiro.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::models::Track;

const MAX_NAME: usize = 100;
/// A foto vem já reduzida pela UI (JPEG de 512 px, ~60 KB); o limite só barra algo fora do comum.
const MAX_COVER_BYTES: usize = 400 * 1024;
const COVER_PREFIX: &str = "data:image/jpeg;base64,";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalPlaylist {
    pub id: String,
    pub name: String,
    pub tracks: Vec<Track>,
    /// Foto escolhida pela pessoa (data URL JPEG). Sem ela, a capa é o mosaico das músicas.
    #[serde(default)]
    pub cover: Option<String>,
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

    pub fn create(&self, name: &str, tracks: Vec<Track>, cover: Option<String>) -> Result<Vec<LocalPlaylist>> {
        let playlist = LocalPlaylist {
            id: format!("{:016x}", rand::random::<u64>()),
            name: clean_name(name)?,
            tracks: without_repeats(tracks),
            cover: cover.map(checked_cover).transpose()?,
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

    /// Muda o nome e/ou a foto (`cover: Some(None)` tira a foto).
    pub fn edit(&self, id: &str, name: Option<&str>, cover: Option<Option<String>>) -> Result<Vec<LocalPlaylist>> {
        let name = name.map(clean_name).transpose()?;
        let cover = cover.map(|c| c.map(checked_cover).transpose()).transpose()?;
        self.change(|list| {
            let playlist = find(list, id)?;
            if let Some(name) = name {
                playlist.name = name;
            }
            if let Some(cover) = cover {
                playlist.cover = cover;
            }
            Ok(())
        })
    }

    pub fn get(&self, id: &str) -> Option<LocalPlaylist> {
        self.list.lock().ok()?.iter().find(|p| p.id == id).cloned()
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

fn clean_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::Other("dê um nome para a playlist".into()));
    }
    Ok(name.chars().take(MAX_NAME).collect())
}

fn checked_cover(cover: String) -> Result<String> {
    if !cover.starts_with(COVER_PREFIX) || cover.len() > MAX_COVER_BYTES {
        return Err(Error::Other("não deu para usar essa imagem; tente outra".into()));
    }
    Ok(cover)
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
        assert!(store.create("  ", vec![], None).is_err(), "sem nome não cria");
        assert!(store.create("x", vec![], Some("data:text/html,oi".into())).is_err(), "só JPEG");

        let list = store.create("Estudar", vec![track("a"), track("a"), track("b")], None).unwrap();
        let id = list[0].id.clone();
        assert_eq!(list[0].tracks.len(), 2, "repetidas entram uma vez só");

        let list = store.add(&id, vec![track("b"), track("c")]).unwrap();
        assert_eq!(list[0].tracks.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);

        let list = store.remove_track(&id, 0).unwrap();
        assert_eq!(list[0].tracks[0].id, "b");

        let photo = format!("{COVER_PREFIX}AAAA");
        let list = store.edit(&id, Some("Foco"), Some(Some(photo.clone()))).unwrap();
        assert_eq!((list[0].name.as_str(), list[0].cover.as_deref()), ("Foco", Some(photo.as_str())));
        let list = store.edit(&id, None, Some(None)).unwrap();
        assert_eq!((list[0].name.as_str(), list[0].cover.as_deref()), ("Foco", None), "tira só a foto");

        assert!(store.delete(&id).unwrap().is_empty());
        assert!(store.add(&id, vec![track("x")]).is_err());
    }
}

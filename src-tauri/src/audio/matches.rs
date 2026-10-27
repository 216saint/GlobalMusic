//! Equivalência no YouTube: qual vídeo toca no lugar de uma faixa do Spotify (conta Free, ou quando
//! o Spotify recusa tocar). A busca acha sozinha, mas pode errar (versão ao vivo, cover…); por isso a
//! pessoa pode **escolher outra versão**, e a escolha dela vale para sempre.
//!
//! Fica num arquivo pequeno (`youtube-matches.json`): abrir o app de novo não refaz as buscas.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// Teto do arquivo (~100 bytes por faixa): além disso, as automáticas mais antigas saem.
const MAX_ENTRIES: usize = 5_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    video_id: String,
    /// Escolhida pela pessoa (a busca automática nunca troca).
    chosen: bool,
    /// Ordem de gravação, para saber quais são as mais antigas.
    seq: u64,
}

pub struct Matches {
    entries: Mutex<HashMap<String, Entry>>,
    file: Option<PathBuf>,
}

impl Matches {
    pub fn load(file: Option<PathBuf>) -> Self {
        let entries = file
            .as_deref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self { entries: Mutex::new(entries), file }
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.entries.lock().ok()?.get(key).map(|e| e.video_id.clone())
    }

    /// Resultado da busca automática (não passa por cima de uma escolha da pessoa).
    pub fn remember(&self, key: &str, video_id: &str) {
        self.update(|entries, seq| {
            if entries.get(key).is_some_and(|e| e.chosen) {
                return false;
            }
            entries.insert(key.to_owned(), Entry { video_id: video_id.to_owned(), chosen: false, seq });
            true
        });
    }

    /// A pessoa escolheu esta versão.
    pub fn choose(&self, key: &str, video_id: &str) {
        self.update(|entries, seq| {
            entries.insert(key.to_owned(), Entry { video_id: video_id.to_owned(), chosen: true, seq });
            true
        });
    }

    fn update(&self, change: impl FnOnce(&mut HashMap<String, Entry>, u64) -> bool) {
        let Ok(mut entries) = self.entries.lock() else { return };
        let seq = entries.values().map(|e| e.seq).max().map_or(0, |s| s + 1);
        if !change(&mut entries, seq) {
            return;
        }
        if entries.len() > MAX_ENTRIES {
            let mut automatic: Vec<_> = entries.iter().filter(|(_, e)| !e.chosen).map(|(k, e)| (e.seq, k.clone())).collect();
            automatic.sort();
            for (_, key) in automatic.into_iter().take(entries.len() - MAX_ENTRIES) {
                entries.remove(&key);
            }
        }
        if let (Some(path), Ok(json)) = (&self.file, serde_json::to_vec(&*entries)) {
            if let Err(e) = std::fs::write(path, json) {
                log::info!("não gravei as equivalências do YouTube: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn your_choice_wins_and_survives_restarts() {
        let file = std::env::temp_dir().join(format!("gm-matches-{}.json", std::process::id()));
        let matches = Matches::load(Some(file.clone()));
        matches.remember("spotify:1", "auto");
        matches.choose("spotify:1", "escolhido");
        matches.remember("spotify:1", "outra-busca");
        assert_eq!(matches.get("spotify:1").as_deref(), Some("escolhido"), "a busca não troca a escolha");

        let reopened = Matches::load(Some(file.clone()));
        assert_eq!(reopened.get("spotify:1").as_deref(), Some("escolhido"));
        let _ = std::fs::remove_file(file);
    }
}

//! Cópia em disco da biblioteca do Spotify: a lista de playlists e as faixas de cada uma.
//!
//! Sem Client ID próprio, todo mundo usa o client oficial e divide a mesma cota da Web API, então
//! o Spotify responde 429 ("espere N s") com frequência — principalmente para contas Free, que não
//! têm alternativa. Em vez de a biblioteca sumir, mostramos a última cópia lida com sucesso; a
//! leitura nova a substitui quando a cota voltar.

use std::path::PathBuf;

use serde::de::DeserializeOwned;
use serde::Serialize;

pub struct LibraryCache {
    dir: Option<PathBuf>,
}

impl LibraryCache {
    pub fn new(dir: Option<PathBuf>) -> Self {
        Self { dir }
    }

    /// Arquivo de `name`; o nome vira só letras, números e `-` (ids do Spotify já são assim).
    fn path(&self, name: &str) -> Option<PathBuf> {
        let safe: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
        Some(self.dir.as_ref()?.join(format!("{safe}.json")))
    }

    pub fn load<T: DeserializeOwned>(&self, name: &str) -> Option<T> {
        serde_json::from_slice(&std::fs::read(self.path(name)?).ok()?).ok()
    }

    pub fn save<T: Serialize>(&self, name: &str, value: &T) {
        let (Some(dir), Some(path)) = (&self.dir, self.path(name)) else { return };
        let _ = std::fs::create_dir_all(dir);
        if let Ok(json) = serde_json::to_vec(value) {
            if let Err(e) = std::fs::write(&path, json) {
                log::info!("Spotify: não gravei a cópia da biblioteca ({e})");
            }
        }
    }

    /// Ao desconectar: a próxima conta não pode ver a biblioteca da anterior.
    pub fn clear(&self) {
        if let Some(dir) = &self.dir {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_clears() {
        let dir = std::env::temp_dir().join(format!("gm-spotify-cache-{}", std::process::id()));
        let cache = LibraryCache::new(Some(dir.clone()));
        cache.save("tracks-37i9dQ/../x", &vec!["a".to_owned()]);
        assert_eq!(cache.load::<Vec<String>>("tracks-37i9dQ/../x"), Some(vec!["a".to_owned()]));
        assert!(dir.join("tracks-37i9dQx.json").is_file(), "nome sem barras nem pontos");
        cache.clear();
        assert_eq!(cache.load::<Vec<String>>("tracks-37i9dQ/../x"), None);
    }
}

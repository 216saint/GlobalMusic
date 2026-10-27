//! Auto-atualização do client ID do Spotify.
//!
//! O ID embutido (`config::spotify::CLIENT_ID`) é o mesmo que o librespot usa.
//! Se o Spotify trocá-lo, o projeto librespot atualiza o código-fonte dele.
//! Então, uma vez por dia, lemos esse arquivo no GitHub, extraímos o ID,
//! validamos o formato e guardamos em cache no disco.

use std::path::PathBuf;
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::config::spotify::CLIENT_ID;
use crate::error::{Error, Result};

const SOURCE_URL: &str = "https://raw.githubusercontent.com/librespot-org/librespot/dev/core/src/config.rs";
const MARKER: &str = "KEYMASTER_CLIENT_ID: &str = \"";
const CHECK_EVERY_SECS: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Cached {
    id: String,
    checked_at: u64,
}

pub struct ClientId {
    http: reqwest::Client,
    cache_path: Option<PathBuf>,
    custom_path: Option<PathBuf>,
    state: RwLock<Cached>,
    /// Client ID do app que a própria pessoa criou no dashboard do Spotify.
    /// Tem prioridade: com ele, a cota de requisições é só dela (ADR 0003).
    custom: RwLock<Option<String>>,
}

impl ClientId {
    /// Usa o cache do disco se existir e for válido; senão, o ID embutido.
    pub fn load(http: reqwest::Client, data_dir: Option<PathBuf>) -> Self {
        let cache_path = data_dir.as_ref().map(|d| d.join("spotify_client_id.json"));
        let custom_path = data_dir.as_ref().map(|d| d.join("spotify_custom_client_id.txt"));
        let cached = cache_path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|json| serde_json::from_str::<Cached>(&json).ok())
            .filter(|c| is_valid(&c.id))
            .unwrap_or(Cached { id: CLIENT_ID.to_owned(), checked_at: 0 });
        let custom = custom_path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|s| s.trim().to_owned())
            .filter(|id| is_valid(id));
        Self { http, cache_path, custom_path, state: RwLock::new(cached), custom: RwLock::new(custom) }
    }

    pub fn get(&self) -> String {
        if let Some(custom) = self.custom() {
            return custom;
        }
        self.state.read().map(|s| s.id.clone()).unwrap_or_else(|_| CLIENT_ID.to_owned())
    }

    /// O client ID do app oficial (keymaster), mesmo com um próprio configurado.
    /// É o único que o Spotify aceita para **tocar** (librespot).
    pub fn official(&self) -> String {
        self.state.read().map(|s| s.id.clone()).unwrap_or_else(|_| CLIENT_ID.to_owned())
    }

    pub fn custom(&self) -> Option<String> {
        self.custom.read().ok().and_then(|c| c.clone())
    }

    /// Define (ou remove, com `None`) o client ID próprio. Devolve `true` se mudou.
    pub fn set_custom(&self, id: Option<&str>) -> Result<bool> {
        let id = id.map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned);
        if let Some(id) = &id {
            if !is_valid(id) {
                return Err(Error::Other("Client ID inválido: são 32 caracteres (0-9, a-f)".into()));
            }
        }
        let changed = self.custom() != id;
        if let Some(path) = &self.custom_path {
            match &id {
                Some(id) => {
                    if let Some(dir) = path.parent() {
                        let _ = std::fs::create_dir_all(dir);
                    }
                    std::fs::write(path, id).map_err(|e| Error::Other(e.to_string()))?;
                }
                None => {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
        if let Ok(mut custom) = self.custom.write() {
            *custom = id;
        }
        Ok(changed)
    }

    /// Verifica a fonte se a última checagem tiver mais de um dia.
    pub async fn refresh_if_stale(&self) -> Result<bool> {
        let checked_at = self.state.read().map(|s| s.checked_at).unwrap_or(0);
        if now().saturating_sub(checked_at) < CHECK_EVERY_SECS {
            return Ok(false);
        }
        self.refresh().await
    }

    /// Busca o ID mais recente. Devolve `true` se ele mudou.
    pub async fn refresh(&self) -> Result<bool> {
        let source = self.http.get(SOURCE_URL).send().await?.error_for_status()?.text().await?;
        let latest = parse_keymaster_id(&source)
            .ok_or_else(|| Error::Other("não achei o client ID na fonte do librespot".into()))?;

        let updated = Cached { id: latest.to_owned(), checked_at: now() };
        let changed = self.state.read().map(|s| s.id != updated.id).unwrap_or(true);
        if let Ok(mut state) = self.state.write() {
            *state = updated.clone();
        }
        if let Some(path) = &self.cache_path {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(path, serde_json::to_string(&updated)?);
        }
        Ok(changed)
    }
}

/// Extrai o ID de `pub(crate) const KEYMASTER_CLIENT_ID: &str = "<32 hex>";`.
pub fn parse_keymaster_id(source: &str) -> Option<&str> {
    let start = source.find(MARKER)? + MARKER.len();
    let id = source.get(start..start + 32)?;
    is_valid(id).then_some(id)
}

/// Só aceitamos exatamente 32 caracteres hexadecimais: o que vem da internet
/// nunca é usado sem validar.
fn is_valid(id: &str) -> bool {
    id.len() == 32 && id.chars().all(|c| c.is_ascii_hexdigit())
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_id_from_librespot_source() {
        let src = r#"pub(crate) const KEYMASTER_CLIENT_ID: &str = "65b708073fc0480ea92a077233ca87bd";
pub(crate) const ANDROID_CLIENT_ID: &str = "9a8d2f0ce77a4e248bb71fefcb557637";"#;
        assert_eq!(parse_keymaster_id(src), Some("65b708073fc0480ea92a077233ca87bd"));
    }

    #[test]
    fn rejects_malformed_ids() {
        assert_eq!(parse_keymaster_id(r#"KEYMASTER_CLIENT_ID: &str = "curto";"#), None);
        assert_eq!(parse_keymaster_id(r#"KEYMASTER_CLIENT_ID: &str = "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz";"#), None);
        assert_eq!(parse_keymaster_id("arquivo sem o marcador"), None);
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("gm-clientid-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn falls_back_to_embedded_id_without_valid_cache() {
        let dir = temp_dir("cache");
        let path = dir.join("spotify_client_id.json");

        std::fs::write(&path, r#"{"id":"invalido","checked_at":1}"#).unwrap();
        assert_eq!(ClientId::load(reqwest::Client::new(), Some(dir.clone())).get(), CLIENT_ID);

        let good = "0123456789abcdef0123456789abcdef";
        std::fs::write(&path, format!(r#"{{"id":"{good}","checked_at":1}}"#)).unwrap();
        assert_eq!(ClientId::load(reqwest::Client::new(), Some(dir.clone())).get(), good);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn custom_id_wins_persists_and_can_be_removed() {
        let dir = temp_dir("custom");
        let mine = "abcdefabcdefabcdefabcdefabcdef12";

        let ids = ClientId::load(reqwest::Client::new(), Some(dir.clone()));
        assert!(ids.set_custom(Some("não-é-um-id")).is_err(), "formato inválido é recusado");
        assert!(ids.set_custom(Some(&format!("  {mine} "))).unwrap(), "espaços são ignorados");
        assert_eq!(ids.get(), mine);

        // Sobrevive a reiniciar o app.
        let reloaded = ClientId::load(reqwest::Client::new(), Some(dir.clone()));
        assert_eq!(reloaded.custom().as_deref(), Some(mine));

        assert!(reloaded.set_custom(None).unwrap());
        assert_eq!(reloaded.get(), CLIENT_ID);
        assert!(ClientId::load(reqwest::Client::new(), Some(dir.clone())).custom().is_none());

        let _ = std::fs::remove_dir_all(dir);
    }
}

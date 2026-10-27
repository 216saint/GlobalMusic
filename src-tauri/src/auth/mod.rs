//! Autenticação:
//! - `oauth`: OAuth 2.0 + PKCE pelo navegador do sistema (Spotify);
//! - `web_login`: login na página oficial numa janela do app, sessão por cookie
//!   (YouTube Music, SoundCloud);
//! - `TokenStore`: tokens OAuth no cofre de credenciais do sistema operacional.

pub mod oauth;
pub mod web_login;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::models::Source;

const SERVICE: &str = "com.globalmusic.app";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Unix timestamp (segundos) de expiração do access token.
    pub expires_at: Option<u64>,
    /// Client ID que emitiu estes tokens. A renovação precisa usar o mesmo;
    /// se o client ID mudar, a pessoa precisa conectar de novo.
    #[serde(default)]
    pub client_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct TokenStore;

impl TokenStore {
    pub fn save(&self, source: Source, tokens: &Tokens) -> Result<()> {
        self.save_as(source.key(), tokens)
    }

    pub fn clear(&self, source: Source) -> Result<()> {
        self.clear_as(source.key())
    }

    // Variantes com nome livre: um serviço pode ter mais de um token
    // (ex.: o Spotify, com um token para a Web API e outro para a reprodução).

    pub fn save_as(&self, key: &str, tokens: &Tokens) -> Result<()> {
        keyring::Entry::new(SERVICE, key)?.set_password(&serde_json::to_string(tokens)?)?;
        Ok(())
    }

    pub fn load_as(&self, key: &str) -> Result<Option<Tokens>> {
        match keyring::Entry::new(SERVICE, key)?.get_password() {
            Ok(json) => Ok(Some(serde_json::from_str(&json)?)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn clear_as(&self, key: &str) -> Result<()> {
        match keyring::Entry::new(SERVICE, key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

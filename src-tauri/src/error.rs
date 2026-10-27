use serde::{Serialize, Serializer};

use crate::models::Source;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0} não está conectado")]
    NotConnected(Source),
    #[error("tempo esgotado")]
    Timeout,
    #[error("muitas requisições agora; tente de novo em {0}s")]
    RateLimited(u64),
    #[error("autorização recusada ou inválida: {0}")]
    OAuth(String),
    #[error("erro de rede: {0}")]
    Http(reqwest::Error),
    #[error("erro no cofre de credenciais: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("JSON inválido: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Other(String),
}

// A URL sai da mensagem: ela pode carregar tokens (ex.: `track_authorization` do SoundCloud),
// que não devem aparecer na tela nem no log. O código HTTP (`status()`) continua disponível.
impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::Http(e.without_url())
    }
}

// Os comandos do Tauri devolvem o erro ao frontend como uma string legível.
impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

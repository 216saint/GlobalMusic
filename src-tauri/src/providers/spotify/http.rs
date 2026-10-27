//! Requisições à Web API do Spotify com tratamento de 429 (Too Many Requests).
//!
//! O Spotify conta o limite por client ID, numa janela de 30s, e responde 429
//! com o header `Retry-After: <segundos>` quando estoura. Em vez de mostrar
//! erro, esperamos o tempo pedido e tentamos de novo — algumas vezes.

use std::time::Duration;

use reqwest::{RequestBuilder, Response, StatusCode};

use crate::error::{Error, Result};

const MAX_RETRIES: u32 = 3;
/// Se o Spotify pedir para esperar mais que isso, desistimos e avisamos a UI.
const MAX_WAIT: Duration = Duration::from_secs(20);
const DEFAULT_WAIT: Duration = Duration::from_secs(1);

/// Envia a requisição montada por `build`, repetindo em caso de 429.
/// Recebe uma closure (e não um `RequestBuilder`) porque cada tentativa
/// precisa de uma requisição nova.
pub async fn send(build: impl Fn() -> RequestBuilder) -> Result<Response> {
    let mut attempt = 0;
    loop {
        let response = build().send().await?;
        if response.status() != StatusCode::TOO_MANY_REQUESTS {
            return Ok(response);
        }
        let wait = retry_after(response.headers().get("retry-after").and_then(|v| v.to_str().ok()));
        attempt += 1;
        if attempt > MAX_RETRIES || wait > MAX_WAIT {
            return Err(Error::RateLimited(wait.as_secs()));
        }
        tokio::time::sleep(wait).await;
    }
}

/// `Retry-After` em segundos; sem header (ou inválido), espera 1s.
fn retry_after(header: Option<&str>) -> Duration {
    header
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map_or(DEFAULT_WAIT, Duration::from_secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_retry_after_seconds() {
        assert_eq!(retry_after(Some("7")), Duration::from_secs(7));
        assert_eq!(retry_after(Some(" 3 ")), Duration::from_secs(3));
    }

    #[test]
    fn defaults_when_header_is_missing_or_garbage() {
        assert_eq!(retry_after(None), DEFAULT_WAIT);
        assert_eq!(retry_after(Some("amanhã")), DEFAULT_WAIT);
    }
}

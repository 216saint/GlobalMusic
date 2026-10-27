//! Limite de requisições por IP (janela fixa): protege o servidor de abuso
//! e de um app com bug fazendo chamadas em loop.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::error::ApiError;
use crate::AppState;

const WINDOW: Duration = Duration::from_secs(60);
const MAX_PER_WINDOW: u32 = 240;

#[derive(Default)]
pub struct RateLimiter {
    windows: Mutex<HashMap<IpAddr, (Instant, u32)>>,
}

impl RateLimiter {
    /// Conta uma requisição; `false` se o IP passou do limite nesta janela.
    pub fn allow(&self, ip: IpAddr, now: Instant) -> bool {
        let Ok(mut windows) = self.windows.lock() else { return true };
        if windows.len() > 50_000 {
            windows.retain(|_, (start, _)| now.duration_since(*start) < WINDOW);
        }
        let entry = windows.entry(ip).or_insert((now, 0));
        if now.duration_since(entry.0) >= WINDOW {
            *entry = (now, 0);
        }
        entry.1 += 1;
        entry.1 <= MAX_PER_WINDOW
    }
}

/// O servidor só escuta em 127.0.0.1, atrás do Caddy, que repassa o IP real em X-Forwarded-For.
fn client_ip(request: &Request) -> Option<IpAddr> {
    request
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .and_then(|v| v.trim().parse().ok())
}

pub async fn middleware(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if let Some(ip) = client_ip(&request) {
        if !state.limiter.allow(ip, Instant::now()) {
            return ApiError::TooManyRequests.into_response();
        }
    }
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_after_the_limit_and_resets_with_the_window() {
        let limiter = RateLimiter::default();
        let ip: IpAddr = "203.0.113.7".parse().unwrap();
        let t0 = Instant::now();
        for _ in 0..MAX_PER_WINDOW {
            assert!(limiter.allow(ip, t0));
        }
        assert!(!limiter.allow(ip, t0), "passou do limite");
        assert!(limiter.allow("203.0.113.8".parse().unwrap(), t0), "outro IP não é afetado");
        assert!(limiter.allow(ip, t0 + WINDOW), "janela nova zera a contagem");
    }
}

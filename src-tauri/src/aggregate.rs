//! Busca e biblioteca federadas: dispara a mesma operação em todas as fontes
//! em paralelo. Uma fonte lenta ou fora do ar nunca bloqueia as outras.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use futures::future::join_all;

use crate::error::{Error, Result};
use crate::models::{Playlist, SourceResults, Track};
use crate::providers::MusicProvider;

pub const SEARCH_TIMEOUT: Duration = Duration::from_secs(8);
pub const LIBRARY_TIMEOUT: Duration = Duration::from_secs(20);

pub async fn fan_out<T, F, Fut>(
    providers: &[Arc<dyn MusicProvider>],
    timeout: Duration,
    op: F,
) -> Vec<SourceResults<T>>
where
    F: Fn(Arc<dyn MusicProvider>) -> Fut,
    Fut: Future<Output = Result<Vec<T>>>,
{
    let calls = providers.iter().cloned().map(|provider| {
        let source = provider.source();
        let call = op(provider);
        async move {
            match tokio::time::timeout(timeout, call).await {
                Ok(Ok(items)) => SourceResults::ok(source, items),
                Ok(Err(e)) => SourceResults::failed(source, e.to_string()),
                Err(_) => SourceResults::failed(source, Error::Timeout.to_string()),
            }
        }
    });
    join_all(calls).await
}

pub async fn search(providers: &[Arc<dyn MusicProvider>], query: &str, limit: u32) -> Vec<SourceResults<Track>> {
    let query = query.trim().to_owned();
    fan_out(providers, SEARCH_TIMEOUT, move |p| {
        let query = query.clone();
        async move { p.search_tracks(&query, limit).await }
    })
    .await
}

pub async fn library(providers: &[Arc<dyn MusicProvider>]) -> Vec<SourceResults<Playlist>> {
    fan_out(providers, LIBRARY_TIMEOUT, |p| async move { p.playlists().await }).await
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use tauri::AppHandle;

    use super::*;
    use crate::models::{AccountStatus, Source};

    enum Behavior {
        Ok,
        Fail,
        Hang,
    }

    struct Fake(Source, Behavior);

    #[async_trait]
    impl MusicProvider for Fake {
        fn source(&self) -> Source {
            self.0
        }
        async fn account(&self) -> AccountStatus {
            AccountStatus::disconnected(self.0)
        }
        async fn connect(&self, _: &AppHandle) -> Result<AccountStatus> {
            unreachable!()
        }
        async fn disconnect(&self) -> Result<()> {
            Ok(())
        }
        async fn search_tracks(&self, query: &str, _limit: u32) -> Result<Vec<Track>> {
            match self.1 {
                Behavior::Ok => Ok(vec![Track {
                    id: "1".into(),
                    source: self.0,
                    title: query.into(),
                    artists: vec![],
                    album: None,
                    duration_ms: 0,
                    artwork: None,
                    isrc: None,
                }]),
                Behavior::Fail => Err(Error::NotConnected(self.0)),
                Behavior::Hang => std::future::pending().await,
            }
        }
        async fn playlists(&self) -> Result<Vec<Playlist>> {
            Ok(vec![])
        }
        async fn playlist_tracks(&self, _: &str) -> Result<Vec<Track>> {
            Ok(vec![])
        }
        async fn set_liked(&self, _: &Track, _: bool) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test(start_paused = true)]
    async fn search_returns_partial_results_when_sources_fail_or_hang() {
        let providers: Vec<Arc<dyn MusicProvider>> = vec![
            Arc::new(Fake(Source::Spotify, Behavior::Fail)),
            Arc::new(Fake(Source::YoutubeMusic, Behavior::Ok)),
            Arc::new(Fake(Source::Soundcloud, Behavior::Hang)),
        ];

        let results = search(&providers, "  worth it ", 10).await;

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].source, Source::Spotify);
        assert_eq!(results[0].error.as_deref(), Some("Spotify não está conectado"));
        assert_eq!(results[1].items[0].title, "worth it");
        assert!(results[1].error.is_none());
        assert_eq!(results[2].error.as_deref(), Some("tempo esgotado"));
    }
}

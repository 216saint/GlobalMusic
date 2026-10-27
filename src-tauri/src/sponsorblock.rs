//! SponsorBlock (sponsor.ajay.app): trechos **sem música** em vídeos do YouTube (a introdução falada
//! de um clipe, a cena depois da música…), marcados pela comunidade, para o player pular.
//!
//! Só vale para áudio que vem do YouTube. Privacidade: pedimos pelos 4 primeiros caracteres do
//! SHA-256 do vídeo (vêm vários vídeos misturados e escolhemos o nosso aqui), então o servidor não
//! sabe o que a pessoa está ouvindo.

use std::collections::VecDeque;
use std::time::Duration;

use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

const API: &str = "https://sponsor.ajay.app/api/skipSegments";
const TIMEOUT: Duration = Duration::from_secs(6);
const REMEMBER: usize = 50;
/// Trechos muito curtos não valem o pulo (e o seek reabre o arquivo).
const MIN_SEGMENT_MS: u64 = 3_000;

/// Um trecho para pular: `[início, fim)` em milissegundos.
pub type Segment = (u64, u64);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VideoSegments {
    #[serde(rename = "videoID")]
    video_id: String,
    #[serde(default)]
    segments: Vec<SegmentRecord>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SegmentRecord {
    segment: [f64; 2],
    #[serde(default)]
    action_type: Option<String>,
}

pub struct SponsorBlock {
    http: reqwest::Client,
    recent: Mutex<VecDeque<(String, Vec<Segment>)>>,
}

impl SponsorBlock {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http, recent: Mutex::new(VecDeque::new()) }
    }

    /// Trechos sem música de um vídeo, em ordem. Falhou a consulta? Nenhum: só não pulamos nada.
    pub async fn segments(&self, video_id: &str) -> Vec<Segment> {
        if let Some((_, found)) = self.recent.lock().await.iter().find(|(id, _)| id == video_id) {
            return found.clone();
        }
        let found = match self.fetch(video_id).await {
            Ok(found) => found,
            Err(e) => {
                log::info!("SponsorBlock indisponível agora: {e}");
                return Vec::new();
            }
        };
        let mut recent = self.recent.lock().await;
        recent.push_front((video_id.to_owned(), found.clone()));
        recent.truncate(REMEMBER);
        found
    }

    async fn fetch(&self, video_id: &str) -> reqwest::Result<Vec<Segment>> {
        let prefix: String = Sha256::digest(video_id.as_bytes()).iter().take(2).map(|b| format!("{b:02x}")).collect();
        let response = self
            .http
            .get(format!("{API}/{prefix}"))
            .query(&[("categories", r#"["music_offtopic"]"#), ("actionTypes", r#"["skip"]"#)])
            .timeout(TIMEOUT)
            .send()
            .await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(Vec::new()); // nenhum vídeo com esse prefixo tem trechos marcados
        }
        let videos: Vec<VideoSegments> = response.error_for_status()?.json().await?;
        Ok(pick(videos, video_id))
    }
}

/// Os trechos do nosso vídeo, em milissegundos, em ordem e sem os curtos demais.
fn pick(videos: Vec<VideoSegments>, video_id: &str) -> Vec<Segment> {
    let mut segments: Vec<Segment> = videos
        .into_iter()
        .filter(|v| v.video_id == video_id)
        .flat_map(|v| v.segments)
        .filter(|s| s.action_type.as_deref().unwrap_or("skip") == "skip")
        .map(|s| ((s.segment[0].max(0.0) * 1000.0) as u64, (s.segment[1].max(0.0) * 1000.0) as u64))
        .filter(|(start, end)| end.saturating_sub(*start) >= MIN_SEGMENT_MS)
        .collect();
    segments.sort();
    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_our_video_in_ms_and_drops_tiny_segments() {
        let videos: Vec<VideoSegments> = serde_json::from_str(
            r#"[
                {"videoID":"outro","segments":[{"segment":[0,30],"actionType":"skip"}]},
                {"videoID":"nosso","segments":[
                    {"segment":[190.5,212.0],"actionType":"skip"},
                    {"segment":[0,14.2],"actionType":"skip"},
                    {"segment":[50,51],"actionType":"skip"}
                ]}
            ]"#,
        )
        .unwrap();
        assert_eq!(pick(videos, "nosso"), [(0, 14_200), (190_500, 212_000)]);
    }
}

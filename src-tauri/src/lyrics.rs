//! Letras, de mais de uma fonte, na ordem:
//!
//! 1. **LRCLIB** (lrclib.net): gratuito, aberto e sem conta. É a única com letra **sincronizada**
//!    (formato LRC, `[01:23.45] verso`): a UI destaca o verso do momento.
//! 2. **YouTube Music** (aba "Letra" do site, em geral licenciada da Musixmatch): só texto corrido,
//!    mas cobre muita coisa que o LRCLIB não tem. Quem busca é `commands::lyrics`, porque precisa
//!    do vídeo equivalente da faixa (ver `Audio::youtube_equivalent`).
//!
//! Guardamos as últimas em memória (poucos KB cada), inclusive o "não tem letra".

use std::collections::VecDeque;
use std::future::Future;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::error::Result;
use crate::models::Track;

const API: &str = "https://lrclib.net/api";
/// O LRCLIB pede um User-Agent que identifique o app.
const USER_AGENT: &str = concat!("GlobalMusic/", env!("CARGO_PKG_VERSION"), " (https://globalmusic.216saint.com.br)");
const TIMEOUT: Duration = Duration::from_secs(8);
const REMEMBER: usize = 20;
/// Registros com menos que isso são lixo de teste (vimos "[00:00.00]probe" no lugar de uma letra real).
const MIN_SYNCED_LINES: usize = 4;
const MIN_PLAIN_CHARS: usize = 80;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricLine {
    pub time_ms: u64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Lyrics {
    /// Versos com o tempo de cada um (vazio se só houver a letra corrida).
    pub synced: Vec<LyricLine>,
    pub plain: Option<String>,
    pub instrumental: bool,
    /// De onde veio, para o crédito na tela (ex.: "LRCLIB", "Musixmatch, pelo YouTube Music").
    pub source: String,
}

impl Lyrics {
    /// Letra corrida do YouTube Music. `credit` é o rodapé do site ("Fonte: Musixmatch").
    pub fn from_youtube(text: String, credit: Option<&str>) -> Self {
        let source = match credit.map(|c| c.trim_start_matches("Fonte:").trim()).filter(|c| !c.is_empty()) {
            Some(provider) => format!("{provider}, pelo YouTube Music"),
            None => "YouTube Music".to_owned(),
        };
        Self { synced: Vec::new(), plain: Some(text), instrumental: false, source }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    duration: Option<f64>,
    synced_lyrics: Option<String>,
    plain_lyrics: Option<String>,
    #[serde(default)]
    instrumental: bool,
}

impl Record {
    /// Tem letra de verdade (ou é instrumental)?
    fn is_real(&self) -> bool {
        let synced_lines = self.synced_lyrics.as_deref().map_or(0, |s| s.lines().filter(|l| !l.trim().is_empty()).count());
        let plain_chars = self.plain_lyrics.as_deref().map_or(0, |p| p.trim().chars().count());
        self.instrumental || synced_lines >= MIN_SYNCED_LINES || plain_chars >= MIN_PLAIN_CHARS
    }

    /// Distância da duração da faixa, em segundos (menor = mais provável que seja a mesma gravação).
    fn duration_gap(&self, track_secs: f64) -> f64 {
        self.duration.map_or(f64::MAX, |d| (d - track_secs).abs())
    }

    fn into_lyrics(self) -> Lyrics {
        Lyrics {
            synced: self.synced_lyrics.as_deref().map(parse_lrc).unwrap_or_default(),
            plain: self.plain_lyrics.filter(|p| !p.trim().is_empty()),
            instrumental: self.instrumental,
            source: "LRCLIB".to_owned(),
        }
    }
}

pub struct LyricsClient {
    http: reqwest::Client,
    /// Últimas buscas (inclusive "não tem letra"), para não repetir ao voltar numa música.
    recent: Mutex<VecDeque<(String, Option<Lyrics>)>>,
}

impl LyricsClient {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http, recent: Mutex::new(VecDeque::new()) }
    }

    /// A letra de `track`: primeiro o LRCLIB; sem nada lá, `fallback` (o YouTube Music).
    pub async fn get<F, Fut>(&self, track: &Track, fallback: F) -> Result<Option<Lyrics>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Option<Lyrics>>,
    {
        let key = format!("{}:{}", track.source.key(), track.id);
        if let Some((_, found)) = self.recent.lock().await.iter().find(|(k, _)| *k == key) {
            return Ok(found.clone());
        }
        let found = match self.fetch(track).await {
            Ok(Some(lyrics)) => Some(lyrics),
            Ok(None) => fallback().await,
            Err(e) => {
                log::warn!("LRCLIB falhou ({e}); tentando a letra do YouTube Music");
                // As duas falharam: devolve o erro e não guarda (pode ter sido a rede).
                Some(fallback().await.ok_or(e)?)
            }
        };
        let mut recent = self.recent.lock().await;
        recent.push_front((key, found.clone()));
        recent.truncate(REMEMBER);
        Ok(found)
    }

    async fn fetch(&self, track: &Track) -> Result<Option<Lyrics>> {
        let track_secs = track.duration_ms as f64 / 1000.0;
        let attempts = queries(track);
        // 1º: busca aberta. Entre as letras de verdade, a de duração mais próxima da faixa.
        // (A busca exata às vezes devolve um registro de teste no lugar da letra.)
        for (title, artist) in &attempts {
            let results: Vec<Record> = self
                .request("search")
                .query(&[("track_name", title.as_str()), ("artist_name", artist.as_str())])
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if let Some(best) = pick(results, track_secs) {
                return Ok(Some(best.into_lyrics()));
            }
        }
        // 2º: busca exata (título + artista + duração).
        let (title, artist) = &attempts[0];
        let mut exact = vec![("track_name", title.clone()), ("artist_name", artist.clone())];
        exact.push(("duration", (track.duration_ms / 1000).to_string()));
        let response = self.request("get").query(&exact).send().await?;
        if !response.status().is_success() {
            return Ok(None);
        }
        let record: Record = response.json().await?;
        Ok(record.is_real().then(|| record.into_lyrics()))
    }

    fn request(&self, endpoint: &str) -> reqwest::RequestBuilder {
        self.http.get(format!("{API}/{endpoint}")).header(reqwest::header::USER_AGENT, USER_AGENT).timeout(TIMEOUT)
    }
}

/// A letra de verdade com a duração mais próxima da faixa.
fn pick(records: Vec<Record>, track_secs: f64) -> Option<Record> {
    records.into_iter().filter(Record::is_real).min_by(|a, b| a.duration_gap(track_secs).total_cmp(&b.duration_gap(track_secs)))
}

/// Pares (título, artista) para procurar, do mais provável ao menos. Vídeo do YouTube costuma vir
/// como "Artista - Música" enviado por um canal qualquer: aí vale também o que está em volta do " - ".
fn queries(track: &Track) -> Vec<(String, String)> {
    let title = clean_title(&track.title);
    let artist = track.artists.first().cloned().unwrap_or_default();
    let mut attempts = vec![(title.clone(), artist)];
    if let Some((before, after)) = title.split_once(" - ") {
        let (before, after) = (before.trim(), after.trim());
        if !before.is_empty() && !after.is_empty() {
            attempts.push((after.to_owned(), before.to_owned()));
        }
    }
    attempts
}

/// Títulos do YouTube vêm com "(Official Video)", "[Lyrics]"…: sem isso, a busca acha mais.
fn clean_title(title: &str) -> String {
    let mut clean = String::with_capacity(title.len());
    let mut depth = 0;
    for c in title.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = (depth - 1).max(0),
            _ if depth == 0 => clean.push(c),
            _ => {}
        }
    }
    let clean = clean.trim();
    if clean.is_empty() { title.trim().to_owned() } else { clean.to_owned() }
}

/// `[mm:ss.xx] verso` → versos com tempo. Uma linha pode ter vários tempos (refrão repetido).
fn parse_lrc(lrc: &str) -> Vec<LyricLine> {
    let mut lines = Vec::new();
    for raw in lrc.lines() {
        let mut rest = raw.trim();
        let mut times = Vec::new();
        while let Some(stripped) = rest.strip_prefix('[') {
            let Some(end) = stripped.find(']') else { break };
            match parse_time(&stripped[..end]) {
                Some(ms) => times.push(ms),
                None => break, // [ar:Artista] e outras tags: não é verso
            }
            rest = &stripped[end + 1..];
        }
        let text = rest.trim();
        lines.extend(times.into_iter().map(|time_ms| LyricLine { time_ms, text: text.to_owned() }));
    }
    lines.sort_by_key(|l| l.time_ms);
    lines
}

/// "01:23.45" → 83 450 ms.
fn parse_time(tag: &str) -> Option<u64> {
    let (minutes, seconds) = tag.split_once(':')?;
    let minutes: u64 = minutes.trim().parse().ok()?;
    let seconds: f64 = seconds.trim().parse().ok()?;
    Some(minutes * 60_000 + (seconds * 1000.0).round() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lrc_including_repeated_choruses_and_skips_tags() {
        let lines = parse_lrc("[ar:Artista]\n[00:12.50] primeiro\n[00:05.00][00:30.00] refrão\n[01:02.3]\n");
        let got: Vec<_> = lines.iter().map(|l| (l.time_ms, l.text.as_str())).collect();
        assert_eq!(got, [(5_000, "refrão"), (12_500, "primeiro"), (30_000, "refrão"), (62_300, "")]);
    }

    #[test]
    fn skips_junk_records_and_prefers_the_closest_duration() {
        let records: Vec<Record> = serde_json::from_str(
            r#"[
                {"duration":213,"syncedLyrics":"[00:00.00]probe","plainLyrics":"probe"},
                {"duration":240,"syncedLyrics":"[00:01.00]a\n[00:02.00]b\n[00:03.00]c\n[00:04.00]d","plainLyrics":null},
                {"duration":212,"syncedLyrics":"[00:01.00]a\n[00:02.00]b\n[00:03.00]c\n[00:05.00]certa","plainLyrics":null}
            ]"#,
        )
        .unwrap();
        let best = pick(records, 213.0).unwrap().into_lyrics();
        assert_eq!(best.synced.last().unwrap().text, "certa");
    }

    #[test]
    fn also_tries_artist_dash_title_uploads() {
        let track = Track {
            id: "x".into(),
            source: crate::models::Source::YoutubeMusic,
            title: "Racionais MC's - Negro Drama (Clipe Oficial)".into(),
            artists: vec!["Canal Qualquer".into()],
            album: None,
            duration_ms: 0,
            artwork: None,
            isrc: None,
        };
        let attempts = queries(&track);
        assert_eq!(attempts[0], ("Racionais MC's - Negro Drama".into(), "Canal Qualquer".into()));
        assert_eq!(attempts[1], ("Negro Drama".into(), "Racionais MC's".into()));
    }

    #[test]
    fn credits_the_youtube_lyrics_provider() {
        assert_eq!(Lyrics::from_youtube("a".into(), Some("Fonte: Musixmatch")).source, "Musixmatch, pelo YouTube Music");
        assert_eq!(Lyrics::from_youtube("a".into(), None).source, "YouTube Music");
    }

    #[test]
    fn strips_youtube_decorations_from_titles() {
        assert_eq!(clean_title("Tudo Roxo (Official Video) [4K]"), "Tudo Roxo");
        assert_eq!(clean_title("(intro)"), "(intro)", "não fica vazio");
    }
}

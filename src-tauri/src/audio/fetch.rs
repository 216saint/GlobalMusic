//! Baixa áudio rápido, em pedaços paralelos (o servidor do YouTube limita a velocidade de
//! **uma** conexão a ~tempo real; com 4 conexões, uma música de 3–5 MB chega em 1–2 s).
//!
//! Formas:
//! - [`download`]: o arquivo inteiro na memória (pré-carregar a próxima faixa; seek livre).
//! - [`stream`]: devolve um [`Incoming`] assim que o **primeiro pedaço** chega (~1 MiB ≈ 1 min de
//!   música), e o resto continua chegando em segundo plano. Dá para começar a tocar ~1–2 s antes.
//! - [`stream_parts`] / [`download_parts`]: o mesmo, para áudio dividido em vários arquivos
//!   (HLS do SoundCloud: `init.mp4` + pedaços `.m4s`, ou pedaços MP3).
//!
//! Por que o `stream` toca sem seek (ver `Audio::reopen_seekable`)? O decodificador de MP4, quando
//! acha que pode pular pelo arquivo, lê o cabeçalho de **todos** os blocos ao abrir — ou seja,
//! esperaria o download inteiro. Sem seek, ele lê só até o primeiro bloco de áudio e começa.

use std::io::{self, Read, Seek, SeekFrom};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::time::{Duration, Instant};

use futures::stream::{self, StreamExt, TryStreamExt};
use reqwest::header::{CONTENT_RANGE, RANGE};
use reqwest::StatusCode;

use crate::error::{Error, Result};

/// Tamanho de cada pedaço. YouTube costuma liberar pedaços de até ~10 MB sem limitar.
const CHUNK: u64 = 1024 * 1024;
const PARALLEL: usize = 4;
/// Teto para a música inteira: estourou, é falha (nunca mais "Carregando…" eterno).
pub const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(45);
/// Áudio de música não passa disso; protege a RAM de URLs erradas (ex.: um vídeo em 4K).
const MAX_BYTES: u64 = 80 * 1024 * 1024;

pub async fn download(http: &reqwest::Client, url: &str) -> Result<Vec<u8>> {
    tokio::time::timeout(DOWNLOAD_TIMEOUT, download_inner(http, url))
        .await
        .map_err(|_| Error::Other("o download do áudio demorou demais".into()))?
}

async fn download_inner(http: &reqwest::Client, url: &str) -> Result<Vec<u8>> {
    // Sonda: pede só o 1º byte para descobrir o tamanho total (Content-Range: bytes 0-0/TOTAL).
    let probe = http.get(url).header(RANGE, "bytes=0-0").send().await?.error_for_status()?;
    let total = probe
        .headers()
        .get(CONTENT_RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(content_range_total);

    match (probe.status(), total) {
        (StatusCode::PARTIAL_CONTENT, Some(total)) if total > 0 => {
            if total > MAX_BYTES {
                return Err(Error::Other(format!("arquivo de áudio grande demais ({} MB)", total / 1024 / 1024)));
            }
            let parts: Vec<Vec<u8>> = stream::iter(chunk_ranges(total, CHUNK))
                .map(|(start, end)| fetch_part(http.clone(), Part::range(url, start, end)))
                .buffered(PARALLEL) // paralelo, mas devolve na ordem certa
                .try_collect()
                .await?;
            Ok(parts.concat())
        }
        // Servidor sem suporte a Range: baixa de uma vez.
        _ => Ok(http.get(url).send().await?.error_for_status()?.bytes().await?.to_vec()),
    }
}

/// Áudio que ainda está chegando. Os pedaços entram em ordem; quem lê espera os que faltam.
pub struct Incoming {
    total: u64,
    state: Mutex<Arrived>,
    arrived: Condvar,
}

/// O que fazer com o arquivo inteiro quando ele terminar de chegar.
type OnComplete = Box<dyn FnOnce(Vec<u8>) + Send>;

struct Arrived {
    data: Vec<u8>,
    done: bool,
    error: Option<String>,
    on_complete: Option<OnComplete>,
}

/// Tamanho ainda desconhecido (HLS: só sabemos ao fim do último pedaço).
const UNKNOWN_LEN: u64 = u64::MAX;

impl Incoming {
    fn new(total: u64, data: Vec<u8>, done: bool) -> Arc<Self> {
        let state = Arrived { data, done, error: None, on_complete: None };
        Arc::new(Self { total, state: Mutex::new(state), arrived: Condvar::new() })
    }

    /// Chama `f` uma vez, com o arquivo inteiro, se o download terminar bem (ex.: guardar no cache).
    /// Se a pessoa pular a faixa antes, o download para e `f` nunca roda.
    pub fn when_complete(&self, f: impl FnOnce(Vec<u8>) + Send + 'static) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.done && state.error.is_none() {
            let data = state.data.clone();
            drop(state);
            f(data);
        } else {
            state.on_complete = Some(Box::new(f));
        }
    }

    /// Terminou de chegar: acorda quem lê e roda o `when_complete` (fora da trava).
    fn finish(&self) {
        let ready = {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            state.done = true;
            state.on_complete.take().map(|f| (f, state.data.clone()))
        };
        self.arrived.notify_all();
        if let Some((f, data)) = ready {
            f(data);
        }
    }

    /// Quantos bytes já chegaram.
    fn arrived(&self) -> u64 {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).data.len() as u64
    }

    fn update(&self, change: impl FnOnce(&mut Arrived)) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        change(&mut state);
        self.arrived.notify_all();
    }

    /// Tamanho total, se já se sabe (o decodificador usa para saber onde o arquivo acaba).
    pub fn known_len(&self) -> Option<u64> {
        (self.total != UNKNOWN_LEN).then_some(self.total)
    }

    /// Leitor para o decodificador (bloqueia esperando os bytes que ainda não chegaram).
    pub fn reader(self: &Arc<Self>) -> IncomingReader {
        IncomingReader { incoming: self.clone(), pos: 0 }
    }

    /// Espera o download terminar e devolve o arquivo inteiro (para reabrir com seek).
    pub async fn complete(self: Arc<Self>) -> Result<Vec<u8>> {
        tokio::task::spawn_blocking(move || {
            let deadline = Instant::now() + DOWNLOAD_TIMEOUT;
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            loop {
                if let Some(error) = &state.error {
                    return Err(Error::Other(error.clone()));
                }
                if state.done {
                    return Ok(state.data.clone());
                }
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return Err(Error::Other("o download do áudio demorou demais".into()));
                }
                state = self.arrived.wait_timeout(state, left).unwrap_or_else(|e| e.into_inner()).0;
            }
        })
        .await
        .map_err(|e| Error::Other(format!("download interrompido: {e}")))?
    }
}

/// Lê um [`Incoming`] do começo ao fim, esperando o que ainda não chegou.
pub struct IncomingReader {
    incoming: Arc<Incoming>,
    pos: u64,
}

impl Read for IncomingReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let incoming = &self.incoming;
        let deadline = Instant::now() + DOWNLOAD_TIMEOUT;
        let mut state = incoming.state.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            let available = state.data.len() as u64;
            if self.pos < available {
                let start = self.pos as usize;
                let n = out.len().min(state.data.len() - start);
                out[..n].copy_from_slice(&state.data[start..start + n]);
                self.pos += n as u64;
                return Ok(n);
            }
            if self.pos >= incoming.total || state.done {
                return Ok(0);
            }
            if let Some(error) = &state.error {
                return Err(io::Error::other(error.clone()));
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "o download do áudio parou"));
            }
            state = incoming.arrived.wait_timeout(state, left).unwrap_or_else(|e| e.into_inner()).0;
        }
    }
}

impl Seek for IncomingReader {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let target = match to {
            SeekFrom::Start(n) => Some(n),
            SeekFrom::End(delta) => self.incoming.total.checked_add_signed(delta),
            SeekFrom::Current(delta) => self.pos.checked_add_signed(delta),
        };
        self.pos = target.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "posição inválida"))?;
        Ok(self.pos)
    }
}

/// Um pedaço a baixar: um intervalo de um link, ou um link inteiro (pedaço de HLS).
#[derive(Debug, Clone)]
pub struct Part {
    url: Arc<str>,
    range: Option<(u64, u64)>,
}

impl Part {
    pub fn whole(url: impl Into<Arc<str>>) -> Self {
        Self { url: url.into(), range: None }
    }

    pub fn range(url: impl Into<Arc<str>>, start: u64, end: u64) -> Self {
        Self { url: url.into(), range: Some((start, end)) }
    }
}

/// Os intervalos de `[from, total)` de um link, em pedaços de [`CHUNK`].
fn ranges_of(url: &str, from: u64, total: u64) -> Vec<Part> {
    let url: Arc<str> = url.into();
    chunk_ranges(total.saturating_sub(from), CHUNK)
        .into_iter()
        .map(|(start, end)| Part::range(url.clone(), from + start, from + end))
        .collect()
}

/// Começa a baixar e volta assim que o **primeiro pedaço** chegou; o resto segue em segundo plano.
/// Quando ninguém mais segura o `Incoming` (a pessoa pulou a faixa), o download para.
pub async fn stream(http: &reqwest::Client, url: &str) -> Result<Arc<Incoming>> {
    let probe = http.get(url).header(RANGE, "bytes=0-0").send().await?.error_for_status()?;
    let total = probe.headers().get(CONTENT_RANGE).and_then(|v| v.to_str().ok()).and_then(content_range_total);
    let total = match (probe.status(), total) {
        (StatusCode::PARTIAL_CONTENT, Some(total)) if total > 0 => total,
        // Servidor sem suporte a Range: baixa de uma vez.
        _ => {
            let bytes = download(http, url).await?;
            return Ok(Incoming::new(bytes.len() as u64, bytes, true));
        }
    };
    if total > MAX_BYTES {
        return Err(Error::Other(format!("arquivo de áudio grande demais ({} MB)", total / 1024 / 1024)));
    }
    let incoming = Incoming::new(total, Vec::with_capacity(total as usize), false);
    start_filling(http, ranges_of(url, 0, total), &incoming).await?;
    Ok(incoming)
}

/// Como [`stream`], para áudio dividido em vários arquivos (HLS), de tamanho total desconhecido.
pub async fn stream_parts(http: &reqwest::Client, parts: Vec<Part>) -> Result<Arc<Incoming>> {
    let incoming = Incoming::new(UNKNOWN_LEN, Vec::new(), false);
    start_filling(http, parts, &incoming).await?;
    Ok(incoming)
}

/// Baixa todos os pedaços (4 em paralelo, juntados na ordem) e devolve o arquivo inteiro.
pub async fn download_parts(http: &reqwest::Client, parts: Vec<Part>) -> Result<Vec<u8>> {
    let incoming = stream_parts(http, parts).await?;
    incoming.complete().await
}

/// Dispara o download dos `parts` e espera o primeiro chegar (erro aqui = erro para quem chamou).
async fn start_filling(http: &reqwest::Client, parts: Vec<Part>, incoming: &Arc<Incoming>) -> Result<()> {
    let (first_tx, first_rx) = tokio::sync::oneshot::channel::<std::result::Result<(), String>>();
    tokio::spawn(fill(http.clone(), parts, Arc::downgrade(incoming), Some(first_tx)));
    match first_rx.await {
        Ok(Ok(())) | Err(_) => Ok(()),
        Ok(Err(e)) => Err(Error::Other(e)),
    }
}

/// Baixa os pedaços (4 em paralelo, entregues na ordem) e vai anexando ao `Incoming`.
async fn fill(
    http: reqwest::Client,
    parts: Vec<Part>,
    incoming: Weak<Incoming>,
    first: Option<tokio::sync::oneshot::Sender<std::result::Result<(), String>>>,
) {
    let mut first = first;
    let work = async {
        let mut chunks = stream::iter(parts).map(|part| fetch_part(http.clone(), part)).buffered(PARALLEL);
        while let Some(chunk) = chunks.next().await {
            let Some(target) = incoming.upgrade() else { return Ok(()) }; // ninguém mais quer
            let bytes = chunk?;
            if target.arrived() + bytes.len() as u64 > MAX_BYTES {
                return Err(Error::Other("arquivo de áudio grande demais".into()));
            }
            target.update(|s| s.data.extend_from_slice(&bytes));
            if let Some(tx) = first.take() {
                let _ = tx.send(Ok(()));
            }
        }
        Ok::<_, Error>(())
    };
    let outcome = match tokio::time::timeout(DOWNLOAD_TIMEOUT, work).await {
        Ok(result) => result.map_err(|e| e.to_string()),
        Err(_) => Err("o download do áudio demorou demais".to_owned()),
    };
    if let Some(tx) = first.take() {
        // Falhou antes do primeiro pedaço: quem chamou recebe o erro (e pode tentar outro link).
        let _ = tx.send(outcome.clone());
    }
    if let Some(target) = incoming.upgrade() {
        match outcome {
            Ok(()) => target.finish(),
            Err(e) => target.update(|s| s.error = Some(e)),
        }
    }
}

async fn fetch_part(http: reqwest::Client, part: Part) -> Result<Vec<u8>> {
    let mut request = http.get(&*part.url);
    if let Some((start, end)) = part.range {
        request = request.header(RANGE, format!("bytes={start}-{end}"));
    }
    Ok(request.send().await?.error_for_status()?.bytes().await?.to_vec())
}

/// "bytes 0-0/3456789" → 3456789.
fn content_range_total(header: &str) -> Option<u64> {
    header.rsplit('/').next()?.trim().parse().ok()
}

/// Divide `[0, total)` em intervalos inclusivos de até `chunk` bytes.
fn chunk_ranges(total: u64, chunk: u64) -> Vec<(u64, u64)> {
    (0..total).step_by(chunk as usize).map(|start| (start, (start + chunk).min(total) - 1)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_total_from_content_range() {
        assert_eq!(content_range_total("bytes 0-0/3456789"), Some(3_456_789));
        assert_eq!(content_range_total("bytes 0-0/*"), None, "tamanho desconhecido");
    }

    #[test]
    fn reader_waits_for_bytes_that_arrive_later() {
        let incoming = Incoming::new(6, Vec::new(), false);
        let mut reader = incoming.reader();
        let writer = incoming.clone();
        let feeder = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            writer.update(|s| s.data.extend_from_slice(b"abc"));
            std::thread::sleep(Duration::from_millis(30));
            writer.update(|s| {
                s.data.extend_from_slice(b"def");
                s.done = true;
            });
        });
        let mut all = Vec::new();
        reader.read_to_end(&mut all).unwrap();
        feeder.join().unwrap();
        assert_eq!(all, b"abcdef");
    }

    #[test]
    fn completion_hook_gets_the_whole_file_once() {
        let incoming = Incoming::new(6, b"abc".to_vec(), false);
        let (tx, rx) = std::sync::mpsc::channel();
        incoming.when_complete(move |data| tx.send(data).unwrap());
        assert!(rx.try_recv().is_err(), "ainda não terminou");
        incoming.update(|s| s.data.extend_from_slice(b"def"));
        incoming.finish();
        assert_eq!(rx.recv().unwrap(), b"abcdef");

        let (tx, rx) = std::sync::mpsc::channel();
        incoming.when_complete(move |data| tx.send(data).unwrap());
        assert_eq!(rx.recv().unwrap(), b"abcdef", "pedir depois de terminar também entrega");
    }

    #[test]
    fn reader_reports_a_failed_download() {
        let incoming = Incoming::new(10, b"abc".to_vec(), false);
        incoming.update(|s| s.error = Some("403".into()));
        let mut reader = incoming.reader();
        let mut buf = [0u8; 8];
        assert_eq!(reader.read(&mut buf).unwrap(), 3, "o que já chegou é entregue");
        assert!(reader.read(&mut buf).is_err(), "depois, o erro do download aparece");
    }

    #[test]
    fn ranges_cover_the_file_in_chunks() {
        let parts = ranges_of("https://x/a.m4a", 0, 3_449_447);
        assert_eq!(parts[0].range, Some((0, 1_048_575)));
        assert_eq!(parts.last().unwrap().range, Some((3_145_728, 3_449_446)));
    }

    #[test]
    fn splits_into_inclusive_ranges_covering_everything() {
        assert_eq!(chunk_ranges(10, 4), [(0, 3), (4, 7), (8, 9)]);
        assert_eq!(chunk_ranges(8, 4), [(0, 3), (4, 7)]);
        assert_eq!(chunk_ranges(1, 4), [(0, 0)]);
        let total: u64 = chunk_ranges(3_456_789, CHUNK).iter().map(|(a, b)| b - a + 1).sum();
        assert_eq!(total, 3_456_789);
    }
}

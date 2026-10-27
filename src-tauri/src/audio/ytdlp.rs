//! `yt-dlp`: descobre a URL do áudio de um vídeo do YouTube.
//!
//! O YouTube embaralha as URLs de mídia e muda as regras com frequência; o
//! yt-dlp é mantido justamente para acompanhar isso. Nós só pedimos a URL
//! (`-g`) do melhor áudio em m4a/AAC — formato que o symphonia decodifica.
//!
//! Usamos a versão "onedir" (executável + pasta `_internal/`): a de arquivo único se
//! descompacta a cada execução e gastava ~1 s a mais por música (ver `scripts/fetch-yt-dlp.mjs`).
//! Onde procuramos, em ordem:
//! 1. nos recursos do app instalado (`<recursos>/yt-dlp/`, copiado pelo instalador);
//! 2. em `src-tauri/binaries/yt-dlp/` (desenvolvimento; `npm run setup:yt-dlp`);
//! 3. no PATH do sistema.
//!
//! As URLs resolvidas ficam num cache **em disco**, válido até perto do `expire=` que o
//! próprio YouTube põe na URL (~6 h): uma música tocada há pouco abre rápido mesmo depois
//! de fechar e abrir o app.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures::future::{BoxFuture, FutureExt, Shared, WeakShared};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

const TIMEOUT: Duration = Duration::from_secs(40);
/// m4a (AAC) primeiro; Opus/WebM o symphonia não decodifica.
const FORMAT: &str = "bestaudio[ext=m4a]/bestaudio[acodec^=mp4a]/bestaudio[acodec=mp3]";

/// Margem antes do `expire=` da URL: uma música começada perto do fim ainda termina de baixar.
const EXPIRY_MARGIN_SECS: u64 = 30 * 60;
/// Se a URL não disser quando expira, guardamos por 1 h.
const DEFAULT_TTL_SECS: u64 = 60 * 60;
/// Teto do cache em disco (cada entrada tem ~1 KB).
const CACHE_MAX_ENTRIES: usize = 500;

/// Uma chamada ao yt-dlp em andamento, que vários pedidos podem esperar juntos.
type Job = Shared<BoxFuture<'static, std::result::Result<String, String>>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CachedUrl {
    url: String,
    /// Unix (segundos) até quando vale a pena usar a URL.
    valid_until: u64,
}

pub struct YtDlp {
    program: PathBuf,
    /// videoId → URL do áudio. Cada chamada ao yt-dlp custa ~4 s.
    cache: Mutex<HashMap<String, CachedUrl>>,
    /// Onde o cache é gravado (`None` = só em memória).
    cache_file: Option<PathBuf>,
    /// Chamadas em andamento. Pedir o mesmo vídeo de novo (duplo clique, pré-carga + play)
    /// espera a mesma chamada em vez de abrir outro yt-dlp. A referência é **fraca**: quando
    /// ninguém mais espera (a pessoa pulou a faixa), a chamada é descartada e o processo
    /// encerrado (`kill_on_drop`), sem ficar disputando CPU e rede com a faixa nova.
    inflight: Mutex<HashMap<String, WeakShared<BoxFuture<'static, std::result::Result<String, String>>>>>,
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Até quando usar `url`: o `expire=` do YouTube menos a margem (ou 1 h, se não houver).
fn valid_until(url: &str, now: u64) -> u64 {
    let expire = url::Url::parse(url)
        .ok()
        .and_then(|u| u.query_pairs().find(|(k, _)| k == "expire").and_then(|(_, v)| v.parse::<u64>().ok()));
    match expire {
        Some(expire) => expire.saturating_sub(EXPIRY_MARGIN_SECS),
        None => now + DEFAULT_TTL_SECS,
    }
}

impl YtDlp {
    /// `resource_dir`: pasta de recursos do app instalado. `cache_file`: onde guardar as URLs.
    pub fn new(resource_dir: Option<&Path>, cache_file: Option<PathBuf>) -> Self {
        let name = if cfg!(windows) { "yt-dlp.exe" } else { "yt-dlp" };
        let bundled = resource_dir.map(|dir| dir.join("yt-dlp").join(name));
        let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries").join("yt-dlp").join(name);
        let program = [bundled, Some(dev)].into_iter().flatten().find(|p| p.is_file()).unwrap_or_else(|| name.into());
        log::info!("yt-dlp: {}", program.display());
        sweep_cookie_files();
        let cache = cache_file.as_deref().map(load_cache).unwrap_or_default();
        Self { program, cache: Mutex::new(cache), cache_file, inflight: Mutex::new(HashMap::new()) }
    }

    /// Esquece a URL guardada (ex.: o download com ela falhou; o IP mudou).
    pub fn forget(&self, video_id: &str) {
        if let Ok(mut cache) = self.cache.lock() {
            if cache.remove(video_id).is_some() {
                self.save(&cache);
            }
        }
    }

    /// `cookies`: conteúdo de um cookies.txt da sessão do YouTube da pessoa. Desde 2025 o YouTube
    /// recusa ("Sign in to confirm you're not a bot") quem pede o áudio sem estar logado.
    pub async fn audio_url(&self, video_id: &str, cookies: Option<&str>) -> Result<String> {
        if let Some(cached) = self.cache.lock().ok().and_then(|c| c.get(video_id).cloned()) {
            if cached.valid_until > now_secs() {
                return Ok(cached.url);
            }
        }
        let url = self.job(video_id, cookies).await.map_err(Error::Other)?;
        if let Ok(mut cache) = self.cache.lock() {
            let entry = CachedUrl { valid_until: valid_until(&url, now_secs()), url: url.clone() };
            cache.insert(video_id.to_owned(), entry);
            self.save(&cache);
        }
        Ok(url)
    }

    /// Já tem URL válida guardada (tocar esta faixa não espera o yt-dlp)?
    pub fn is_resolved(&self, video_id: &str) -> bool {
        self.cache.lock().ok().and_then(|c| c.get(video_id).map(|e| e.valid_until > now_secs())).unwrap_or(false)
    }

    /// Grava o cache sem as URLs vencidas. Arquivo pequeno (centenas de KB no máximo).
    fn save(&self, cache: &HashMap<String, CachedUrl>) {
        let Some(path) = &self.cache_file else { return };
        let now = now_secs();
        let mut live: Vec<_> = cache.iter().filter(|(_, e)| e.valid_until > now).collect();
        live.sort_by_key(|(_, e)| std::cmp::Reverse(e.valid_until));
        live.truncate(CACHE_MAX_ENTRIES);
        let live: HashMap<_, _> = live.into_iter().collect();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_vec(&live) {
            if let Err(e) = std::fs::write(path, json) {
                log::info!("não gravei o cache de URLs do YouTube: {e}");
            }
        }
    }

    /// A chamada em andamento para `video_id`, ou uma nova.
    fn job(&self, video_id: &str, cookies: Option<&str>) -> Job {
        let mut inflight = self.inflight.lock().unwrap_or_else(|e| e.into_inner());
        inflight.retain(|_, job| job.upgrade().is_some());
        if let Some(job) = inflight.get(video_id).and_then(WeakShared::upgrade) {
            return job;
        }
        let job = resolve(self.program.clone(), video_id.to_owned(), cookies.map(str::to_owned))
            .map(|result| result.map_err(|e| e.to_string()))
            .boxed()
            .shared();
        if let Some(weak) = job.downgrade() {
            inflight.insert(video_id.to_owned(), weak);
        }
        job
    }
}

const COOKIE_FILE_PREFIX: &str = "globalmusic-cookies-";

/// Apaga cookies temporários que sobraram (ex.: o app fechou com um yt-dlp rodando).
fn sweep_cookie_files() {
    let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else { return };
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().starts_with(COOKIE_FILE_PREFIX) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

fn load_cache(path: &Path) -> HashMap<String, CachedUrl> {
    let now = now_secs();
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<HashMap<String, CachedUrl>>(&bytes).ok())
        .map(|cache| cache.into_iter().filter(|(_, e)| e.valid_until > now).collect())
        .unwrap_or_default()
}

async fn resolve(program: PathBuf, video_id: String, cookies: Option<String>) -> Result<String> {
    let page = format!("https://music.youtube.com/watch?v={video_id}");
    // Arquivo temporário só durante a chamada: o yt-dlp lê (e reescreve) os cookies nele.
    let cookie_file = match cookies.as_deref() {
        Some(content) => {
            let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
            let path = std::env::temp_dir().join(format!("{COOKIE_FILE_PREFIX}{}-{nanos}.txt", std::process::id()));
            std::fs::write(&path, content).map_err(|e| Error::Other(format!("não gravei os cookies para o yt-dlp: {e}")))?;
            Some(TempFile(path))
        }
        None => None,
    };

    let mut command = tokio::process::Command::new(&program);
    command.args(["--no-playlist", "--no-warnings", "--quiet", "--js-runtimes", "node", "-f", FORMAT]);
    if let Some(file) = &cookie_file {
        command.arg("--cookies").arg(&file.0);
    }
    command.args(["-g", &page]).stdin(Stdio::null()).kill_on_drop(true);
    // Sem isso, cada chamada piscaria uma janela de terminal no Windows.
    #[cfg(windows)]
    command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW

    let output = tokio::time::timeout(TIMEOUT, command.output()).await;
    let output = output
        .map_err(|_| Error::Timeout)?
        .map_err(|e| Error::Other(format!("não consegui executar o yt-dlp ({}): {e}", program.display())))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        log::warn!("yt-dlp falhou para {video_id} (cookies: {}): {}", cookie_file.is_some(), stderr.trim());
        let reason = stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("erro desconhecido");
        if reason.contains("not a bot") {
            return Err(Error::Other(if cookie_file.is_some() {
                "o YouTube recusou a sessão salva; desconecte e conecte o YouTube Music de novo no Perfil".into()
            } else {
                "o YouTube exige login para tocar: conecte o YouTube Music no Perfil".into()
            }));
        }
        return Err(Error::Other(format!("yt-dlp: {reason}")));
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("http"))
        .map(str::to_owned)
        .ok_or_else(|| Error::Other("yt-dlp não devolveu URL de áudio".into()))
}

/// Apaga o arquivo quando sai de escopo — inclusive se a chamada for cancelada no meio
/// (a pessoa pulou a faixa): os cookies da sessão não podem ficar esquecidos na pasta temporária.
struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_the_expiry_youtube_puts_in_the_url() {
        let url = "https://rr1.googlevideo.com/videoplayback?expire=1800000000&ei=x&itag=140";
        assert_eq!(valid_until(url, 1_799_990_000), 1_800_000_000 - EXPIRY_MARGIN_SECS);
        assert_eq!(valid_until("https://example.com/audio.m4a", 1000), 1000 + DEFAULT_TTL_SECS, "sem expire: 1 h");
    }

    #[test]
    fn disk_cache_round_trips_and_drops_expired_urls() {
        let dir = std::env::temp_dir().join(format!("gm-ytdlp-cache-test-{}", std::process::id()));
        let file = dir.join("youtube-urls.json");
        let yt = YtDlp::new(None, Some(file.clone()));
        {
            let mut cache = yt.cache.lock().unwrap();
            cache.insert("vivo".into(), CachedUrl { url: "https://a".into(), valid_until: now_secs() + 600 });
            cache.insert("vencido".into(), CachedUrl { url: "https://b".into(), valid_until: now_secs() - 1 });
            yt.save(&cache);
        }
        let reopened = YtDlp::new(None, Some(file));
        assert!(reopened.is_resolved("vivo"));
        assert!(!reopened.is_resolved("vencido"));
        let _ = std::fs::remove_dir_all(dir);
    }
}

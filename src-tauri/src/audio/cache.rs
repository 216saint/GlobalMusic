//! Cache de áudio em disco: música ouvida (ou pré-carregada) fica guardada; na próxima vez ela toca
//! na hora, sem rede e sem yt-dlp.
//!
//! Leve de propósito:
//! - **teto de 300 MB** (~70 músicas). Passou disso, saem as ouvidas há mais tempo: cada vez que
//!   uma música toca do cache, a data do arquivo é atualizada, então a limpeza tira as "esquecidas";
//! - só guarda download **completo** (faixa pulada no meio não ocupa espaço);
//! - nada fica na RAM: a pré-carga da próxima faixa também vai para o disco.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use sha1::{Digest, Sha1};

const MAX_BYTES: u64 = 300 * 1024 * 1024;
/// A limpeza desce um pouco abaixo do teto, para não rodar a cada música nova.
const EVICT_TO: u64 = MAX_BYTES / 10 * 9;
/// Formatos que guardamos (a extensão do arquivo diz ao decodificador o que ele é).
const HINTS: [&str; 2] = ["m4a", "mp3"];
const PARTIAL_SUFFIX: &str = ".part";

#[derive(Debug, Clone)]
pub struct DiskCache {
    dir: Option<PathBuf>,
}

impl DiskCache {
    /// `dir = None` desliga o cache (ex.: sem pasta de cache no sistema).
    pub fn new(dir: Option<PathBuf>) -> Self {
        if let Some(dir) = &dir {
            let _ = fs::create_dir_all(dir);
            // Gravação interrompida (app fechado no meio): o arquivo pela metade não serve.
            for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
                if entry.file_name().to_string_lossy().ends_with(PARTIAL_SUFFIX) {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
        Self { dir }
    }

    /// Arquivo de uma faixa: hash da identidade dela (fonte, id, rota), nunca o título.
    fn file(&self, key: &str, hint: &str) -> Option<PathBuf> {
        let name: String = Sha1::digest(key.as_bytes()).iter().map(|b| format!("{b:02x}")).collect();
        Some(self.dir.as_ref()?.join(format!("{name}.{hint}")))
    }

    /// Esquece a música guardada (ex.: a pessoa escolheu outra versão dela).
    pub fn remove(&self, key: &str) {
        for hint in HINTS {
            if let Some(path) = self.file(key, hint) {
                let _ = fs::remove_file(path);
            }
        }
    }

    pub fn contains(&self, key: &str) -> bool {
        HINTS.iter().any(|hint| self.file(key, hint).is_some_and(|p| p.is_file()))
    }

    /// O áudio guardado e o formato dele.
    pub async fn get(&self, key: &str) -> Option<(Vec<u8>, &'static str)> {
        let (cache, key) = (self.clone(), key.to_owned());
        tokio::task::spawn_blocking(move || {
            HINTS.into_iter().find_map(|hint| {
                let path = cache.file(&key, hint)?;
                let bytes = fs::read(&path).ok()?;
                // Ouvida agora: a limpeza tira primeiro as que estão há mais tempo sem tocar.
                let _ = fs::File::options().write(true).open(&path).and_then(|f| f.set_modified(SystemTime::now()));
                Some((bytes, hint))
            })
        })
        .await
        .ok()
        .flatten()
    }

    /// Guarda em segundo plano (não atrasa quem chamou).
    pub fn put(&self, key: &str, bytes: Vec<u8>, hint: &'static str) {
        let (cache, key) = (self.clone(), key.to_owned());
        tokio::task::spawn_blocking(move || {
            if let Err(e) = cache.write(&key, &bytes, hint) {
                log::info!("não guardei a música no cache: {e}");
            }
        });
    }

    fn write(&self, key: &str, bytes: &[u8], hint: &str) -> io::Result<()> {
        let (Some(dir), Some(path)) = (self.dir.as_deref(), self.file(key, hint)) else { return Ok(()) };
        // Grava com outro nome e renomeia: quem ler nunca pega o arquivo pela metade.
        let partial = path.with_file_name(format!("{}{PARTIAL_SUFFIX}", path.file_name().unwrap_or_default().to_string_lossy()));
        fs::write(&partial, bytes)?;
        fs::rename(&partial, &path)?;
        evict(dir, MAX_BYTES, EVICT_TO)
    }
}

/// Se a pasta passou de `max`, apaga as músicas tocadas há mais tempo até ficar em `target`.
fn evict(dir: &Path, max: u64, target: u64) -> io::Result<()> {
    let mut files: Vec<(SystemTime, u64, PathBuf)> = fs::read_dir(dir)?
        .flatten()
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            meta.is_file().then(|| (meta.modified().unwrap_or(SystemTime::UNIX_EPOCH), meta.len(), entry.path()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|(_, len, _)| len).sum();
    if total <= max {
        return Ok(());
    }
    files.sort_by_key(|(modified, _, _)| *modified);
    for (_, len, path) in files {
        if total <= target {
            break;
        }
        if fs::remove_file(&path).is_ok() {
            total -= len;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("gm-audio-cache-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[tokio::test]
    async fn keeps_the_audio_and_its_format() {
        let dir = temp_dir("roundtrip");
        let cache = DiskCache::new(Some(dir.clone()));
        assert!(cache.get("youtube:abc").await.is_none());
        cache.write("youtube:abc", b"audio", "m4a").unwrap();
        assert!(cache.contains("youtube:abc"));
        assert_eq!(cache.get("youtube:abc").await, Some((b"audio".to_vec(), "m4a")));
        let names: Vec<String> = fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert!(names.iter().all(|n| !n.contains("abc")), "o nome do arquivo não revela a música");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn evicts_the_least_recently_played_first() {
        let dir = temp_dir("evict");
        fs::create_dir_all(&dir).unwrap();
        let now = SystemTime::now();
        for (name, age_secs) in [("velha.m4a", 300), ("media.m4a", 200), ("nova.m4a", 100)] {
            let path = dir.join(name);
            fs::write(&path, [0u8; 10]).unwrap();
            fs::File::options().write(true).open(&path).unwrap().set_modified(now - Duration::from_secs(age_secs)).unwrap();
        }
        evict(&dir, 25, 20).unwrap(); // 30 bytes > 25: apaga até ficar em 20
        assert!(!dir.join("velha.m4a").exists());
        assert!(dir.join("media.m4a").exists() && dir.join("nova.m4a").exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn leftovers_of_an_interrupted_write_are_removed() {
        let dir = temp_dir("partial");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("x.m4a.part"), b"meia").unwrap();
        DiskCache::new(Some(dir.clone()));
        assert!(!dir.join("x.m4a.part").exists());
        let _ = fs::remove_dir_all(dir);
    }
}

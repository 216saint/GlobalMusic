//! Motor de áudio: transforma "toque esta faixa" em som, seja qual for a fonte.
//! (Ver docs/decisoes/0002-audio-no-rust.md)
//!
//! ```text
//! Spotify Premium ─► librespot ───────────────────────────────────────────► som
//! Spotify Free ────► resolver acha no YouTube ─┐
//! YouTube Music ───────────────────────────────┴► yt-dlp (URL m4a) ─┐
//! SoundCloud ──────► API (MP3 progressivo ou HLS de MP3) ───────────┴► download paralelo ► rodio ► som
//! ```
//!
//! **Gerações:** cada "carregar faixa" ganha um número crescente. Resolver uma
//! faixa leva segundos (yt-dlp, rede); se a pessoa pular de novo nesse meio
//! tempo, o carregamento antigo é **cancelado na hora** (`superseded`): o yt-dlp
//! dele é encerrado e o download para, sem disputar CPU e rede com a faixa nova.
//!
//! **Tocar antes de baixar tudo:** sem pré-carga, a faixa começa assim que o primeiro
//! pedaço chega (`fetch::stream`), sem seek. Se a pessoa arrastar a barra, esperamos o resto
//! (normalmente já chegou) e reabrimos o arquivo inteiro com seek (`reopen_seekable`).
//!
//! **Cache em disco** (`cache.rs`): toda música baixada por inteiro fica guardada (até 300 MB);
//! na próxima vez ela toca na hora, sem rede. Leve: nada fica na RAM.
//!
//! **Pré-carregamento:** enquanto uma faixa toca, as duas próximas da fila já são
//! baixadas para o cache em disco (`preload`), e a terceira tem a URL resolvida
//! (`warm`). Pular para qualquer uma delas não espera o yt-dlp.

mod cache;
pub mod fetch;
mod matches;
mod output;
#[cfg(feature = "spotify-premium")]
mod spotify_native;
mod ytdlp;

use std::collections::HashSet;
use std::io::{Cursor, Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rodio::Decoder;
use tokio::sync::{watch, Mutex, Semaphore};

use crate::error::{Error, Result};
use crate::models::{Source, Track};
use crate::player::resolver;
use crate::player::routing::PlaybackRoute;
use crate::providers::{MusicProvider, Providers, SoundcloudStreamKind};
use cache::DiskCache;
use matches::Matches;
use output::{BoxedSource, Output};
use ytdlp::YtDlp;

/// Navegadores mandam User-Agent; alguns CDNs de áudio recusam quem não manda.
const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36";

#[derive(Debug, Clone)]
pub enum AudioEvent {
    /// A faixa da geração `.0` terminou.
    Ended(u64),
    /// Não deu para tocar a faixa da geração `.0`.
    Failed(u64, String),
    /// O Spotify (librespot) recusou a faixa da geração `.0`: vale tentar a mesma música pelo YouTube.
    NativeUnavailable(u64),
}

/// Para onde o motor manda os eventos (fim de faixa, falha).
pub type EventSink = Arc<dyn Fn(AudioEvent) + Send + Sync>;

/// Por que estamos preparando faixas antes do clique. Cada motivo tem a sua "vez":
/// um pedido novo do mesmo motivo abandona o que faltava do anterior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrepareReason {
    /// A lista que apareceu na tela (busca, playlist).
    Visible,
    /// A faixa em que o mouse parou.
    Hover,
}

/// Recusas do librespot até desistir dele (ver `Audio::note_native_refused`). Uma basta: a
/// recusa da chave de áudio vale para todas as faixas, e cada tentativa custa ~3 s.
const NATIVE_REFUSALS_TO_GIVE_UP: u32 = 1;
/// Depois de desistir, por quanto tempo nem tentamos (vale entre aberturas do app).
const NATIVE_OFF_FOR: Duration = Duration::from_secs(24 * 60 * 60);

/// yt-dlp em segundo plano ao mesmo tempo, no máximo. Mais que isso pesa no computador
/// (cada um usa ~1 núcleo por 1–2 s) e chama atenção do YouTube.
const BACKGROUND_YTDLP: usize = 2;

/// Avança um contador de "vez" e devolve o valor novo.
fn bump(counter: &watch::Sender<u64>) -> u64 {
    let mut next = 0;
    counter.send_modify(|value| {
        *value += 1;
        next = *value;
    });
    next
}

/// Termina quando o contador sair de `value` (chegou uma vez mais nova).
async fn moved_on(counter: &watch::Sender<u64>, value: u64) {
    let mut rx = counter.subscribe();
    let _ = rx.wait_for(|v| *v != value).await;
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Active {
    Nothing,
    Stream,
    #[cfg(feature = "spotify-premium")]
    Spotify,
}

/// Áudio pronto para decodificar: inteiro na memória, ou ainda chegando.
enum Opened {
    Full(Vec<u8>, &'static str),
    Streaming(Arc<fetch::Incoming>, &'static str),
}

pub struct Audio {
    output: Output,
    #[cfg(feature = "spotify-premium")]
    spotify: Mutex<Option<spotify_native::SpotifyNative>>,
    active: Mutex<Active>,
    /// Canal (e não só um número) para quem está carregando poder *esperar* a troca de faixa.
    generation: watch::Sender<u64>,
    http: reqwest::Client,
    ytdlp: YtDlp,
    providers: Providers,
    events: EventSink,
    /// Músicas já baixadas (inclusive a próxima da fila, pré-carregada).
    disk: DiskCache,
    /// Faixa do Spotify → vídeo equivalente no YouTube (salvo em disco; a escolha da pessoa vence).
    matches: Matches,
    /// A faixa atual está tocando de um download ainda em andamento (sem seek).
    streaming: Mutex<Option<(Arc<fetch::Incoming>, &'static str)>>,
    /// Quantas vezes o Spotify recusou tocar pelo librespot.
    native_refusals: AtomicU32,
    /// Marca em disco de "o Spotify está recusando o librespot" (a data do arquivo diz desde quando).
    native_off_marker: Option<PathBuf>,
    /// Vagas para yt-dlp em segundo plano (preparar antes do clique, próximas da fila).
    background_slots: Semaphore,
    /// Pré-cargas em andamento (pela chave do cache): pedir a mesma de novo não baixa duas vezes.
    preloading: std::sync::Mutex<HashSet<String>>,
    /// A "vez" de cada motivo de preparação (ver `PrepareReason`).
    visible_turn: watch::Sender<u64>,
    hover_turn: watch::Sender<u64>,
}

impl Audio {
    /// `resource_dir`: recursos do app instalado (onde fica o yt-dlp). `cache_dir`: cache do app
    /// (pode ser apagado pelo sistema). `data_dir`: dados da pessoa (as versões que ela escolheu).
    pub fn new(
        providers: Providers,
        events: EventSink,
        resource_dir: Option<&Path>,
        cache_dir: Option<PathBuf>,
        data_dir: Option<PathBuf>,
    ) -> Self {
        let native_off_marker = cache_dir.as_ref().map(|dir| dir.join("spotify-native-off"));
        Self {
            output: Output::spawn(events.clone()),
            #[cfg(feature = "spotify-premium")]
            spotify: Mutex::new(None),
            active: Mutex::new(Active::Nothing),
            generation: watch::Sender::new(0),
            http: reqwest::Client::builder()
                .user_agent(USER_AGENT)
                .connect_timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
            ytdlp: YtDlp::new(resource_dir, cache_dir.as_ref().map(|dir| dir.join("youtube-urls.json"))),
            providers,
            events,
            disk: DiskCache::new(cache_dir.as_ref().map(|dir| dir.join("audio"))),
            matches: Matches::load(data_dir.map(|dir| dir.join("youtube-matches.json"))),
            streaming: Mutex::new(None),
            native_refusals: AtomicU32::new(if native_recently_refused(native_off_marker.as_deref()) {
                NATIVE_REFUSALS_TO_GIVE_UP
            } else {
                0
            }),
            native_off_marker,
            background_slots: Semaphore::new(BACKGROUND_YTDLP),
            preloading: std::sync::Mutex::new(HashSet::new()),
            visible_turn: watch::Sender::new(0),
            hover_turn: watch::Sender::new(0),
        }
    }

    /// Começa uma nova geração (invalida qualquer carregamento em andamento).
    pub fn begin(&self) -> u64 {
        bump(&self.generation)
    }

    pub fn current_generation(&self) -> u64 {
        *self.generation.borrow()
    }

    pub fn is_current(&self, generation: u64) -> bool {
        self.current_generation() == generation
    }

    /// Termina quando outra faixa é pedida depois de `generation`.
    pub async fn superseded(&self, generation: u64) {
        moved_on(&self.generation, generation).await;
    }

    /// Resolve com antecedência as faixas que a pessoa provavelmente vai tocar, uma de cada vez.
    /// Se ela clicar numa delas no meio do caminho, o play espera a mesma chamada do yt-dlp.
    pub async fn prepare(&self, tracks: Vec<(Track, PlaybackRoute)>, reason: PrepareReason) {
        let turn = match reason {
            PrepareReason::Visible => &self.visible_turn,
            PrepareReason::Hover => &self.hover_turn,
        };
        let mine = bump(turn);
        let work = async {
            for (track, route) in &tracks {
                self.warm(track, *route).await;
            }
        };
        tokio::select! {
            () = work => {}
            () = moved_on(turn, mine) => {}
        }
    }

    /// Publica um evento como se viesse do próprio motor (ex.: falha ao resolver a faixa).
    pub fn report(&self, event: AudioEvent) {
        (self.events)(event);
    }

    pub async fn is_active(&self) -> bool {
        *self.active.lock().await != Active::Nothing
    }

    /// Resolve e começa a tocar `track`. Volta quando o som começou (ou falhou).
    pub async fn load(&self, generation: u64, track: Track, route: PlaybackRoute, start_ms: u64, volume: f32) -> Result<()> {
        // Para o que estiver tocando já: a música velha não deve continuar enquanto a nova carrega.
        self.stop().await;
        log::info!("[{generation}] carregando \"{}\" ({}) pela rota {route:?}", track.title, track.source.label());
        let started = std::time::Instant::now();
        let result = self.load_route(generation, &track, route, start_ms, volume).await;
        let secs = started.elapsed().as_secs_f32();
        match &result {
            Ok(()) if self.is_current(generation) => log::info!("[{generation}] tocando \"{}\" ({secs:.1} s)", track.title),
            Ok(()) => log::info!("[{generation}] cancelado \"{}\": outra faixa foi pedida", track.title),
            Err(e) => log::warn!("[{generation}] falhou \"{}\" ({secs:.1} s): {e}", track.title),
        }
        result
    }

    /// Spotify sem Premium (ou recusado pelo librespot): mesma gravação, áudio do YouTube.
    pub async fn load_fallback(&self, generation: u64, track: &Track, start_ms: u64, volume: f32) -> Result<()> {
        self.load_route(generation, track, PlaybackRoute::YoutubeFallback, start_ms, volume).await
    }

    async fn load_route(&self, generation: u64, track: &Track, route: PlaybackRoute, start_ms: u64, volume: f32) -> Result<()> {
        #[cfg(feature = "spotify-premium")]
        if route == PlaybackRoute::SpotifyNative {
            match self.load_spotify(generation, track, start_ms, volume).await {
                Ok(()) => return Ok(()),
                // Se o librespot falhar, a música ainda toca pelo YouTube.
                Err(e) => log::warn!("[{generation}] librespot falhou, tentando pelo YouTube: {e}"),
            }
        }
        let route = if route == PlaybackRoute::SpotifyNative { PlaybackRoute::YoutubeFallback } else { route };

        let key = cache_key(track, route);
        let opened = match self.disk.get(&key).await {
            Some((bytes, hint)) => {
                log::info!("[{generation}] tocando do cache em disco");
                Opened::Full(bytes, hint)
            }
            // Se outra faixa for pedida no meio, larga o yt-dlp e o download na hora.
            None => match tokio::select! {
                opened = self.open_audio(track, route, start_ms) => opened?,
                () = self.superseded(generation) => return Ok(()),
            } {
                // Guarda no disco quando terminar de baixar (só se terminar: faixa pulada não ocupa espaço).
                Opened::Streaming(incoming, hint) => {
                    let disk = self.disk.clone();
                    incoming.when_complete(move |bytes| disk.put(&key, bytes, hint));
                    Opened::Streaming(incoming, hint)
                }
                Opened::Full(bytes, hint) => {
                    self.disk.put(&key, bytes.clone(), hint);
                    Opened::Full(bytes, hint)
                }
            },
        };
        if !self.is_current(generation) {
            return Ok(());
        }
        let source = match opened {
            Opened::Full(bytes, hint) => {
                *self.streaming.lock().await = None;
                let len = bytes.len() as u64;
                spawn_decode(Cursor::new(bytes), hint, Some(len), true).await?
            }
            Opened::Streaming(incoming, hint) => {
                let source = spawn_decode(incoming.reader(), hint, incoming.known_len(), false).await?;
                *self.streaming.lock().await = Some((incoming, hint));
                source
            }
        };
        self.play_stream(generation, source, start_ms, volume).await
    }

    /// Prepara a próxima faixa em segundo plano, para ela tocar sem espera.
    pub async fn preload(&self, track: Track, route: PlaybackRoute) {
        #[cfg(feature = "spotify-premium")]
        if route == PlaybackRoute::SpotifyNative {
            if let Some(native) = self.spotify.lock().await.as_ref() {
                native.preload(&track.id);
                return;
            }
        }
        let route = if route == PlaybackRoute::SpotifyNative { PlaybackRoute::YoutubeFallback } else { route };
        let key = cache_key(&track, route);
        if self.disk.contains(&key) {
            return;
        }
        let Some(_preloading) = Preloading::start(&self.preloading, &key) else { return };
        match self.fetch_audio(&track, route).await {
            Ok((bytes, hint)) => {
                log::info!("pré-carregada \"{}\" ({} KB, no disco)", track.title, bytes.len() / 1024);
                self.disk.put(&key, bytes, hint);
            }
            Err(e) => log::info!("não pré-carreguei \"{}\": {e}", track.title),
        }
    }

    /// Deixa só a URL do áudio resolvida (sem baixar): pular para esta faixa não espera o yt-dlp.
    pub async fn warm(&self, track: &Track, route: PlaybackRoute) {
        let video_id = match (route, track.source) {
            // librespot e SoundCloud não passam pelo yt-dlp.
            (PlaybackRoute::SpotifyNative, _) | (PlaybackRoute::DirectStream, Source::Soundcloud) => return,
            (PlaybackRoute::DirectStream, Source::YoutubeMusic) => track.id.clone(),
            _ => match self.youtube_equivalent(track).await {
                Ok(id) => id,
                Err(e) => return log::info!("não preparei \"{}\": {e}", track.title),
            },
        };
        if self.ytdlp.is_resolved(&video_id) {
            return;
        }
        let Ok(_slot) = self.background_slots.acquire().await else { return };
        let cookies = self.providers.youtube.ytdlp_cookies();
        if let Err(e) = self.ytdlp.audio_url(&video_id, cookies.as_deref()).await {
            log::info!("não preparei \"{}\": {e}", track.title);
        }
    }

    /// Abre o áudio para tocar já: começa com o primeiro pedaço baixado, quando dá.
    async fn open_audio(&self, track: &Track, route: PlaybackRoute, start_ms: u64) -> Result<Opened> {
        let opened = match (route, track.source) {
            (PlaybackRoute::DirectStream, Source::Soundcloud) => self.open_soundcloud(&track.id).await?,
            (PlaybackRoute::DirectStream, Source::YoutubeMusic) => self.stream_youtube(&track.id).await?,
            _ => {
                let video_id = self.youtube_equivalent(track).await?;
                self.stream_youtube(&video_id).await?
            }
        };
        // Começar do meio (ex.: retomando de onde parou) precisa de seek: espera o arquivo inteiro.
        match opened {
            Opened::Streaming(incoming, hint) if start_ms > 0 => Ok(Opened::Full(incoming.complete().await?, hint)),
            other => Ok(other),
        }
    }

    async fn stream_youtube(&self, video_id: &str) -> Result<Opened> {
        let cookies = self.providers.youtube.ytdlp_cookies();
        let url = self.ytdlp.audio_url(video_id, cookies.as_deref()).await?;
        match fetch::stream(&self.http, &url).await {
            Ok(incoming) => Ok(Opened::Streaming(incoming, "m4a")),
            Err(e) => {
                // URL em cache pode ter expirado (ou o IP mudou): pede uma nova ao yt-dlp e tenta mais uma vez.
                log::info!("download do YouTube falhou ({e}); renovando a URL");
                self.ytdlp.forget(video_id);
                let url = self.ytdlp.audio_url(video_id, cookies.as_deref()).await?;
                Ok(Opened::Streaming(fetch::stream(&self.http, &url).await?, "m4a"))
            }
        }
    }

    async fn open_soundcloud(&self, track_id: &str) -> Result<Opened> {
        let stream = self.providers.soundcloud.stream(track_id).await?;
        Ok(match stream.kind {
            SoundcloudStreamKind::Progressive => Opened::Streaming(fetch::stream(&self.http, &stream.url).await?, "mp3"),
            kind => {
                let parts = self.providers.soundcloud.hls_parts(&stream.url).await?;
                Opened::Streaming(fetch::stream_parts(&self.http, parts).await?, kind.hint())
            }
        })
    }

    /// A faixa começou sem seek (para tocar antes de baixar tudo). Para pular para outro ponto,
    /// espera o download terminar (costuma já ter terminado) e reabre o arquivo inteiro, com seek.
    async fn reopen_seekable(&self, incoming: Arc<fetch::Incoming>, hint: &'static str, position_ms: u64) {
        let generation = self.current_generation();
        let bytes = match incoming.complete().await {
            Ok(bytes) => bytes,
            Err(e) => return log::warn!("[{generation}] não deu para pular na faixa: {e}"),
        };
        if !self.is_current(generation) {
            return;
        }
        let len = bytes.len() as u64;
        match spawn_decode(Cursor::new(bytes), hint, Some(len), true).await {
            Ok(source) if self.is_current(generation) => self.output.reopen(source, Duration::from_millis(position_ms)),
            Ok(_) => {}
            Err(e) => log::warn!("[{generation}] não deu para reabrir a faixa com seek: {e}"),
        }
    }

    /// Baixa o áudio inteiro de uma faixa (sem tocar). Serve para pré-carregar.
    async fn fetch_audio(&self, track: &Track, route: PlaybackRoute) -> Result<(Vec<u8>, &'static str)> {
        match (route, track.source) {
            (PlaybackRoute::DirectStream, Source::Soundcloud) => self.fetch_soundcloud(&track.id).await,
            (PlaybackRoute::DirectStream, Source::YoutubeMusic) => self.fetch_youtube(&track.id).await,
            _ => {
                let video_id = self.youtube_equivalent(track).await?;
                self.fetch_youtube(&video_id).await
            }
        }
    }

    async fn fetch_youtube(&self, video_id: &str) -> Result<(Vec<u8>, &'static str)> {
        let cookies = self.providers.youtube.ytdlp_cookies();
        let url = self.ytdlp.audio_url(video_id, cookies.as_deref()).await?;
        match fetch::download(&self.http, &url).await {
            Ok(bytes) => Ok((bytes, "m4a")),
            Err(e) => {
                // URL em cache pode ter expirado: pede uma nova ao yt-dlp e tenta mais uma vez.
                log::info!("download do YouTube falhou ({e}); renovando a URL");
                self.ytdlp.forget(video_id);
                let url = self.ytdlp.audio_url(video_id, cookies.as_deref()).await?;
                Ok((fetch::download(&self.http, &url).await?, "m4a"))
            }
        }
    }

    async fn fetch_soundcloud(&self, track_id: &str) -> Result<(Vec<u8>, &'static str)> {
        let stream = self.providers.soundcloud.stream(track_id).await?;
        let bytes = match stream.kind {
            SoundcloudStreamKind::Progressive => fetch::download(&self.http, &stream.url).await?,
            _ => fetch::download_parts(&self.http, self.providers.soundcloud.hls_parts(&stream.url).await?).await?,
        };
        Ok((bytes, stream.kind.hint()))
    }

    /// videoId da mesma gravação no YouTube (com cache por faixa do Spotify).
    async fn youtube_equivalent(&self, track: &Track) -> Result<String> {
        let key = match_key(track);
        if let Some(id) = self.matches.get(&key) {
            return Ok(id);
        }
        let candidates = self.providers.youtube.search_tracks(&resolver::fallback_query(track), 10).await?;
        let best = resolver::best_match(track, &candidates)
            .ok_or_else(|| Error::Other(format!("não achei \"{}\" no YouTube", track.title)))?;
        self.matches.remember(&key, &best.id);
        Ok(best.id.clone())
    }

    /// Versões no YouTube para uma faixa (da mais parecida para a menos) e qual está em uso.
    pub async fn alternatives(&self, track: &Track) -> Result<(Option<String>, Vec<Track>)> {
        let mut candidates = self.providers.youtube.search_tracks(&resolver::fallback_query(track), 10).await?;
        candidates.sort_by(|a, b| resolver::score(track, b).total_cmp(&resolver::score(track, a)));
        candidates.truncate(MAX_ALTERNATIVES);
        Ok((self.matches.get(&match_key(track)), candidates))
    }

    /// A pessoa escolheu outra versão: vale daqui em diante, e a antiga sai do cache em disco.
    pub fn choose_version(&self, track: &Track, video_id: &str) {
        self.matches.choose(&match_key(track), video_id);
        self.disk.remove(&cache_key(track, PlaybackRoute::YoutubeFallback));
    }

    /// O vídeo do YouTube que está tocando por `track` (para o SponsorBlock), se o áudio vem do YouTube.
    pub fn youtube_video_id(&self, track: &Track, route: PlaybackRoute) -> Option<String> {
        match (route, track.source) {
            (PlaybackRoute::DirectStream, Source::YoutubeMusic) => Some(track.id.clone()),
            (PlaybackRoute::YoutubeFallback, _) => self.matches.get(&match_key(track)),
            _ => None,
        }
    }

    /// O Spotify recusou tocar pelo librespot. Em out/2026 ele passou a negar a chave de
    /// decifragem do áudio (`error audio key 0 1`) a clientes não oficiais: cada tentativa gastava
    /// ~3 s antes de cair no YouTube. Depois da primeira recusa, paramos de tentar e
    /// as faixas do Spotify vão direto pelo YouTube, onde ganham pré-carga e início rápido.
    /// Devolve `true` quando é hora de desistir.
    pub fn note_native_refused(&self) -> bool {
        let give_up = self.native_refusals.fetch_add(1, Ordering::SeqCst) + 1 == NATIVE_REFUSALS_TO_GIVE_UP;
        if give_up {
            // Lembra por 24 h: abrir o app de novo não gasta outra tentativa de ~3 s.
            if let Some(marker) = &self.native_off_marker {
                let _ = std::fs::write(marker, b"");
            }
        }
        give_up
    }

    pub fn native_disabled(&self) -> bool {
        self.native_refusals.load(Ordering::SeqCst) >= NATIVE_REFUSALS_TO_GIVE_UP
    }

    /// Abre a sessão do librespot em segundo plano (custa 1–2 s), para a primeira faixa do
    /// Spotify não pagar isso no clique. Só faz sentido para contas Premium.
    pub async fn warm_spotify(&self) {
        #[cfg(feature = "spotify-premium")]
        {
            let mut native = self.spotify.lock().await;
            if native.is_some() || self.native_disabled() {
                return;
            }
            let session = match self.providers.spotify.playback_token().await {
                Ok((token, client_id)) => spotify_native::SpotifyNative::connect(&token, &client_id, self.events.clone()).await,
                Err(e) => Err(e),
            };
            match session {
                Ok(session) => {
                    log::info!("librespot: sessão aberta antes do primeiro play");
                    *native = Some(session);
                }
                Err(e) => log::info!("librespot: sessão não aberta antes do play: {e}"),
            }
        }
    }

    #[cfg(feature = "spotify-premium")]
    async fn load_spotify(&self, generation: u64, track: &Track, start_ms: u64, volume: f32) -> Result<()> {
        // Bloco próprio: o lock do `spotify` é solto antes de pegar o de `active`.
        // Outros métodos pegam `active` → `spotify`; pegar na ordem inversa aqui
        // poderia travar as duas tarefas esperando uma pela outra (deadlock).
        let result = {
            let mut native = self.spotify.lock().await;
            if native.is_none() {
                let (token, client_id) = self.providers.spotify.playback_token().await?;
                *native = Some(spotify_native::SpotifyNative::connect(&token, &client_id, self.events.clone()).await?);
            }
            if !self.is_current(generation) {
                return Ok(());
            }
            let result = native.as_ref().expect("conectado acima").load(&track.id, generation, start_ms, volume);
            if result.is_err() {
                // Sessão possivelmente morta: na próxima vez, conecta do zero.
                *native = None;
            }
            result
        };
        if result.is_ok() {
            *self.active.lock().await = Active::Spotify;
        }
        result
    }

    async fn play_stream(&self, generation: u64, source: BoxedSource, start_ms: u64, volume: f32) -> Result<()> {
        if !self.is_current(generation) {
            return Ok(());
        }
        self.output.play(source, generation, Duration::from_millis(start_ms), volume);
        *self.active.lock().await = Active::Stream;
        Ok(())
    }

    pub async fn pause(&self) {
        match *self.active.lock().await {
            Active::Stream => self.output.pause(),
            #[cfg(feature = "spotify-premium")]
            Active::Spotify => self.with_spotify(|s| s.pause()).await,
            Active::Nothing => {}
        }
    }

    pub async fn resume(&self) {
        match *self.active.lock().await {
            Active::Stream => self.output.resume(),
            #[cfg(feature = "spotify-premium")]
            Active::Spotify => self.with_spotify(|s| s.resume()).await,
            Active::Nothing => {}
        }
    }

    pub async fn seek(&self, position_ms: u64) {
        let active = *self.active.lock().await;
        match active {
            Active::Stream => match self.streaming.lock().await.take() {
                Some((incoming, hint)) => self.reopen_seekable(incoming, hint, position_ms).await,
                None => self.output.seek(Duration::from_millis(position_ms)),
            },
            #[cfg(feature = "spotify-premium")]
            Active::Spotify => self.with_spotify(|s| s.seek(position_ms)).await,
            Active::Nothing => {}
        }
    }

    pub async fn set_volume(&self, volume: f32) {
        self.output.set_volume(volume);
        #[cfg(feature = "spotify-premium")]
        self.with_spotify(|s| s.set_volume(volume)).await;
    }

    pub async fn stop(&self) {
        let mut active = self.active.lock().await;
        match *active {
            Active::Stream => self.output.stop(),
            #[cfg(feature = "spotify-premium")]
            Active::Spotify => self.with_spotify(|s| s.stop()).await,
            Active::Nothing => {}
        }
        *active = Active::Nothing;
        *self.streaming.lock().await = None;
    }

    #[cfg(feature = "spotify-premium")]
    async fn with_spotify(&self, f: impl FnOnce(&spotify_native::SpotifyNative)) {
        if let Some(native) = self.spotify.lock().await.as_ref() {
            f(native);
        }
    }
}

/// O Spotify recusou o librespot há menos de [`NATIVE_OFF_FOR`]?
fn native_recently_refused(marker: Option<&Path>) -> bool {
    let age = marker
        .and_then(|m| std::fs::metadata(m).ok())
        .and_then(|meta| meta.modified().ok())
        .and_then(|modified| modified.elapsed().ok());
    age.is_some_and(|age| age < NATIVE_OFF_FOR)
}

/// Quantas versões mostramos em "Escolher outra versão".
const MAX_ALTERNATIVES: usize = 6;

/// Identifica a faixa na tabela de equivalências.
fn match_key(track: &Track) -> String {
    format!("{}:{}", track.source.key(), track.id)
}

/// Identifica "este áudio" no cache de pré-carregamento.
/// Marca uma pré-carga como "em andamento" enquanto vive; ao sair (mesmo com erro), desmarca.
struct Preloading<'a> {
    set: &'a std::sync::Mutex<HashSet<String>>,
    key: String,
}

impl<'a> Preloading<'a> {
    /// `None` se a mesma pré-carga já está em andamento.
    fn start(set: &'a std::sync::Mutex<HashSet<String>>, key: &str) -> Option<Self> {
        let fresh = set.lock().map(|mut s| s.insert(key.to_owned())).unwrap_or(false);
        fresh.then(|| Self { set, key: key.to_owned() })
    }
}

impl Drop for Preloading<'_> {
    fn drop(&mut self) {
        if let Ok(mut set) = self.set.lock() {
            set.remove(&self.key);
        }
    }
}

fn cache_key(track: &Track, route: PlaybackRoute) -> String {
    format!("{route:?}:{}:{}", track.source.key(), track.id)
}

/// Decodificar lê o começo do arquivo (bloqueante): roda fora do runtime async.
/// `seekable = false`: o decodificador lê só o começo do arquivo para abrir (ver `fetch::stream`).
async fn spawn_decode<R>(reader: R, hint: &'static str, len: Option<u64>, seekable: bool) -> Result<BoxedSource>
where
    R: Read + Seek + Send + Sync + 'static,
{
    tokio::task::spawn_blocking(move || {
        let mut builder = Decoder::builder().with_data(reader).with_hint(hint).with_seekable(seekable).with_gapless(true);
        if let Some(len) = len {
            builder = builder.with_byte_len(len);
        }
        let decoder = builder.build().map_err(|e| Error::Other(format!("formato de áudio não suportado: {e}")))?;
        Ok(Box::new(decoder) as BoxedSource)
    })
    .await
    .map_err(|e| Error::Other(e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_preload_does_not_run_twice_at_once() {
        let set = std::sync::Mutex::new(HashSet::new());
        let first = Preloading::start(&set, "faixa");
        assert!(first.is_some());
        assert!(Preloading::start(&set, "faixa").is_none(), "a segunda espera a primeira");
        assert!(Preloading::start(&set, "outra").is_some());
        drop(first);
        assert!(Preloading::start(&set, "faixa").is_some(), "terminada, pode pré-carregar de novo");
    }
}

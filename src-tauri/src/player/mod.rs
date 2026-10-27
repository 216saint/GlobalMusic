//! Player: fila (contexto + sua fila, ver `queue`) + roteamento por fonte. Todo o áudio é tocado no
//! processo Rust (nada de mídia na WebView), o que mantém a RAM baixa.
//!
//! O player é a **fonte da verdade** da posição da música: a UI, o Discord e o
//! motor de áudio leem daqui, então nunca discordam entre si.
//!
//! Estados de uma faixa: **carregando** (resolvendo o áudio, relógio parado) →
//! **tocando** (relógio andando) ⇄ **pausada**. O relógio só anda quando o som
//! começou de verdade (`started`); antes disso a UI mostra "Carregando…".

pub mod clock;
pub mod queue;
pub mod resolver;
pub mod routing;

use std::time::Instant;

use serde::Serialize;

use crate::models::Track;
use clock::PlaybackClock;
use queue::{Queue, Slot};
use routing::{route_for, PlaybackRoute};

/// Quantas faixas seguidas podem falhar antes de o player desistir (em vez de varrer a fila inteira).
pub const MAX_CONSECUTIVE_FAILURES: u32 = 3;

#[derive(Debug)]
pub struct Player {
    queue: Queue,
    /// Nome da lista que está tocando (ex.: "best of misery"), para a UI dizer "A seguir de …".
    context_name: Option<String>,
    clock: PlaybackClock,
    /// A pessoa quer ouvir, mas o áudio ainda está sendo resolvido.
    loading: bool,
    volume: f32,
    spotify_premium: bool,
    consecutive_failures: u32,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            queue: Queue::default(),
            context_name: None,
            clock: PlaybackClock::default(),
            loading: false,
            volume: 0.8,
            spotify_premium: false,
            consecutive_failures: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSnapshot {
    pub current: Option<Track>,
    pub route: Option<PlaybackRoute>,
    /// A lista de onde a pessoa deu play (playlist, busca…) e onde ela está nessa lista.
    pub context: Vec<Track>,
    pub context_index: Option<usize>,
    pub context_name: Option<String>,
    /// A sua fila: toca antes do resto do contexto.
    pub up_next: Vec<Track>,
    /// Intenção de tocar: verdadeiro também enquanto carrega (o botão mostra "pausar").
    pub playing: bool,
    /// Resolvendo o áudio: ainda sem som, relógio parado.
    pub loading: bool,
    /// Posição no instante em que o snapshot foi tirado.
    pub position_ms: u64,
    pub volume: f32,
}

impl PlayerSnapshot {
    /// Há som saindo agora? (o que o Discord deve mostrar)
    pub fn audible(&self) -> bool {
        self.playing && !self.loading
    }
}

impl Player {
    pub fn snapshot(&self) -> PlayerSnapshot {
        let current = self.queue.current().cloned();
        let duration = current.as_ref().map_or(0, |t| t.duration_ms);
        PlayerSnapshot {
            route: current.as_ref().map(|t| route_for(t, self.spotify_premium)),
            current,
            context: self.queue.context().to_vec(),
            context_index: self.queue.position(),
            context_name: self.context_name.clone(),
            up_next: self.queue.queued().cloned().collect(),
            playing: self.clock.is_running() || self.loading,
            loading: self.loading,
            position_ms: self.clock.position_ms(Instant::now(), duration),
            volume: self.volume,
        }
    }

    pub fn set_spotify_premium(&mut self, premium: bool) {
        self.spotify_premium = premium;
    }

    pub fn is_loading(&self) -> bool {
        self.loading
    }

    pub fn play_queue(&mut self, tracks: Vec<Track>, start: usize, name: Option<String>) {
        self.queue.replace(tracks, start);
        self.context_name = name;
        self.consecutive_failures = 0;
        self.restart_track(self.queue.current().is_some());
    }

    pub fn toggle(&mut self) {
        let now = Instant::now();
        if self.loading {
            // Pausou antes de o som começar: desiste de tocar quando o áudio chegar.
            self.loading = false;
        } else if self.clock.is_running() {
            self.clock.pause(now);
        } else if self.queue.current().is_some() {
            self.clock.resume(now);
        }
    }

    pub fn next(&mut self) {
        let has_next = self.queue.next().is_some();
        self.restart_track(has_next);
    }

    pub fn previous(&mut self) {
        let playing = self.clock.is_running() || self.loading;
        self.queue.previous();
        self.restart_track(playing);
    }

    /// Adiciona ao fim da sua fila. Devolve `true` se nada tocava e ela começou a tocar.
    pub fn enqueue(&mut self, track: Track) -> bool {
        let started = self.queue.push(track);
        if started {
            self.restart_track(true);
        }
        started
    }

    /// "Tocar a seguir". Devolve `true` se a fila estava parada e começou a tocar.
    pub fn play_next(&mut self, track: Track) -> bool {
        let started = self.queue.insert_next(track);
        if started {
            self.restart_track(true);
        }
        started
    }

    pub fn remove(&mut self, slot: Slot) -> bool {
        self.queue.remove(slot)
    }

    pub fn jump(&mut self, slot: Slot) -> bool {
        let jumped = self.queue.jump(slot);
        if jumped {
            self.consecutive_failures = 0;
            self.restart_track(true);
        }
        jumped
    }

    /// Por onde `track` tocaria agora (depende de a conta do Spotify ser Premium).
    pub fn route(&self, track: &Track) -> PlaybackRoute {
        route_for(track, self.spotify_premium)
    }

    /// As próximas `n` faixas da fila, com a rota de cada uma (para preparar com antecedência).
    pub fn upcoming(&self, n: usize) -> Vec<(Track, PlaybackRoute)> {
        self.queue.peek_ahead(n).into_iter().map(|t| {
            let route = route_for(&t, self.spotify_premium);
            (t, route)
        }).collect()
    }

    pub fn seek(&mut self, position_ms: u64) {
        let duration = self.queue.current().map_or(0, |t| t.duration_ms);
        self.clock.seek(position_ms.min(duration), Instant::now());
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    /// A faixa atual vai carregar de novo, do começo (ex.: o Spotify recusou e ela vai pelo YouTube):
    /// relógio zerado e parado até o som começar.
    pub fn reload(&mut self) {
        self.restart_track(true);
    }

    /// O som começou de verdade: o relógio passa a andar a partir de `position_ms`.
    pub fn started(&mut self, position_ms: u64) {
        let now = Instant::now();
        self.loading = false;
        self.consecutive_failures = 0;
        self.clock.seek(position_ms, now);
        self.clock.resume(now);
    }

    /// Registra uma falha. Devolve `true` se ainda vale tentar a próxima faixa.
    pub fn failed(&mut self) -> bool {
        self.consecutive_failures += 1;
        if self.consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
            self.consecutive_failures = 0;
            self.loading = false;
            self.clock.pause(Instant::now());
            return false;
        }
        true
    }

    fn restart_track(&mut self, play: bool) {
        self.clock = PlaybackClock::default();
        self.loading = play;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Source;

    fn queue(n: usize) -> Vec<Track> {
        (0..n)
            .map(|i| Track {
                id: i.to_string(),
                source: Source::YoutubeMusic,
                title: format!("t{i}"),
                artists: vec![],
                album: None,
                duration_ms: 100_000,
                artwork: None,
                isrc: None,
            })
            .collect()
    }

    #[test]
    fn clock_waits_for_audio_before_running() {
        let mut p = Player::default();
        p.play_queue(queue(2), 0, None);
        let s = p.snapshot();
        assert!(s.playing && s.loading && !s.audible(), "carregando: quer tocar, mas sem som ainda");
        assert_eq!(s.position_ms, 0);

        p.started(0);
        let s = p.snapshot();
        assert!(s.playing && !s.loading && s.audible());
    }

    #[test]
    fn pausing_while_loading_cancels_the_intent() {
        let mut p = Player::default();
        p.play_queue(queue(1), 0, None);
        p.toggle();
        assert!(!p.snapshot().playing && !p.is_loading());
    }

    #[test]
    fn gives_up_after_consecutive_failures() {
        let mut p = Player::default();
        p.play_queue(queue(10), 0, None);
        assert!(p.failed());
        assert!(p.failed());
        assert!(!p.failed(), "a terceira falha seguida para o player");
        assert!(!p.snapshot().playing);
    }

    #[test]
    fn a_success_resets_the_failure_count() {
        let mut p = Player::default();
        p.play_queue(queue(10), 0, None);
        assert!(p.failed());
        assert!(p.failed());
        p.started(0);
        assert!(p.failed(), "depois de tocar algo, a contagem recomeça");
    }
}

//! Relógio de reprodução: sabe em que ponto da música estamos sem precisar
//! de um "tick" a cada segundo.
//!
//! Guardamos só duas coisas: quanto já tinha tocado (`base_ms`) e desde quando
//! está tocando (`running_since`). A posição atual é calculada na hora:
//! `base + (agora - running_since)`. É o mesmo truque que o Discord usa com
//! timestamps de início/fim.

use std::time::Instant;

#[derive(Debug, Clone, Default)]
pub struct PlaybackClock {
    base_ms: u64,
    running_since: Option<Instant>,
}

impl PlaybackClock {
    pub fn is_running(&self) -> bool {
        self.running_since.is_some()
    }

    /// Posição em `now`, nunca além da duração da faixa (se conhecida).
    pub fn position_ms(&self, now: Instant, duration_ms: u64) -> u64 {
        let elapsed = self
            .running_since
            .map_or(0, |since| now.saturating_duration_since(since).as_millis() as u64);
        let position = self.base_ms + elapsed;
        if duration_ms > 0 { position.min(duration_ms) } else { position }
    }

    pub fn resume(&mut self, now: Instant) {
        self.running_since.get_or_insert(now);
    }

    pub fn pause(&mut self, now: Instant) {
        if let Some(since) = self.running_since.take() {
            self.base_ms += now.saturating_duration_since(since).as_millis() as u64;
        }
    }

    /// Pula para `position_ms` mantendo o estado tocando/pausado.
    pub fn seek(&mut self, position_ms: u64, now: Instant) {
        self.base_ms = position_ms;
        if self.running_since.is_some() {
            self.running_since = Some(now);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn advances_only_while_running() {
        let t0 = Instant::now();
        let mut clock = PlaybackClock::default();
        assert_eq!(clock.position_ms(t0 + ms(5_000), 0), 0, "parado não anda");

        clock.resume(t0);
        assert_eq!(clock.position_ms(t0 + ms(3_000), 0), 3_000);

        clock.pause(t0 + ms(3_000));
        assert_eq!(clock.position_ms(t0 + ms(60_000), 0), 3_000, "pausado congela");

        clock.resume(t0 + ms(10_000));
        assert_eq!(clock.position_ms(t0 + ms(12_000), 0), 5_000, "retoma de onde parou");
    }

    #[test]
    fn seek_keeps_play_state() {
        let t0 = Instant::now();
        let mut clock = PlaybackClock::default();
        clock.resume(t0);
        clock.seek(40_000, t0 + ms(1_000));
        assert_eq!(clock.position_ms(t0 + ms(3_000), 0), 42_000);

        clock.pause(t0 + ms(3_000));
        clock.seek(10_000, t0 + ms(4_000));
        assert!(!clock.is_running());
        assert_eq!(clock.position_ms(t0 + ms(9_000), 0), 10_000);
    }

    #[test]
    fn never_passes_the_end_of_the_track() {
        let t0 = Instant::now();
        let mut clock = PlaybackClock::default();
        clock.resume(t0);
        assert_eq!(clock.position_ms(t0 + ms(500_000), 92_000), 92_000);
    }

    #[test]
    fn double_resume_does_not_reset_start() {
        let t0 = Instant::now();
        let mut clock = PlaybackClock::default();
        clock.resume(t0);
        clock.resume(t0 + ms(2_000));
        assert_eq!(clock.position_ms(t0 + ms(4_000), 0), 4_000);
    }
}

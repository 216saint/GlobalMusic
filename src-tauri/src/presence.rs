//! Discord Rich Presence ("Ouvindo GlobalMusic"), via IPC com o cliente local
//! do Discord. Roda numa thread própria porque o IPC é bloqueante, e tenta
//! reconectar sozinho se o Discord for aberto depois do app.
//!
//! Para o card ser *consistente* (ver docs/decisoes/0006-discord-por-ipc.md):
//! - o início da música é um horário absoluto calculado no snapshot, então um
//!   envio atrasado não desalinha a barra de progresso;
//! - rajadas (pular 5 faixas seguidas) viram um único envio, da última;
//! - respeitamos o limite do Discord (5 atualizações / 20s) com 1 envio a cada 4s;
//! - mudanças que não alteram o card (ex.: volume) não geram envio.
//!
//! O card usa o mesmo modelo de Activity do Discord Social SDK; se um dia
//! trocarmos para o SDK, só este módulo muda.

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use discord_rich_presence::activity::{Activity, ActivityType, Assets, Button, StatusDisplayType, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};

use crate::models::Track;
use crate::player::PlayerSnapshot;

/// Sem Discord aberto, tenta conectar de novo a cada 15s.
const RETRY_EVERY: Duration = Duration::from_secs(15);
/// Janela para juntar atualizações em rajada antes de enviar.
const COALESCE: Duration = Duration::from_millis(400);
/// O Discord aceita 5 SET_ACTIVITY a cada 20s; 1 a cada 4s nunca estoura.
const MIN_INTERVAL: Duration = Duration::from_secs(4);
/// Diferença de início abaixo disso é ruído (jitter), não um seek de verdade.
const SAME_START_TOLERANCE_MS: i64 = 1_500;
/// Limites do Discord para `details`/`state`/`large_text`.
const MIN_LEN: usize = 2;
const MAX_LEN: usize = 128;
/// Asset padrão cadastrado no app do Discord (Rich Presence → Art Assets).
const FALLBACK_ART: &str = "globalmusic";

#[derive(Debug, Clone, PartialEq)]
enum Update {
    /// `started_at_ms`: horário Unix (ms) em que a música "começou" — já descontada a posição.
    Listening { track: Track, started_at_ms: i64 },
    Clear,
}

pub struct Presence {
    tx: Option<Sender<Update>>,
}

impl Presence {
    /// Sem `DISCORD_APP_ID` a presença fica desligada e o resto do app segue normal.
    pub fn spawn(app_id: Option<String>) -> Self {
        let Some(app_id) = app_id else { return Self { tx: None } };
        let (tx, rx) = mpsc::channel::<Update>();

        std::thread::Builder::new()
            .name("discord-presence".into())
            .spawn(move || run(app_id, rx))
            .ok()
            .map_or(Self { tx: None }, |_| Self { tx: Some(tx) })
    }

    pub fn show(&self, snapshot: &PlayerSnapshot) {
        let Some(tx) = &self.tx else { return };
        // Só com som saindo de verdade (carregando não conta).
        let update = match (&snapshot.current, snapshot.audible()) {
            (Some(track), true) => {
                Update::Listening { track: track.clone(), started_at_ms: started_at(now_ms(), snapshot.position_ms) }
            }
            _ => Update::Clear,
        };
        let _ = tx.send(update);
    }
}

/// Loop da thread: recebe atualizações, junta rajadas, respeita o limite e envia.
fn run(app_id: String, rx: mpsc::Receiver<Update>) {
    let mut client = DiscordIpcClient::new(&app_id);
    let mut connected = false;
    let mut desired = Update::Clear;
    let mut sent: Option<Update> = None;
    let mut last_sent_at: Option<Instant> = None;

    loop {
        match rx.recv_timeout(RETRY_EVERY) {
            Ok(update) => desired = update,
            // Nada novo: só vale acordar se ainda falta entregar algo (Discord estava fechado).
            Err(RecvTimeoutError::Timeout) if sent.as_ref().is_some_and(|s| same_card(s, &desired)) => continue,
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        while let Ok(update) = rx.recv_timeout(COALESCE) {
            desired = update;
        }
        if sent.as_ref().is_some_and(|s| same_card(s, &desired)) {
            continue;
        }
        if let Some(wait) = last_sent_at.map(|at| MIN_INTERVAL.saturating_sub(at.elapsed())) {
            std::thread::sleep(wait);
            while let Ok(update) = rx.try_recv() {
                desired = update;
            }
        }

        if !connected {
            connected = client.connect().is_ok();
            if !connected {
                continue;
            }
        }
        let result = match &desired {
            Update::Listening { track, started_at_ms } => client.set_activity(activity(track, *started_at_ms)),
            Update::Clear => client.clear_activity(),
        };
        last_sent_at = Some(Instant::now());
        if result.is_ok() {
            sent = Some(desired.clone());
        } else {
            // Discord fechado no meio do caminho: tenta de novo no próximo ciclo.
            let _ = client.close();
            connected = false;
            sent = None;
        }
    }
    let _ = client.close();
}

/// Dois estados mostram o mesmo card? (mesma faixa e mesmo início, a menos de jitter)
fn same_card(a: &Update, b: &Update) -> bool {
    match (a, b) {
        (Update::Clear, Update::Clear) => true,
        (
            Update::Listening { track: ta, started_at_ms: sa },
            Update::Listening { track: tb, started_at_ms: sb },
        ) => ta.source == tb.source && ta.id == tb.id && (sa - sb).abs() < SAME_START_TOLERANCE_MS,
        _ => false,
    }
}

/// Card no formato dos players de música do Discord:
/// ```text
/// Ouvindo GlobalMusic          ← nome do app no Developer Portal
/// [capa]  Título               ← details
///         por Artista          ← state
///         Álbum                ← large_text (sem álbum, repete o título)
///         02:43 ━━━━━━── 03:17 ← timestamps
/// ```
fn activity(track: &Track, started_at_ms: i64) -> Activity<'static> {
    let assets = Assets::new()
        // O Discord aceita URL https como imagem: a capa real aparece no card.
        .large_image(track.artwork.as_ref().map_or_else(|| FALLBACK_ART.into(), |a| a.full.clone()))
        .large_text(clip(track.album.as_deref().unwrap_or(&track.title)))
        .small_image(track.source.key())
        .small_text(format!("via {}", track.source.label()));

    let mut activity = Activity::new()
        .activity_type(ActivityType::Listening)
        // `Name`: o status mostra "Ouvindo GlobalMusic" (e não o nome da música).
        .status_display_type(StatusDisplayType::Name)
        .details(clip(&track.title))
        .state(clip(&format!("por {}", track.artists.join(", "))))
        .assets(assets);
    if let Some((start, end)) = timestamps(started_at_ms, track.duration_ms) {
        activity = activity.timestamps(Timestamps::new().start(start).end(end));
    }
    if let Some(url) = track.web_url() {
        activity = activity
            .details_url(url.clone())
            .buttons(vec![Button::new(format!("Abrir no {}", track.source.label()), url)]);
    }
    activity
}

/// Horário (ms Unix) em que a faixa teria começado, dada a posição atual.
fn started_at(now_ms: i64, position_ms: u64) -> i64 {
    now_ms - position_ms as i64
}

/// Início/fim em ms Unix: o Discord desenha a barra de progresso a partir disso.
fn timestamps(started_at_ms: i64, duration_ms: u64) -> Option<(i64, i64)> {
    (duration_ms > 0).then(|| (started_at_ms, started_at_ms + duration_ms as i64))
}

fn clip(text: &str) -> String {
    let mut text: String = text.trim().chars().take(MAX_LEN).collect();
    while text.chars().count() < MIN_LEN {
        text.push(' ');
    }
    text
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Source;

    fn track(id: &str) -> Track {
        Track {
            id: id.into(),
            source: Source::Spotify,
            title: "Cut To The Feeling".into(),
            artists: vec!["Carly Rae Jepsen".into()],
            album: None,
            duration_ms: 207_000,
            artwork: None,
            isrc: None,
        }
    }

    #[test]
    fn progress_bar_starts_where_the_song_is() {
        assert_eq!(started_at(100_000, 30_000), 70_000);
        assert_eq!(timestamps(70_000, 92_000), Some((70_000, 162_000)));
        assert_eq!(timestamps(70_000, 0), None, "sem duração não há barra");
    }

    #[test]
    fn jitter_and_volume_changes_do_not_resend() {
        let a = Update::Listening { track: track("x"), started_at_ms: 70_000 };
        let jitter = Update::Listening { track: track("x"), started_at_ms: 70_040 };
        let seek = Update::Listening { track: track("x"), started_at_ms: 40_000 };
        let other = Update::Listening { track: track("y"), started_at_ms: 70_000 };

        assert!(same_card(&a, &jitter));
        assert!(!same_card(&a, &seek), "seek muda a barra: precisa reenviar");
        assert!(!same_card(&a, &other));
        assert!(!same_card(&a, &Update::Clear));
        assert!(same_card(&Update::Clear, &Update::Clear));
    }

    #[test]
    fn card_reads_listening_to_globalmusic() {
        // O Activity é serializado no JSON que vai para o Discord: testamos exatamente isso.
        let json = serde_json::to_value(activity(&track("x"), 70_000)).unwrap();

        assert_eq!(json["type"], 2, "2 = Listening (\"Ouvindo\")");
        assert_eq!(json["status_display_type"], 0, "0 = Name: \"Ouvindo GlobalMusic\"");
        assert_eq!(json["details"], "Cut To The Feeling");
        assert_eq!(json["state"], "por Carly Rae Jepsen");
        assert_eq!(json["assets"]["large_text"], "Cut To The Feeling", "sem álbum, repete o título");
        assert_eq!(json["assets"]["large_image"], FALLBACK_ART);
        assert_eq!(json["timestamps"]["end"], 277_000);
    }

    #[test]
    fn clip_respects_discord_limits() {
        assert_eq!(clip("a"), "a ");
        assert_eq!(clip(&"é".repeat(300)).chars().count(), MAX_LEN);
        assert_eq!(clip("  worth it "), "worth it");
    }

    #[test]
    fn disabled_without_app_id() {
        let presence = Presence::spawn(None);
        assert!(presence.tx.is_none());
    }

    /// Teste manual: mostra um card de verdade no seu Discord por 20s.
    /// `cargo test --manifest-path src-tauri/Cargo.toml live_discord_card -- --ignored --nocapture`
    #[test]
    #[ignore = "precisa do Discord desktop aberto e de DISCORD_APP_ID preenchido em config.rs"]
    fn live_discord_card() {
        let app_id = crate::config::DISCORD_APP_ID.expect("preencha DISCORD_APP_ID em src-tauri/src/config.rs");
        let mut client = DiscordIpcClient::new(app_id);
        client.connect().expect("não conectou: o Discord desktop está aberto?");

        // Faixa de exemplo da documentação do Spotify, 30s já tocados.
        let sample = track("11dFghVXANMlKmJXsNCbNl");
        client.set_activity(activity(&sample, started_at(now_ms(), 30_000))).expect("Discord recusou o card");
        println!("Card enviado: confira seu perfil no Discord (some em 20s).");
        std::thread::sleep(Duration::from_secs(20));
        client.clear_activity().expect("não limpou o card");
        let _ = client.close();
    }
}

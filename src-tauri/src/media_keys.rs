//! Teclas de mídia do teclado (play/pause, próxima, anterior, como Fn+F8) e o painel de mídia do
//! sistema: no Windows é o SMTC (o mesmo que aparece no volume do Windows 11 e na tela de bloqueio),
//! no macOS o "Agora tocando" e no Linux o MPRIS. Usa a biblioteca `souvlaki`.
//!
//! Por que não um atalho global? Atalhos globais brigam com outros apps e não aparecem no painel do
//! sistema. Pelo SMTC, o Windows entrega as teclas de mídia ao player que está tocando, como faz com
//! o Spotify.
//!
//! Os controles vivem na thread principal (`run_on_main_thread`): no macOS eles só funcionam lá, e
//! assim o mesmo código serve nos três sistemas.

use std::cell::RefCell;
use std::time::Duration;

use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig};
use tauri::{AppHandle, Manager};

use crate::player::PlayerSnapshot;
use crate::state::AppState;

struct Controls {
    inner: MediaControls,
    /// Faixa mostrada no painel (fonte:id): os metadados só são reenviados quando ela muda.
    shown: Option<String>,
}

thread_local! {
    static CONTROLS: RefCell<Option<Controls>> = const { RefCell::new(None) };
}

/// Liga os controles. Chamar na thread principal, depois de criar a janela.
pub fn setup(app: &AppHandle) {
    #[cfg(target_os = "windows")]
    let hwnd = match app.get_webview_window("main").map(|w| w.hwnd()) {
        Some(Ok(hwnd)) => Some(hwnd.0 as *mut std::ffi::c_void),
        _ => return log::warn!("teclas de mídia: janela principal sem HWND"),
    };
    #[cfg(not(target_os = "windows"))]
    let hwnd = None;

    let config = PlatformConfig { display_name: "GlobalMusic", dbus_name: "globalmusic", hwnd };
    let mut controls = match MediaControls::new(config) {
        Ok(controls) => controls,
        Err(e) => return log::warn!("teclas de mídia indisponíveis: {e:?}"),
    };
    let handle = app.clone();
    let attached = controls.attach(move |event| {
        let app = handle.clone();
        tauri::async_runtime::spawn(async move { on_event(app, event).await });
    });
    if let Err(e) = attached {
        return log::warn!("teclas de mídia indisponíveis: {e:?}");
    }
    CONTROLS.with(|c| *c.borrow_mut() = Some(Controls { inner: controls, shown: None }));
}

/// Atualiza o painel do sistema com o estado do player (chamado a cada mudança).
pub fn show(app: &AppHandle, snapshot: &PlayerSnapshot) {
    let snapshot = snapshot.clone();
    let _ = app.run_on_main_thread(move || {
        CONTROLS.with(|c| {
            if let Some(controls) = c.borrow_mut().as_mut() {
                update(controls, &snapshot);
            }
        });
    });
}

fn update(controls: &mut Controls, snapshot: &PlayerSnapshot) {
    let Some(track) = &snapshot.current else {
        controls.shown = None;
        let _ = controls.inner.set_playback(MediaPlayback::Stopped);
        return;
    };
    let key = format!("{}:{}", track.source.key(), track.id);
    if controls.shown.as_deref() != Some(&key) {
        let artists = track.artists.join(", ");
        let metadata = MediaMetadata {
            title: Some(&track.title),
            artist: Some(&artists),
            album: track.album.as_deref(),
            cover_url: track.artwork.as_ref().map(|a| a.full.as_str()),
            duration: (track.duration_ms > 0).then(|| Duration::from_millis(track.duration_ms)),
        };
        if controls.inner.set_metadata(metadata).is_ok() {
            controls.shown = Some(key);
        }
    }
    let progress = Some(MediaPosition(Duration::from_millis(snapshot.position_ms)));
    let playback = if snapshot.playing { MediaPlayback::Playing { progress } } else { MediaPlayback::Paused { progress } };
    let _ = controls.inner.set_playback(playback);
}

/// Uma tecla de mídia (ou um botão do painel do sistema) foi apertada: faz o mesmo que o botão do app.
async fn on_event(app: AppHandle, event: MediaControlEvent) {
    use crate::commands::{player_next, player_previous, player_seek, player_toggle};
    let state = app.state::<AppState>();
    let playing = state.player.lock().await.snapshot().playing;
    let result = match event {
        MediaControlEvent::Toggle => player_toggle(app.clone(), state).await,
        MediaControlEvent::Play if !playing => player_toggle(app.clone(), state).await,
        MediaControlEvent::Pause | MediaControlEvent::Stop if playing => player_toggle(app.clone(), state).await,
        MediaControlEvent::Next => player_next(app.clone(), state).await,
        MediaControlEvent::Previous => player_previous(app.clone(), state).await,
        MediaControlEvent::SetPosition(MediaPosition(at)) => player_seek(app.clone(), state, at.as_millis() as u64).await,
        _ => return,
    };
    if let Err(e) = result {
        log::warn!("tecla de mídia falhou: {e}");
    }
}

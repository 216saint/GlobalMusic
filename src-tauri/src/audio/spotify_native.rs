//! Reprodução nativa do Spotify (contas Premium) com o `librespot`.
//!
//! O librespot se conecta aos servidores do Spotify como um cliente oficial
//! faria, usando o mesmo access token do nosso login OAuth (escopo `streaming`),
//! baixa o áudio criptografado, decodifica (Ogg Vorbis) e toca com o próprio rodio.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use librespot::core::authentication::Credentials;
use librespot::core::config::SessionConfig;
use librespot::core::session::Session;
use librespot::core::SpotifyUri;
use librespot::playback::audio_backend;
use librespot::playback::config::{AudioFormat, PlayerConfig};
use librespot::playback::mixer::softmixer::SoftMixer;
use librespot::playback::mixer::{Mixer, MixerConfig};
use librespot::playback::player::{Player, PlayerEvent};

use super::{AudioEvent, EventSink};
use crate::error::{Error, Result};

pub struct SpotifyNative {
    // A sessão precisa ficar viva enquanto o player existir.
    _session: Session,
    player: Arc<Player>,
    mixer: Arc<SoftMixer>,
    /// Geração da faixa carregada agora (para ignorar eventos de faixas antigas).
    generation: Arc<AtomicU64>,
}

impl SpotifyNative {
    pub async fn connect(access_token: &str, client_id: &str, events: EventSink) -> Result<Self> {
        let config = SessionConfig { client_id: client_id.to_owned(), ..SessionConfig::default() };
        let session = Session::new(config, None);
        session
            .connect(Credentials::with_access_token(access_token), false)
            .await
            .map_err(|e| Error::Other(format!("librespot não conectou: {e}")))?;

        let mixer = Arc::new(SoftMixer::open(MixerConfig::default()).map_err(|e| Error::Other(e.to_string()))?);
        let backend = audio_backend::find(None).ok_or_else(|| Error::Other("nenhum backend de áudio no librespot".into()))?;
        let player = Player::new(PlayerConfig::default(), session.clone(), mixer.get_soft_volume(), move || {
            backend(None, AudioFormat::default())
        });

        let generation = Arc::new(AtomicU64::new(0));
        let mut channel = player.get_player_event_channel();
        let current = generation.clone();
        tokio::spawn(async move {
            while let Some(event) = channel.recv().await {
                let generation = current.load(Ordering::SeqCst);
                match event {
                    PlayerEvent::EndOfTrack { .. } => (events)(AudioEvent::Ended(generation)),
                    PlayerEvent::Unavailable { track_id, .. } => {
                        log::warn!("[{generation}] librespot: faixa indisponível ({track_id:?})");
                        (events)(AudioEvent::NativeUnavailable(generation))
                    }
                    _ => {}
                }
            }
        });

        Ok(Self { _session: session, player, mixer, generation })
    }

    pub fn load(&self, track_id: &str, generation: u64, start_ms: u64, volume: f32) -> Result<()> {
        let uri = SpotifyUri::from_uri(&format!("spotify:track:{track_id}")).map_err(|e| Error::Other(e.to_string()))?;
        self.generation.store(generation, Ordering::SeqCst);
        self.set_volume(volume);
        self.player.load(uri, true, start_ms.min(u32::MAX as u64) as u32);
        Ok(())
    }

    /// Começa a baixar a próxima faixa enquanto a atual toca (o librespot guarda e usa no `load`).
    pub fn preload(&self, track_id: &str) {
        if let Ok(uri) = SpotifyUri::from_uri(&format!("spotify:track:{track_id}")) {
            self.player.preload(uri);
        }
    }

    pub fn pause(&self) {
        self.player.pause();
    }
    pub fn resume(&self) {
        self.player.play();
    }
    pub fn seek(&self, position_ms: u64) {
        self.player.seek(position_ms.min(u32::MAX as u64) as u32);
    }
    pub fn stop(&self) {
        self.player.stop();
    }
    pub fn set_volume(&self, volume: f32) {
        self.mixer.set_volume((volume.clamp(0.0, 1.0) * u16::MAX as f32) as u16);
    }
}

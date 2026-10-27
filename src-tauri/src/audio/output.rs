//! Saída de áudio com `rodio`, numa thread dedicada.
//!
//! Por que uma thread só para isso? O `OutputStream` do rodio (que segura o
//! dispositivo de som) não pode mudar de thread em todas as plataformas. Então
//! ele nasce e morre aqui, e o resto do app conversa por mensagens (`Cmd`).
//!
//! A thread também percebe quando a música acaba (a fila do `Sink` esvazia) e
//! avisa o player para tocar a próxima.

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::Duration;

use rodio::{OutputStream, OutputStreamBuilder, Sink, Source};

use super::{AudioEvent, EventSink};

/// De quanto em quanto tempo verificamos se a música acabou.
const POLL: Duration = Duration::from_millis(250);

pub type BoxedSource = Box<dyn Source + Send>;

enum Cmd {
    Play { source: BoxedSource, generation: u64, start: Duration, volume: f32 },
    /// Troca a fonte da faixa atual pela mesma música aberta de outro jeito (com seek),
    /// mantendo volume, pausa e geração.
    Reopen { source: BoxedSource, start: Duration },
    Pause,
    Resume,
    Seek(Duration),
    Volume(f32),
    Stop,
}

pub struct Output {
    tx: Sender<Cmd>,
}

impl Output {
    pub fn spawn(events: EventSink) -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("audio-output".into())
            .spawn(move || run(rx, events))
            .expect("não foi possível criar a thread de áudio");
        Self { tx }
    }

    pub fn play(&self, source: BoxedSource, generation: u64, start: Duration, volume: f32) {
        let _ = self.tx.send(Cmd::Play { source, generation, start, volume });
    }
    pub fn reopen(&self, source: BoxedSource, start: Duration) {
        let _ = self.tx.send(Cmd::Reopen { source, start });
    }
    pub fn pause(&self) {
        let _ = self.tx.send(Cmd::Pause);
    }
    pub fn resume(&self) {
        let _ = self.tx.send(Cmd::Resume);
    }
    pub fn seek(&self, position: Duration) {
        let _ = self.tx.send(Cmd::Seek(position));
    }
    pub fn set_volume(&self, volume: f32) {
        let _ = self.tx.send(Cmd::Volume(volume));
    }
    pub fn stop(&self) {
        let _ = self.tx.send(Cmd::Stop);
    }
}

struct Playing {
    sink: Sink,
    generation: u64,
}

fn run(rx: mpsc::Receiver<Cmd>, events: EventSink) {
    // O dispositivo só é aberto no primeiro "play": app parado não segura a placa de som.
    let mut stream: Option<OutputStream> = None;
    let mut current: Option<Playing> = None;

    loop {
        match rx.recv_timeout(POLL) {
            Ok(Cmd::Play { source, generation, start, volume }) => {
                if let Some(old) = current.take() {
                    old.sink.stop();
                }
                if stream.is_none() {
                    match OutputStreamBuilder::open_default_stream() {
                        Ok(s) => stream = Some(s),
                        Err(e) => {
                            (events)(AudioEvent::Failed(generation, format!("sem saída de áudio: {e}")));
                            continue;
                        }
                    }
                }
                let sink = Sink::connect_new(stream.as_ref().expect("aberto acima").mixer());
                sink.set_volume(volume);
                sink.append(source);
                if !start.is_zero() {
                    let _ = sink.try_seek(start);
                }
                current = Some(Playing { sink, generation });
            }
            Ok(Cmd::Reopen { source, start }) => {
                if let (Some(old), Some(stream)) = (current.take(), stream.as_ref()) {
                    let sink = Sink::connect_new(stream.mixer());
                    sink.set_volume(old.sink.volume());
                    if old.sink.is_paused() {
                        sink.pause();
                    }
                    sink.append(source);
                    let _ = sink.try_seek(start);
                    old.sink.stop();
                    current = Some(Playing { sink, generation: old.generation });
                }
            }
            Ok(Cmd::Pause) => current.iter().for_each(|p| p.sink.pause()),
            Ok(Cmd::Resume) => current.iter().for_each(|p| p.sink.play()),
            Ok(Cmd::Seek(position)) => {
                if let Some(p) = &current {
                    let _ = p.sink.try_seek(position);
                }
            }
            Ok(Cmd::Volume(v)) => current.iter().for_each(|p| p.sink.set_volume(v)),
            Ok(Cmd::Stop) => {
                if let Some(p) = current.take() {
                    p.sink.stop();
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        // Acabou a música? (fila vazia e não está pausado por nós)
        if current.as_ref().is_some_and(|p| p.sink.empty() && !p.sink.is_paused()) {
            let finished = current.take().expect("verificado acima");
            (events)(AudioEvent::Ended(finished.generation));
        }
    }
}

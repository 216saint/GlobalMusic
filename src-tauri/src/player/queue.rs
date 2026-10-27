//! Fila no modelo do Spotify, misturando faixas de qualquer fonte. São duas listas:
//!
//! - **contexto**: a lista de onde a pessoa deu play (uma playlist, uma busca, as curtidas).
//!   Ela anda sozinha, uma faixa depois da outra.
//! - **sua fila** (`queued`): o que a pessoa pôs à mão ("Adicionar à fila", "Tocar a seguir").
//!   Toca **antes** do resto do contexto, na ordem; depois o contexto continua de onde parou.
//!
//! Antes era uma lista só, e "Adicionar à fila" colocava a música no fim da playlist.

use std::collections::VecDeque;

use serde::Deserialize;

use crate::models::Track;

/// Endereço de uma faixa que ainda vai tocar (tirar da fila, pular até ela).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(tag = "list", content = "index", rename_all = "lowercase")]
pub enum Slot {
    Queued(usize),
    Context(usize),
}

#[derive(Debug, Clone, Default)]
pub struct Queue {
    context: Vec<Track>,
    /// Onde o contexto está: a faixa atual ou, se a atual veio da sua fila, a última do contexto.
    position: Option<usize>,
    queued: VecDeque<Track>,
    /// A faixa atual (do contexto ou da sua fila).
    now: Option<Track>,
    /// A atual veio da sua fila (e não do contexto).
    now_queued: bool,
}

impl Queue {
    /// Toca `tracks` a partir de `start` (novo contexto). A sua fila continua guardada.
    pub fn replace(&mut self, tracks: Vec<Track>, start: usize) {
        self.position = (start < tracks.len()).then_some(start);
        self.now = self.position.map(|i| tracks[i].clone());
        self.now_queued = false;
        self.context = tracks;
    }

    pub fn current(&self) -> Option<&Track> {
        self.now.as_ref()
    }

    pub fn context(&self) -> &[Track] {
        &self.context
    }

    pub fn position(&self) -> Option<usize> {
        self.position
    }

    pub fn queued(&self) -> impl Iterator<Item = &Track> {
        self.queued.iter()
    }

    /// Avança: primeiro a sua fila, depois o contexto. No fim de tudo devolve `None`.
    pub fn next(&mut self) -> Option<&Track> {
        if let Some(track) = self.queued.pop_front() {
            self.now = Some(track);
            self.now_queued = true;
            return self.now.as_ref();
        }
        let next = self.position.map_or(0, |p| p + 1);
        let track = self.context.get(next)?.clone();
        self.position = Some(next);
        self.now = Some(track);
        self.now_queued = false;
        self.now.as_ref()
    }

    /// Volta uma faixa do contexto. Saindo de uma faixa da sua fila, volta para onde o contexto estava.
    pub fn previous(&mut self) -> Option<&Track> {
        let position = self.position?;
        let target = if self.now_queued { position } else { position.saturating_sub(1) };
        self.position = Some(target);
        self.now = self.context.get(target).cloned();
        self.now_queued = false;
        self.now.as_ref()
    }

    /// As próximas `n` faixas (sua fila, depois o contexto), sem andar.
    pub fn peek_ahead(&self, n: usize) -> Vec<Track> {
        let rest = self.position.map_or(0, |p| p + 1);
        self.queued.iter().chain(self.context.iter().skip(rest)).take(n).cloned().collect()
    }

    /// "Adicionar à fila": entra no fim da **sua fila** (antes do resto do contexto).
    /// Com nada tocando, a faixa já começa; devolve `true` nesse caso.
    pub fn push(&mut self, track: Track) -> bool {
        self.queued.push_back(track);
        self.start_if_idle()
    }

    /// "Tocar a seguir": entra no começo da sua fila, logo depois da atual.
    pub fn insert_next(&mut self, track: Track) -> bool {
        self.queued.push_front(track);
        self.start_if_idle()
    }

    /// Tira uma faixa que ainda ia tocar. A atual não sai (pule ou troque antes).
    pub fn remove(&mut self, slot: Slot) -> bool {
        match slot {
            Slot::Queued(i) => self.queued.remove(i).is_some(),
            Slot::Context(i) => {
                let is_current = !self.now_queued && Some(i) == self.position;
                if i >= self.context.len() || is_current {
                    return false;
                }
                self.context.remove(i);
                if let Some(p) = self.position.filter(|&p| p > i) {
                    self.position = Some(p - 1);
                }
                true
            }
        }
    }

    /// Pula até uma faixa. Na sua fila, as que estavam antes dela são puladas junto (como no Spotify).
    pub fn jump(&mut self, slot: Slot) -> bool {
        match slot {
            Slot::Queued(i) if i < self.queued.len() => {
                self.queued.drain(..i);
                self.now = self.queued.pop_front();
                self.now_queued = true;
                true
            }
            Slot::Context(i) if i < self.context.len() => {
                self.position = Some(i);
                self.now = Some(self.context[i].clone());
                self.now_queued = false;
                true
            }
            _ => false,
        }
    }

    fn start_if_idle(&mut self) -> bool {
        if self.now.is_some() {
            return false;
        }
        self.now = self.queued.pop_front();
        self.now_queued = true;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Source;

    fn track(id: &str, source: Source) -> Track {
        Track {
            id: id.into(),
            source,
            title: id.into(),
            artists: vec![],
            album: None,
            duration_ms: 0,
            artwork: None,
            isrc: None,
        }
    }

    fn playlist(ids: &[&str]) -> Vec<Track> {
        ids.iter().map(|id| track(id, Source::Spotify)).collect()
    }

    fn ids(tracks: &[Track]) -> Vec<&str> {
        tracks.iter().map(|t| t.id.as_str()).collect()
    }

    #[test]
    fn navigates_the_context() {
        let mut q = Queue::default();
        q.replace(playlist(&["a", "b", "c"]), 1);
        assert_eq!(q.current().unwrap().id, "b");
        assert_eq!(q.next().unwrap().id, "c");
        assert!(q.next().is_none(), "fim da lista");
        assert_eq!(q.current().unwrap().id, "c", "no fim, a atual continua");
        assert_eq!(q.previous().unwrap().id, "b");
        assert_eq!(q.previous().unwrap().id, "a");
        assert_eq!(q.previous().unwrap().id, "a");
    }

    #[test]
    fn out_of_range_start_leaves_nothing_playing() {
        let mut q = Queue::default();
        q.replace(playlist(&["a"]), 5);
        assert!(q.current().is_none());
    }

    #[test]
    fn added_tracks_play_before_the_rest_of_the_playlist_not_at_its_end() {
        let mut q = Queue::default();
        q.replace(playlist(&["a", "b", "c"]), 0);
        assert!(!q.push(track("yt", Source::YoutubeMusic)), "já tocava: não troca a atual");
        assert!(!q.push(track("sc", Source::Soundcloud)));
        assert_eq!(ids(q.context()), ["a", "b", "c"], "a playlist não muda");
        assert_eq!(ids(&q.peek_ahead(10)), ["yt", "sc", "b", "c"]);

        assert_eq!(q.next().unwrap().id, "yt");
        assert_eq!(q.next().unwrap().id, "sc");
        assert_eq!(q.next().unwrap().id, "b", "depois da sua fila, a playlist continua de onde parou");
    }

    #[test]
    fn play_next_cuts_in_front_of_the_queue() {
        let mut q = Queue::default();
        q.replace(playlist(&["a", "b"]), 0);
        q.push(track("depois", Source::Soundcloud));
        q.insert_next(track("agora", Source::YoutubeMusic));
        assert_eq!(ids(&q.peek_ahead(10)), ["agora", "depois", "b"]);
    }

    #[test]
    fn previous_from_a_queued_track_returns_to_the_playlist() {
        let mut q = Queue::default();
        q.replace(playlist(&["a", "b"]), 0);
        q.push(track("yt", Source::YoutubeMusic));
        q.next();
        assert_eq!(q.previous().unwrap().id, "a");
    }

    #[test]
    fn starting_another_playlist_keeps_your_queue() {
        let mut q = Queue::default();
        q.replace(playlist(&["a", "b"]), 0);
        q.push(track("yt", Source::YoutubeMusic));
        q.replace(playlist(&["x", "y"]), 0);
        assert_eq!(ids(&q.peek_ahead(10)), ["yt", "y"]);
    }

    #[test]
    fn adding_to_an_idle_player_starts_it() {
        let mut q = Queue::default();
        assert!(q.push(track("a", Source::Soundcloud)));
        assert_eq!(q.current().unwrap().id, "a");
        assert!(q.peek_ahead(3).is_empty());
    }

    #[test]
    fn removes_and_jumps_in_either_list() {
        let mut q = Queue::default();
        q.replace(playlist(&["a", "b", "c", "d"]), 1);
        q.push(track("q1", Source::YoutubeMusic));
        q.push(track("q2", Source::YoutubeMusic));
        q.push(track("q3", Source::YoutubeMusic));

        assert!(!q.remove(Slot::Context(1)), "a atual não sai");
        assert!(q.remove(Slot::Context(0)));
        assert_eq!(q.current().unwrap().id, "b", "tirar antes da atual não troca a música");
        assert!(q.remove(Slot::Queued(1)));
        assert_eq!(ids(&q.peek_ahead(10)), ["q1", "q3", "c", "d"]);

        assert!(q.jump(Slot::Queued(1)), "pular para q3 descarta q1");
        assert_eq!(q.current().unwrap().id, "q3");
        assert_eq!(ids(&q.peek_ahead(10)), ["c", "d"]);
        assert!(q.jump(Slot::Context(2)));
        assert_eq!(q.current().unwrap().id, "d");
        assert!(!q.jump(Slot::Context(9)));
    }

    #[test]
    fn slot_reads_from_the_frontend_json() {
        let slot: Slot = serde_json::from_str(r#"{"list":"queued","index":2}"#).unwrap();
        assert_eq!(slot, Slot::Queued(2));
    }
}

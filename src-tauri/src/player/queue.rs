//! Fila no modelo do Spotify, misturando faixas de qualquer fonte. São duas listas:
//!
//! - **contexto**: a lista de onde a pessoa deu play (uma playlist, uma busca, as curtidas).
//!   Ela anda sozinha, uma faixa depois da outra.
//! - **sua fila** (`queued`): o que a pessoa pôs à mão ("Adicionar à fila", "Tocar a seguir").
//!   Toca **antes** do resto do contexto, na ordem; depois o contexto continua de onde parou.
//!
//! Antes era uma lista só, e "Adicionar à fila" colocava a música no fim da playlist.
//!
//! **Aleatório** reordena o próprio contexto (a atual no topo, o resto embaralhado) e guarda a
//! ordem original para quando desligar. Assim a fila na tela já mostra a ordem em que vai tocar.
//! **Repetir** vale para o contexto: "todas" volta ao começo no fim da lista; "uma" repete a atual
//! quando ela acaba sozinha (ver `Player::finished`). Pular com "próxima" ainda avança.

use std::collections::VecDeque;

use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use crate::models::Track;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Repeat {
    #[default]
    Off,
    /// A lista inteira, em volta.
    All,
    /// A música atual, até a pessoa pular.
    One,
}

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
    pub repeat: Repeat,
    /// Com o aleatório ligado: o contexto na ordem original (para voltar a ela ao desligar).
    original: Option<Vec<Track>>,
}

impl Queue {
    /// Toca `tracks` a partir de `start` (novo contexto). A sua fila continua guardada.
    pub fn replace(&mut self, tracks: Vec<Track>, start: usize) {
        self.position = (start < tracks.len()).then_some(start);
        self.now = self.position.map(|i| tracks[i].clone());
        self.now_queued = false;
        self.context = tracks;
        if self.original.is_some() {
            self.original = None;
            self.set_shuffle(true);
        }
    }

    /// Troca o resto da lista por `tracks`: a faixa atual vira a primeira do novo contexto e
    /// continua tocando, e a sua fila fica como está. Sem faixa atual, é um `replace` do começo.
    pub fn continue_with(&mut self, tracks: Vec<Track>) {
        let Some(now) = self.now.clone() else { return self.replace(tracks, 0) };
        self.replace(std::iter::once(now).chain(tracks).collect(), 0);
    }

    pub fn shuffled(&self) -> bool {
        self.original.is_some()
    }

    /// Liga: a faixa do contexto em que estamos vai para o topo e o resto é embaralhado.
    /// Desliga: volta à ordem original, continuando da mesma faixa.
    pub fn set_shuffle(&mut self, on: bool) {
        match (on, self.original.take()) {
            (true, None) => {
                self.original = Some(self.context.clone());
                let anchor = self.position.map(|p| self.context.remove(p));
                self.context.shuffle(&mut rand::rng());
                if let Some(track) = anchor {
                    self.context.insert(0, track);
                    self.position = Some(0);
                }
            }
            (false, Some(original)) => {
                let anchor = self.position.and_then(|p| self.context.get(p));
                self.position = anchor.and_then(|a| original.iter().position(|t| same(t, a))).or(self.position.map(|_| 0));
                self.context = original;
            }
            (_, unchanged) => self.original = unchanged,
        }
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

    /// Avança: primeiro a sua fila, depois o contexto. No fim de tudo devolve `None`
    /// (com repetição ligada, o contexto recomeça).
    pub fn next(&mut self) -> Option<&Track> {
        if let Some(track) = self.queued.pop_front() {
            self.now = Some(track);
            self.now_queued = true;
            return self.now.as_ref();
        }
        let mut next = self.position.map_or(0, |p| p + 1);
        if next >= self.context.len() && self.repeat != Repeat::Off {
            next = 0;
        }
        let track = self.context.get(next)?.clone();
        self.position = Some(next);
        self.now = Some(track);
        self.now_queued = false;
        self.now.as_ref()
    }

    /// Volta uma faixa do contexto. Saindo de uma faixa da sua fila, volta para onde o contexto estava.
    /// Na primeira, com repetição ligada, vai para a última.
    pub fn previous(&mut self) -> Option<&Track> {
        let position = self.position?;
        let target = match (self.now_queued, position) {
            (true, p) => p,
            (false, 0) if self.repeat != Repeat::Off => self.context.len().saturating_sub(1),
            (false, p) => p.saturating_sub(1),
        };
        self.position = Some(target);
        self.now = self.context.get(target).cloned();
        self.now_queued = false;
        self.now.as_ref()
    }

    /// `previous` levaria a outra faixa? (Na primeira, sem repetição, ele só fica onde está.)
    pub fn can_go_back(&self) -> bool {
        self.position.is_some_and(|p| self.now_queued || p > 0 || self.repeat != Repeat::Off)
    }

    /// As próximas `n` faixas (sua fila, depois o contexto), sem andar. Com repetição, o começo da
    /// lista vem depois do fim (para pré-carregar a volta).
    pub fn peek_ahead(&self, n: usize) -> Vec<Track> {
        let rest = self.position.map_or(0, |p| p + 1);
        let wrap = if self.repeat == Repeat::Off { 0 } else { rest.min(self.context.len()) };
        self.queued.iter().chain(self.context.iter().skip(rest)).chain(self.context.iter().take(wrap)).take(n).cloned().collect()
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
                let removed = self.context.remove(i);
                if let Some(original) = &mut self.original {
                    if let Some(at) = original.iter().position(|t| same(t, &removed)) {
                        original.remove(at);
                    }
                }
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

fn same(a: &Track, b: &Track) -> bool {
    a.source == b.source && a.id == b.id
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
    fn continue_with_keeps_the_current_song_and_your_queue() {
        let mut q = Queue::default();
        q.replace(playlist(&["a", "b", "c"]), 1);
        q.push(track("mine", Source::Soundcloud));
        q.continue_with(playlist(&["x", "y"]));
        assert_eq!(q.current().unwrap().id, "b", "a música que toca não muda");
        assert_eq!(ids(q.context()), ["b", "x", "y"]);
        assert_eq!(q.next().unwrap().id, "mine", "a sua fila continua na frente");
        assert_eq!(q.next().unwrap().id, "x");

        let mut empty = Queue::default();
        empty.continue_with(playlist(&["x"]));
        assert_eq!(empty.current().unwrap().id, "x", "sem nada tocando: começa pelas novas");
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
    fn repeat_all_wraps_around_both_ways() {
        let mut q = Queue::default();
        q.repeat = Repeat::All;
        q.replace(playlist(&["a", "b"]), 1);
        assert_eq!(ids(&q.peek_ahead(5)), ["a", "b"], "pré-carrega a volta");
        assert_eq!(q.next().unwrap().id, "a", "no fim, volta ao começo");
        assert_eq!(q.previous().unwrap().id, "b", "no começo, vai para o fim");
    }

    #[test]
    fn shuffle_keeps_the_current_first_and_restores_the_order() {
        let mut q = Queue::default();
        let order: Vec<String> = (0..30).map(|i| i.to_string()).collect();
        let names: Vec<&str> = order.iter().map(String::as_str).collect();
        q.replace(playlist(&names), 7);

        q.set_shuffle(true);
        assert!(q.shuffled());
        assert_eq!(q.position(), Some(0));
        assert_eq!(q.current().unwrap().id, "7", "a atual não muda");
        assert_eq!(q.context()[0].id, "7");
        let mut sorted = ids(q.context()).iter().map(|id| id.parse::<u32>().unwrap()).collect::<Vec<_>>();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..30).collect::<Vec<_>>(), "mesmas faixas, sem perder nem repetir");

        q.next();
        let playing = q.current().unwrap().id.clone();
        q.set_shuffle(false);
        assert_eq!(ids(q.context()), names, "volta à ordem original");
        assert_eq!(q.context()[q.position().unwrap()].id, playing, "continua da mesma faixa");
    }

    #[test]
    fn a_new_playlist_starts_shuffled_from_the_chosen_track() {
        let mut q = Queue::default();
        q.set_shuffle(true);
        q.replace(playlist(&["a", "b", "c", "d"]), 2);
        assert!(q.shuffled());
        assert_eq!(q.context()[0].id, "c");
        assert_eq!(q.position(), Some(0));
    }

    #[test]
    fn removing_while_shuffled_also_leaves_the_original_order() {
        let mut q = Queue::default();
        q.replace(playlist(&["a", "b", "c"]), 0);
        q.set_shuffle(true);
        let gone = q.context()[1].id.clone();
        assert!(q.remove(Slot::Context(1)));
        q.set_shuffle(false);
        assert_eq!(q.context().len(), 2);
        assert!(q.context().iter().all(|t| t.id != gone));
    }

    #[test]
    fn slot_reads_from_the_frontend_json() {
        let slot: Slot = serde_json::from_str(r#"{"list":"queued","index":2}"#).unwrap();
        assert_eq!(slot, Slot::Queued(2));
    }
}

//! Acha, entre resultados do YouTube Music, a mesma gravação de uma faixa do
//! Spotify. Usado pelo fallback de contas Free.

use std::collections::HashSet;

use crate::models::Track;

/// Abaixo disso preferimos não tocar nada a tocar a música errada.
pub const MIN_SCORE: f32 = 0.6;

/// Palavras que indicam outra versão da música.
const VERSION_MARKERS: &[&str] = &[
    "live", "ao vivo", "cover", "remix", "karaoke", "instrumental", "sped up", "slowed", "nightcore", "8d",
];

/// Ruído comum em títulos de vídeo que não muda a gravação.
const TITLE_NOISE: &[&str] = &["official", "oficial", "video", "audio", "lyrics", "letra", "hd", "4k", "mv", "visualizer"];

pub fn fallback_query(track: &Track) -> String {
    match track.artists.first() {
        Some(artist) => format!("{artist} {}", track.title),
        None => track.title.clone(),
    }
}

/// Se a busca principal não achar nada parecido o bastante: título com todos os artistas e, por
/// último, o ISRC (código da gravação, que o YouTube Music às vezes encontra direto).
pub fn retry_queries(track: &Track) -> Vec<String> {
    let mut queries = vec![format!("{} {}", track.title, track.artists.join(" ")).trim().to_owned()];
    queries.extend(track.isrc.clone());
    queries
}

pub fn best_match<'a>(target: &Track, candidates: &'a [Track]) -> Option<&'a Track> {
    candidates
        .iter()
        .map(|c| (c, score(target, c)))
        .filter(|(_, s)| *s >= MIN_SCORE)
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(c, _)| c)
}

/// Uma cópia de cada música, sem `playing`. A mesma gravação costuma existir em vários vídeos (álbum,
/// single, clipe), com IDs diferentes; versões de verdade ("Live", "Remix") têm outro título e ficam.
pub fn distinct_songs(playing: &Track, tracks: Vec<Track>) -> Vec<Track> {
    let mut seen = HashSet::from([song_key(playing)]);
    tracks.into_iter().filter(|t| seen.insert(song_key(t))).collect()
}

fn song_key(track: &Track) -> String {
    let artist = track.artists.first().map_or(Vec::new(), |a| words(a));
    format!("{} / {}", words(&track.title).join(" "), artist.join(" "))
}

pub fn score(target: &Track, candidate: &Track) -> f32 {
    let title = title_similarity(&target.title, &candidate.title);
    let artist = artist_match(target, candidate);
    let duration = duration_match(target.duration_ms, candidate.duration_ms);
    let penalty = if introduces_version_marker(&target.title, &candidate.title) { 0.35 } else { 0.0 };
    0.45 * title + 0.30 * artist + 0.25 * duration - penalty
}

fn normalize(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect()
}

fn words(text: &str) -> Vec<String> {
    normalize(text)
        .split_whitespace()
        .filter(|w| !TITLE_NOISE.contains(w))
        .map(str::to_owned)
        .collect()
}

/// Fração das palavras do título original presentes no candidato.
fn title_similarity(target: &str, candidate: &str) -> f32 {
    let target = words(target);
    if target.is_empty() {
        return 0.0;
    }
    let candidate = words(candidate);
    let hits = target.iter().filter(|w| candidate.contains(w)).count();
    hits as f32 / target.len() as f32
}

/// No YouTube o artista às vezes só aparece no título ("Artista - Música").
fn artist_match(target: &Track, candidate: &Track) -> f32 {
    let haystack = normalize(&format!("{} {}", candidate.artists.join(" "), candidate.title));
    // "milkshakes, yen archive" num texto só (algumas páginas do Spotify vêm assim): um de cada vez.
    let found = target.artists.iter().flat_map(|a| a.split(',')).any(|a| {
        let a = normalize(a);
        let a = a.trim();
        !a.is_empty() && haystack.contains(a)
    });
    if found { 1.0 } else { 0.0 }
}

fn duration_match(target_ms: u64, candidate_ms: u64) -> f32 {
    if target_ms == 0 || candidate_ms == 0 {
        return 0.5;
    }
    match target_ms.abs_diff(candidate_ms) {
        0..=3_000 => 1.0,
        3_001..=10_000 => 0.6,
        10_001..=20_000 => 0.2,
        _ => 0.0,
    }
}

fn introduces_version_marker(target: &str, candidate: &str) -> bool {
    let target = format!(" {} ", normalize(target));
    let candidate = format!(" {} ", normalize(candidate));
    VERSION_MARKERS.iter().any(|m| {
        let m = format!(" {m} ");
        candidate.contains(&m) && !target.contains(&m)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Source;

    fn t(source: Source, title: &str, artists: &[&str], secs: u64) -> Track {
        Track {
            id: title.into(),
            source,
            title: title.into(),
            artists: artists.iter().map(|a| a.to_string()).collect(),
            album: None,
            duration_ms: secs * 1000,
            artwork: None,
            isrc: None,
        }
    }

    #[test]
    fn picks_the_original_over_live_and_remix() {
        let target = t(Source::Spotify, "Morning Dew", &["Xavier Wulf"], 152);
        let candidates = vec![
            t(Source::YoutubeMusic, "Morning Dew (Live)", &["Xavier Wulf"], 170),
            t(Source::YoutubeMusic, "Morning Dew Remix", &["Someone Else"], 155),
            t(Source::YoutubeMusic, "Xavier Wulf - Morning Dew (Official Audio)", &[], 153),
        ];
        let best = best_match(&target, &candidates).expect("deveria achar");
        assert_eq!(best.title, "Xavier Wulf - Morning Dew (Official Audio)");
    }

    #[test]
    fn refuses_when_nothing_is_close_enough() {
        let target = t(Source::Spotify, "worth it", &["xaviersobased"], 92);
        let candidates = vec![t(Source::YoutubeMusic, "Something Else Entirely", &["Other"], 240)];
        assert!(best_match(&target, &candidates).is_none());
    }

    #[test]
    fn handles_accents_and_punctuation() {
        let target = t(Source::Spotify, "Coração de Gelo", &["WIU"], 180);
        let candidate = t(Source::YoutubeMusic, "WIU — CORAÇÃO DE GELO", &[], 181);
        assert!(score(&target, &candidate) > 0.9);
    }

    #[test]
    fn builds_query_with_main_artist() {
        let target = t(Source::Spotify, "Smoke", &["Xavier Wulf", "Chris Travis"], 100);
        assert_eq!(fallback_query(&target), "Xavier Wulf Smoke");
    }

    #[test]
    fn retries_with_every_artist_and_then_the_isrc() {
        let mut target = t(Source::Spotify, "Headache", &["milkshakes", "yen archive"], 72);
        target.isrc = Some("AEA0Q2482013".into());
        assert_eq!(retry_queries(&target), ["Headache milkshakes yen archive", "AEA0Q2482013"]);
    }

    #[test]
    fn a_list_keeps_one_copy_of_each_song() {
        let mut album = t(Source::YoutubeMusic, "Daylily", &["Movements"], 200);
        album.id = "album".into();
        let mut single = t(Source::YoutubeMusic, "DAYLILY (Official Video)", &["Movements"], 203);
        single.id = "single".into();
        let live = t(Source::YoutubeMusic, "Daylily (Live)", &["Movements"], 230);
        let other_artist = t(Source::YoutubeMusic, "Daylily", &["Outra Banda"], 180);
        let playing = t(Source::YoutubeMusic, "Perfect", &["Dance Gavin Dance"], 179);
        let same_as_playing = t(Source::YoutubeMusic, "Perfect", &["Dance Gavin Dance"], 181);

        let list = distinct_songs(&playing, vec![same_as_playing, album, single, live, other_artist]);
        let kept: Vec<_> = list.iter().map(|t| (t.title.as_str(), t.artists[0].as_str())).collect();
        assert_eq!(kept, [("Daylily", "Movements"), ("Daylily (Live)", "Movements"), ("Daylily", "Outra Banda")]);
    }

    #[test]
    fn artists_glued_in_one_name_still_match() {
        // Algumas páginas do Spotify juntam os artistas num texto só, com espaço não separável.
        let target = t(Source::Spotify, "Headache", &["milkshakes,\u{a0}yen archive"], 72);
        let candidate = t(Source::YoutubeMusic, "Headache", &["milkshakes"], 72);
        assert!(score(&target, &candidate) > 0.99);
    }
}

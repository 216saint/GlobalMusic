//! InnerTube: a API interna que o próprio music.youtube.com usa.
//!
//! As respostas são árvores JSON enormes e mudam de layout com frequência.
//! Em vez de seguir um caminho fixo (`contents.tabbed....[0]...`), procuramos
//! **recursivamente** os "renderers" que interessam:
//! - `musicResponsiveListItemRenderer` → uma faixa (busca, playlists, curtidas);
//! - `musicTwoRowItemRenderer` → um card (playlists da biblioteca).
//! Assim, se o YouTube mover a lista de lugar, o parser continua achando.

use serde_json::{json, Value};

use crate::artwork::{self, Sized};
use crate::models::{Playlist, Source, Track};

pub const API_URL: &str = "https://music.youtube.com/youtubei/v1";
const CLIENT_NAME: &str = "WEB_REMIX";
const CLIENT_VERSION: &str = "1.20251020.01.00";
pub const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36";

/// Filtro "Músicas" da busca (o mesmo que o site usa ao clicar no chip "Músicas").
pub const SONGS_FILTER: &str = "EgWKAQIIAWoMEA4QChADEAQQCRAF";

/// Corpo padrão de toda requisição: quem somos (cliente web do YT Music) + o pedido.
pub fn body(extra: Value) -> Value {
    let mut body = json!({
        "context": { "client": { "clientName": CLIENT_NAME, "clientVersion": CLIENT_VERSION, "hl": "pt", "gl": "BR" } }
    });
    if let (Some(target), Value::Object(fields)) = (body.as_object_mut(), extra) {
        target.extend(fields);
    }
    body
}

pub fn parse_tracks(response: &Value) -> Vec<Track> {
    let mut items = Vec::new();
    collect(response, "musicResponsiveListItemRenderer", &mut items);
    items.into_iter().filter_map(parse_track).collect()
}

pub fn parse_playlists(response: &Value) -> Vec<Playlist> {
    let mut items = Vec::new();
    collect(response, "musicTwoRowItemRenderer", &mut items);
    items.into_iter().filter_map(parse_playlist).collect()
}

/// Junta, em ordem, todo objeto que aparece sob a chave `key` em qualquer profundidade.
fn collect<'a>(value: &'a Value, key: &str, out: &mut Vec<&'a Value>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                if k == key {
                    out.push(v);
                } else {
                    collect(v, key, out);
                }
            }
        }
        Value::Array(list) => list.iter().for_each(|v| collect(v, key, out)),
        _ => {}
    }
}

#[derive(Debug)]
struct Run<'a> {
    text: &'a str,
    page_type: Option<&'a str>,
}

fn runs(text: &Value) -> Vec<Run<'_>> {
    text["runs"]
        .as_array()
        .map(|runs| {
            runs.iter()
                .filter_map(|r| {
                    Some(Run {
                        text: r["text"].as_str()?,
                        page_type: r
                            .pointer("/navigationEndpoint/browseEndpoint/browseEndpointContextSupportedConfigs/browseEndpointContextMusicConfig/pageType")
                            .and_then(Value::as_str),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn flex_column(item: &Value, index: usize) -> Vec<Run<'_>> {
    runs(&item["flexColumns"][index]["musicResponsiveListItemFlexColumnRenderer"]["text"])
}

fn parse_track(item: &Value) -> Option<Track> {
    let video_id = item
        .pointer("/playlistItemData/videoId")
        .or_else(|| item.pointer("/overlay/musicItemThumbnailOverlayRenderer/content/musicPlayButtonRenderer/playNavigationEndpoint/watchEndpoint/videoId"))
        .and_then(Value::as_str)?;
    let title = flex_column(item, 0).first()?.text.to_owned();

    // Todas as colunas de texto juntas: "Artista e Artista • Álbum • 3:14".
    let columns = item["flexColumns"].as_array().map_or(0, Vec::len);
    let meta: Vec<Run> = (1..columns).flat_map(|i| flex_column(item, i)).collect();

    let mut artists: Vec<String> = meta
        .iter()
        .filter(|r| r.page_type == Some("MUSIC_PAGE_TYPE_ARTIST"))
        .map(|r| r.text.to_owned())
        .collect();
    if artists.is_empty() {
        // Uploads sem página de artista: o primeiro trecho antes de " • " é o canal/artista.
        if let Some(first) = meta.first().filter(|r| r.page_type.is_none() && parse_duration(r.text).is_none()) {
            artists.push(first.text.to_owned());
        }
    }
    let album = meta
        .iter()
        .find(|r| r.page_type == Some("MUSIC_PAGE_TYPE_ALBUM"))
        .map(|r| r.text.to_owned());

    // Na busca a duração vem no fim do texto; em playlists, numa coluna fixa.
    let fixed = runs(&item["fixedColumns"][0]["musicResponsiveListItemFixedColumnRenderer"]["text"]);
    let duration_ms = meta.iter().chain(fixed.iter()).rev().find_map(|r| parse_duration(r.text)).unwrap_or(0);

    Some(Track {
        id: video_id.to_owned(),
        source: Source::YoutubeMusic,
        title,
        artists,
        album,
        duration_ms,
        artwork: artwork_of(&item["thumbnail"]["musicThumbnailRenderer"]),
        isrc: None,
    })
}

fn parse_playlist(item: &Value) -> Option<Playlist> {
    // Só cards que abrem uma playlist ("VL" + id); ignora álbuns, artistas e o botão "Nova playlist".
    let browse_id = item.pointer("/navigationEndpoint/browseEndpoint/browseId").and_then(Value::as_str)?;
    let id = browse_id.strip_prefix("VL")?;
    let name = runs(&item["title"]).first()?.text.to_owned();
    let subtitle: Vec<Run> = runs(&item["subtitle"]);
    let track_count = subtitle.iter().rev().find_map(|r| leading_number(r.text));
    let owner = subtitle
        .iter()
        .map(|r| r.text)
        .find(|t| !t.trim().is_empty() && *t != " • " && leading_number(t).is_none() && *t != "Playlist")
        .map(str::to_owned);

    Some(Playlist {
        id: id.to_owned(),
        source: Source::YoutubeMusic,
        name,
        owner,
        track_count,
        artwork: artwork_of(&item["thumbnailRenderer"]["musicThumbnailRenderer"]),
    })
}

fn artwork_of(thumbnail_renderer: &Value) -> Option<artwork::Artwork> {
    let thumbs = thumbnail_renderer["thumbnail"]["thumbnails"].as_array()?;
    let sized: Vec<Sized> = thumbs
        .iter()
        .filter_map(|t| Some(Sized { url: t["url"].as_str()?, px: t["width"].as_u64().map(|w| w as u32) }))
        .collect();
    artwork::from_youtube(&sized)
}

/// "3:14" → 194000 ms; "1:02:03" → 3723000 ms.
pub fn parse_duration(text: &str) -> Option<u64> {
    let parts: Vec<u64> = text.trim().split(':').map(|p| p.parse().ok()).collect::<Option<_>>()?;
    if !(2..=3).contains(&parts.len()) || parts[1..].iter().any(|&p| p >= 60) {
        return None;
    }
    Some(parts.iter().fold(0, |acc, p| acc * 60 + p) * 1000)
}

/// "58 músicas" → 58; "1.204 faixas" → 1204.
fn leading_number(text: &str) -> Option<u32> {
    let text = text.trim();
    let end = text.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == ',')).unwrap_or(text.len());
    let (number, rest) = text.split_at(end);
    let digits: String = number.chars().filter(char::is_ascii_digit).collect();
    // Precisa de uma palavra depois ("58 músicas"); número sozinho pode ser ano.
    if digits.is_empty() || !rest.trim_start().chars().next().is_some_and(char::is_alphabetic) {
        return None;
    }
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Resposta real da busca (2026-10-26), enxugada. Ver docs/diario.md.
    const SEARCH_FIXTURE: &str = include_str!("../fixtures/ytm_search.json");

    #[test]
    fn parses_real_search_response() {
        let response: Value = serde_json::from_str(SEARCH_FIXTURE).unwrap();
        let tracks = parse_tracks(&response);

        assert_eq!(tracks.len(), 3);
        let first = &tracks[0];
        assert_eq!(first.id, "QsVX3OjGp58");
        assert_eq!(first.title, "Morning Dew");
        assert_eq!(first.artists, ["Xavier Wulf", "Bones"]);
        assert_eq!(first.album.as_deref(), Some("Caves"));
        assert_eq!(first.duration_ms, 194_000);
        let art = first.artwork.as_ref().unwrap();
        assert!(art.thumb.ends_with("=w160-h160-l90-rj") && art.full.ends_with("=w544-h544-l90-rj"));

        assert_eq!(tracks[2].title, "Just Know (part. Black Smurf)");
        assert_eq!(tracks[2].artists, ["Xavier Wulf"]);
    }

    #[test]
    fn parses_library_playlist_cards() {
        let response = json!({ "items": [
            { "musicTwoRowItemRenderer": {
                "title": { "runs": [{ "text": "Independentes BR" }] },
                "subtitle": { "runs": [{ "text": "Você" }, { "text": " • " }, { "text": "58 músicas" }] },
                "navigationEndpoint": { "browseEndpoint": { "browseId": "VLPLabc123" } },
                "thumbnailRenderer": { "musicThumbnailRenderer": { "thumbnail": { "thumbnails": [
                    { "url": "https://lh3.googleusercontent.com/x=w226-h226-l90-rj", "width": 226 } ] } } }
            }},
            { "musicTwoRowItemRenderer": {
                "title": { "runs": [{ "text": "Nova playlist" }] },
                "navigationEndpoint": { "createPlaylistEndpoint": {} }
            }}
        ]});
        let playlists = parse_playlists(&response);
        assert_eq!(playlists.len(), 1, "o botão 'Nova playlist' não é uma playlist");
        assert_eq!(playlists[0].id, "PLabc123");
        assert_eq!(playlists[0].owner.as_deref(), Some("Você"));
        assert_eq!(playlists[0].track_count, Some(58));
    }

    #[test]
    fn durations() {
        assert_eq!(parse_duration("3:14"), Some(194_000));
        assert_eq!(parse_duration("1:02:03"), Some(3_723_000));
        assert_eq!(parse_duration("Caves"), None);
        assert_eq!(parse_duration("3:99"), None);
    }

    #[test]
    fn track_counts() {
        assert_eq!(leading_number("58 músicas"), Some(58));
        assert_eq!(leading_number("1.204 faixas"), Some(1204));
        assert_eq!(leading_number("2025"), None, "ano não é contagem");
    }

    #[test]
    fn request_body_keeps_client_context() {
        let b = body(json!({ "query": "x" }));
        assert_eq!(b["context"]["client"]["clientName"], "WEB_REMIX");
        assert_eq!(b["query"], "x");
    }
}

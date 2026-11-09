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
/// Filtro "Vídeos": inclui uploads de usuários (slowed, mashups…) que não estão no catálogo de músicas.
pub const VIDEOS_FILTER: &str = "EgWKAQIQAWoMEA4QChADEAQQCRAF";

/// Intercala músicas e vídeos (m1, v1, m2, v2…) sem repetir o mesmo vídeo, até `limit`.
pub fn interleave(songs: Vec<Track>, videos: Vec<Track>, limit: usize) -> Vec<Track> {
    let mut out: Vec<Track> = Vec::with_capacity(limit);
    let (mut songs, mut videos) = (songs.into_iter(), videos.into_iter());
    while out.len() < limit {
        let (song, video) = (songs.next(), videos.next());
        if song.is_none() && video.is_none() {
            break;
        }
        for track in [song, video].into_iter().flatten() {
            if out.len() < limit && !out.iter().any(|t| t.id == track.id) {
                out.push(track);
            }
        }
    }
    out
}

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

/// Na resposta de `next` (a tela "tocando agora" do site), o id da aba "Letra" (`MPLYt…`).
/// Música sem letra não tem essa aba.
pub fn lyrics_browse_id(next: &Value) -> Option<String> {
    let mut ids = Vec::new();
    collect(next, "browseId", &mut ids);
    ids.into_iter().filter_map(Value::as_str).find(|id| id.starts_with("MPLY")).map(str::to_owned)
}

/// A letra (texto corrido) e o crédito ("Fonte: Musixmatch") da aba "Letra".
pub fn parse_lyrics(browse: &Value) -> Option<(String, Option<String>)> {
    let mut shelves = Vec::new();
    collect(browse, "musicDescriptionShelfRenderer", &mut shelves);
    let shelf = shelves.first()?;
    let text: String = runs(&shelf["description"]).iter().map(|r| r.text).collect();
    let credit: String = runs(&shelf["footer"]).iter().map(|r| r.text).collect();
    let text = text.trim();
    (!text.is_empty()).then(|| (text.to_owned(), Some(credit.trim().to_owned()).filter(|c| !c.is_empty())))
}

/// Junta, em ordem, todo objeto que aparece sob a chave `key` em qualquer profundidade.
/// `counterpart` fica de fora: é a outra versão do mesmo item (o clipe de uma música), não outro item.
fn collect<'a>(value: &'a Value, key: &str, out: &mut Vec<&'a Value>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                if k == key {
                    out.push(v);
                } else if k != "counterpart" {
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
        // Vídeo enviado por alguém: o "artista" é o canal.
        artists.extend(meta.iter().filter(|r| r.page_type == Some("MUSIC_PAGE_TYPE_USER_CHANNEL")).map(|r| r.text.to_owned()));
    }
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

/// Faixas do "rádio" de um vídeo: a fila que o site monta ao tocar uma música (`next` com
/// `playlistId: RDAMVM<id>`). Sem o próprio vídeo, que vem primeiro, e sem repetidas.
pub fn parse_radio(next: &Value, seed: &str) -> Vec<Track> {
    let mut items = Vec::new();
    collect(next, "playlistPanelVideoRenderer", &mut items);
    let mut seen = std::collections::HashSet::from([seed.to_owned()]);
    items
        .into_iter()
        .filter_map(parse_panel_video)
        .filter(|t| seen.insert(t.id.clone()))
        .collect()
}

/// Um item da fila do player do site: "Artista • Álbum • Ano" (música) ou "Canal • views" (vídeo).
fn parse_panel_video(item: &Value) -> Option<Track> {
    let id = item["videoId"].as_str()?;
    let title = runs(&item["title"]).first()?.text.to_owned();
    let byline = runs(&item["longBylineText"]);
    let mut artists: Vec<String> = byline
        .iter()
        .filter(|r| r.page_type == Some("MUSIC_PAGE_TYPE_ARTIST"))
        .map(|r| r.text.to_owned())
        .collect();
    if artists.is_empty() {
        artists.extend(runs(&item["shortBylineText"]).first().map(|r| r.text.to_owned()));
    }
    Some(Track {
        id: id.to_owned(),
        source: Source::YoutubeMusic,
        title,
        artists,
        album: byline.iter().find(|r| r.page_type == Some("MUSIC_PAGE_TYPE_ALBUM")).map(|r| r.text.to_owned()),
        duration_ms: runs(&item["lengthText"]).first().and_then(|r| parse_duration(r.text)).unwrap_or(0),
        artwork: artwork_of(item),
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
        url: None,
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

    fn video(id: &str) -> Track {
        Track {
            id: id.into(),
            source: Source::YoutubeMusic,
            title: id.into(),
            artists: vec![],
            album: None,
            duration_ms: 0,
            artwork: None,
            isrc: None,
        }
    }

    #[test]
    fn interleaves_songs_and_videos_without_repeats() {
        let ids = |tracks: Vec<Track>| tracks.into_iter().map(|t| t.id).collect::<Vec<_>>();
        let songs = vec![video("m1"), video("m2"), video("m3")];
        let videos = vec![video("v1"), video("m2"), video("v2")];
        assert_eq!(ids(interleave(songs.clone(), videos.clone(), 10)), ["m1", "v1", "m2", "m3", "v2"]);
        assert_eq!(ids(interleave(songs, videos, 3)), ["m1", "v1", "m2"]);
        assert_eq!(ids(interleave(vec![], vec![video("v1")], 10)), ["v1"]);
    }

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

    /// Formato real das respostas de `next` e `browse` (2026-10-29), enxugado.
    #[test]
    fn finds_and_parses_the_lyrics_tab() {
        let next = json!({ "contents": { "tabs": [
            { "tabRenderer": { "endpoint": { "browseEndpoint": { "browseId": "FEmusic_queue" } } } },
            { "tabRenderer": { "endpoint": { "browseEndpoint": { "browseId": "MPLYt_wUbiMNbaE6O" } } } }
        ]}});
        assert_eq!(lyrics_browse_id(&next).as_deref(), Some("MPLYt_wUbiMNbaE6O"));
        assert_eq!(lyrics_browse_id(&json!({ "contents": {} })), None, "sem aba de letra");

        let browse = json!({ "contents": { "sectionListRenderer": { "contents": [{ "musicDescriptionShelfRenderer": {
            "description": { "runs": [{ "text": "Primeiro verso\nSegundo verso\n" }] },
            "footer": { "runs": [{ "text": "Fonte: Musixmatch" }] }
        }}]}}});
        let (text, credit) = parse_lyrics(&browse).unwrap();
        assert_eq!(text, "Primeiro verso\nSegundo verso");
        assert_eq!(credit.as_deref(), Some("Fonte: Musixmatch"));
    }

    /// Formato real do `next` com `playlistId: RDAMVM<id>` (2026-11-01), enxugado.
    #[test]
    fn parses_the_radio_without_the_seed_or_repeats() {
        let artist = |name: &str| json!({ "text": name, "navigationEndpoint": { "browseEndpoint": {
            "browseEndpointContextSupportedConfigs": { "browseEndpointContextMusicConfig": { "pageType": "MUSIC_PAGE_TYPE_ARTIST" } } } } });
        let item = |id: &str, title: &str, byline: Value| json!({ "playlistPanelVideoRenderer": {
            "videoId": id,
            "title": { "runs": [{ "text": title }] },
            "longBylineText": { "runs": byline },
            "shortBylineText": { "runs": [{ "text": "Canal" }] },
            "lengthText": { "runs": [{ "text": "3:24" }] },
            "thumbnail": { "thumbnails": [{ "url": "https://i.ytimg.com/vi/x/hqdefault.jpg", "width": 400, "height": 225 }] }
        }});
        let next = json!({ "contents": { "playlistPanelRenderer": { "contents": [
            item("seed", "A que tocou", json!([artist("Rick Astley")])),
            item("a1", "Together Forever", json!([artist("Rick Astley"), { "text": " • " }, { "text": "208 mi de visualizações" }])),
            item("a1", "Together Forever", json!([artist("Rick Astley")])),
            item("v2", "Upload de fã", json!([{ "text": "Alguém" }, { "text": " • " }, { "text": "3 mil visualizações" }])),
        ]}}});
        let radio = parse_radio(&next, "seed");
        assert_eq!(radio.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["a1", "v2"]);
        assert_eq!(radio[0].artists, ["Rick Astley"]);
        assert_eq!(radio[0].duration_ms, 204_000);
        assert!(radio[0].artwork.is_some());
        assert_eq!(radio[1].artists, ["Canal"], "sem página de artista: o canal");
    }

    #[test]
    fn the_radio_takes_the_main_item_and_not_its_counterpart() {
        let item = |id: &str| json!({ "playlistPanelVideoRenderer": { "videoId": id, "title": { "runs": [{ "text": "Perfect" }] } } });
        let next = json!({ "contents": [{ "playlistPanelVideoWrapperRenderer": {
            "primaryRenderer": item("audio"),
            "counterpart": [{ "counterpartRenderer": item("clipe") }]
        }}]});
        assert_eq!(parse_radio(&next, "seed").iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["audio"]);
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

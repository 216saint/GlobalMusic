//! Playlists importadas por link (Spotify, YouTube Music ou SoundCloud): entram na Biblioteca como
//! as da pessoa, e ficam salvas num arquivo pequeno (`imported-playlists.json`).
//!
//! Nome e capa vêm do **oEmbed** (o mesmo endereço público que os sites usam para mostrar a prévia
//! de um link): não precisa de login nem gasta a cota da API. No SoundCloud usamos o `/resolve` da
//! API, que transforma o link no id da playlist. As faixas só são buscadas quando ela é aberta.

use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;
use tokio::sync::Mutex;

use crate::error::{Error, Result};
use crate::models::{Artwork, Playlist, Source};

const TIMEOUT: Duration = Duration::from_secs(10);

/// O que um link aponta.
#[derive(Debug, PartialEq)]
pub enum Link {
    Spotify(String),
    YoutubeMusic(String),
    /// O SoundCloud não põe o id no link: ele é resolvido pela API.
    Soundcloud(String),
}

/// Reconhece links de playlist dos três serviços (com ou sem `https://`, com parâmetros extras).
pub fn parse_link(text: &str) -> Result<Link> {
    let text = text.trim();
    if let Some(id) = text.strip_prefix("spotify:playlist:") {
        return Ok(Link::Spotify(id.to_owned()));
    }
    let with_scheme = if text.contains("://") { text.to_owned() } else { format!("https://{text}") };
    let url = url::Url::parse(&with_scheme).map_err(|_| unrecognized())?;
    let host = url.host_str().unwrap_or_default().trim_start_matches("www.").trim_start_matches("m.");
    let segments: Vec<&str> = url.path_segments().map(|s| s.filter(|p| !p.is_empty()).collect()).unwrap_or_default();
    match host {
        "open.spotify.com" => {
            // /playlist/{id} ou /intl-pt/playlist/{id}
            let at = segments.iter().position(|s| *s == "playlist").ok_or_else(unrecognized)?;
            let id = segments.get(at + 1).ok_or_else(unrecognized)?;
            Ok(Link::Spotify((*id).to_owned()))
        }
        "music.youtube.com" | "youtube.com" | "youtu.be" => {
            let list = url.query_pairs().find(|(k, _)| k == "list").map(|(_, v)| v.into_owned()).ok_or_else(unrecognized)?;
            Ok(Link::YoutubeMusic(list.trim_start_matches("VL").to_owned()))
        }
        "soundcloud.com" if segments.len() >= 3 && segments[1] == "sets" => {
            Ok(Link::Soundcloud(format!("https://soundcloud.com/{}/sets/{}", segments[0], segments[2])))
        }
        _ => Err(unrecognized()),
    }
}

fn unrecognized() -> Error {
    Error::Other("não reconheci esse link. Cole o link de uma playlist do Spotify, do YouTube Music ou do SoundCloud".into())
}

#[derive(Deserialize)]
struct OEmbed {
    title: String,
    author_name: Option<String>,
    thumbnail_url: Option<String>,
}

/// Nome, dono e capa de uma playlist do Spotify ou do YouTube, pelo oEmbed.
pub async fn describe(http: &reqwest::Client, source: Source, id: &str) -> Result<Playlist> {
    let (endpoint, page) = match source {
        Source::Spotify => ("https://open.spotify.com/oembed", format!("https://open.spotify.com/playlist/{id}")),
        _ => ("https://www.youtube.com/oembed", format!("https://www.youtube.com/playlist?list={id}")),
    };
    let response = http.get(endpoint).query(&[("url", page.as_str()), ("format", "json")]).timeout(TIMEOUT).send().await?;
    if !response.status().is_success() {
        return Err(Error::Other("não achei essa playlist. Ela é pública?".into()));
    }
    let info: OEmbed = response.json().await?;
    Ok(Playlist {
        id: id.to_owned(),
        source,
        name: info.title,
        owner: info.author_name,
        track_count: None,
        artwork: info.thumbnail_url.map(|url| Artwork { thumb: url.clone(), full: url }),
    })
}

/// As playlists importadas, guardadas em disco.
pub struct Imports {
    file: Option<PathBuf>,
    list: Mutex<Vec<Playlist>>,
}

impl Imports {
    pub fn load(file: Option<PathBuf>) -> Self {
        let list = file
            .as_deref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self { file, list: Mutex::new(list) }
    }

    pub async fn list(&self) -> Vec<Playlist> {
        self.list.lock().await.clone()
    }

    pub async fn contains(&self, source: Source, id: &str) -> bool {
        self.list.lock().await.iter().any(|p| p.source == source && p.id == id)
    }

    /// Adiciona (ou atualiza, se já estava importada) e devolve a lista.
    pub async fn add(&self, playlist: Playlist) -> Vec<Playlist> {
        let mut list = self.list.lock().await;
        list.retain(|p| !(p.source == playlist.source && p.id == playlist.id));
        list.insert(0, playlist);
        self.save(&list);
        list.clone()
    }

    pub async fn remove(&self, source: Source, id: &str) -> Vec<Playlist> {
        let mut list = self.list.lock().await;
        list.retain(|p| !(p.source == source && p.id == id));
        self.save(&list);
        list.clone()
    }

    fn save(&self, list: &[Playlist]) {
        if let (Some(path), Ok(json)) = (&self.file, serde_json::to_vec(list)) {
            if let Err(e) = std::fs::write(path, json) {
                log::info!("não gravei as playlists importadas: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_playlist_links_from_the_three_services() {
        assert_eq!(
            parse_link("https://open.spotify.com/intl-pt/playlist/37i9dQZF1DXcBWIGoYBM5M?si=abc").unwrap(),
            Link::Spotify("37i9dQZF1DXcBWIGoYBM5M".into())
        );
        assert_eq!(parse_link("spotify:playlist:abc123").unwrap(), Link::Spotify("abc123".into()));
        assert_eq!(
            parse_link("music.youtube.com/playlist?list=PLx1y2&si=z").unwrap(),
            Link::YoutubeMusic("PLx1y2".into())
        );
        assert_eq!(
            parse_link("https://www.youtube.com/watch?v=abc&list=OLAK5uy_q").unwrap(),
            Link::YoutubeMusic("OLAK5uy_q".into())
        );
        assert_eq!(
            parse_link("https://soundcloud.com/artista/sets/minha-lista?si=1&utm_source=x").unwrap(),
            Link::Soundcloud("https://soundcloud.com/artista/sets/minha-lista".into())
        );
    }

    #[test]
    fn rejects_links_that_are_not_playlists() {
        assert!(parse_link("https://open.spotify.com/track/123").is_err());
        assert!(parse_link("https://soundcloud.com/artista/uma-musica").is_err());
        assert!(parse_link("https://exemplo.com/playlist?list=1").is_err());
        assert!(parse_link("qualquer coisa").is_err());
    }
}

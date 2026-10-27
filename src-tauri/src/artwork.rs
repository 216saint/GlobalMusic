//! Capas em dois tamanhos. Uma capa 640x640 decodificada ocupa ~1,6 MB na
//! WebView; listas usam `thumb` e só o "tocando agora" carrega `full`.

use serde::{Deserialize, Serialize};

pub const THUMB_PX: u32 = 160;
pub const FULL_PX: u32 = 544;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Artwork {
    pub thumb: String,
    pub full: String,
}

/// Imagem com dimensão conhecida (largura; as capas são quadradas).
#[derive(Debug, Clone)]
pub struct Sized<'a> {
    pub url: &'a str,
    pub px: Option<u32>,
}

/// Spotify devolve 640/300/64 px: thumb = menor que ainda cobre THUMB_PX, full = maior.
pub fn from_spotify(images: &[Sized<'_>]) -> Option<Artwork> {
    let full = images.iter().max_by_key(|i| i.px.unwrap_or(0))?;
    let thumb = images
        .iter()
        .filter(|i| i.px.is_some_and(|px| px >= THUMB_PX))
        .min_by_key(|i| i.px)
        .unwrap_or(full);
    Some(Artwork { thumb: thumb.url.to_owned(), full: full.url.to_owned() })
}

/// YouTube Music: capas em `lh3.googleusercontent.com` aceitam qualquer tamanho
/// trocando o sufixo `=w120-h120-...`. Miniaturas `i.ytimg.com` usam a lista.
pub fn from_youtube(thumbnails: &[Sized<'_>]) -> Option<Artwork> {
    let largest = thumbnails.iter().max_by_key(|t| t.px.unwrap_or(0))?;
    if let Some(base) = googleusercontent_base(largest.url) {
        return Some(Artwork {
            thumb: format!("{base}=w{THUMB_PX}-h{THUMB_PX}-l90-rj"),
            full: format!("{base}=w{FULL_PX}-h{FULL_PX}-l90-rj"),
        });
    }
    from_spotify(thumbnails)
}

fn googleusercontent_base(url: &str) -> Option<&str> {
    if !url.contains(".googleusercontent.com/") {
        return None;
    }
    Some(url.rsplit_once('=').map_or(url, |(base, _)| base))
}

/// SoundCloud devolve `...-large.jpg` (100px); o mesmo arquivo existe em `-t500x500`.
pub fn from_soundcloud(artwork_url: &str) -> Artwork {
    let sized = |size: &str| artwork_url.replacen("-large.", &format!("-{size}."), 1);
    Artwork { thumb: sized("t200x200"), full: sized("t500x500") }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(url: &str, px: u32) -> Sized<'_> {
        Sized { url, px: Some(px) }
    }

    #[test]
    fn spotify_picks_300_for_thumb_and_640_for_full() {
        let images = [s("a640", 640), s("a300", 300), s("a64", 64)];
        assert_eq!(from_spotify(&images), Some(Artwork { thumb: "a300".into(), full: "a640".into() }));
        assert_eq!(from_spotify(&[]), None);
    }

    #[test]
    fn spotify_with_only_small_images_reuses_largest() {
        let art = from_spotify(&[s("a64", 64)]).unwrap();
        assert_eq!((art.thumb.as_str(), art.full.as_str()), ("a64", "a64"));
    }

    #[test]
    fn youtube_music_rewrites_googleusercontent_size() {
        let thumbs = [
            s("https://lh3.googleusercontent.com/abc=w60-h60-l90-rj", 60),
            s("https://lh3.googleusercontent.com/abc=w120-h120-l90-rj", 120),
        ];
        let art = from_youtube(&thumbs).unwrap();
        assert_eq!(art.thumb, "https://lh3.googleusercontent.com/abc=w160-h160-l90-rj");
        assert_eq!(art.full, "https://lh3.googleusercontent.com/abc=w544-h544-l90-rj");
    }

    #[test]
    fn youtube_video_thumbnails_fall_back_to_list() {
        let thumbs = [s("https://i.ytimg.com/vi/x/default.jpg", 120), s("https://i.ytimg.com/vi/x/hqdefault.jpg", 480)];
        let art = from_youtube(&thumbs).unwrap();
        assert!(art.thumb.ends_with("default.jpg") && art.full.ends_with("hqdefault.jpg"));
    }

    #[test]
    fn soundcloud_upgrades_large_variant() {
        let art = from_soundcloud("https://i1.sndcdn.com/artworks-000-abc-large.jpg");
        assert_eq!(art.thumb, "https://i1.sndcdn.com/artworks-000-abc-t200x200.jpg");
        assert_eq!(art.full, "https://i1.sndcdn.com/artworks-000-abc-t500x500.jpg");
    }
}

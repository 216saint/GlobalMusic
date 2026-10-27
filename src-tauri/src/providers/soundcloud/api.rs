//! Formato dos dados da API do site do SoundCloud (`api-v2.soundcloud.com`)
//! e conversão para os nossos modelos.

use serde::Deserialize;

use crate::artwork;
use crate::models::{Playlist, Source, Track};

#[derive(Debug, Deserialize)]
pub struct Collection<T> {
    pub collection: Vec<T>,
}

#[derive(Debug, Deserialize)]
pub struct User {
    pub id: u64,
    pub username: String,
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub likes_count: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScTrack {
    pub id: u64,
    /// Ausente nos "stubs" que a API devolve dentro de playlists grandes.
    pub title: Option<String>,
    pub user: Option<TrackUser>,
    #[serde(default)]
    pub duration: u64,
    pub artwork_url: Option<String>,
    /// "ALLOW", "MONETIZE", "SNIP" (só prévia de 30s, exige Go+) ou "BLOCK".
    pub policy: Option<String>,
    pub media: Option<Media>,
    pub track_authorization: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TrackUser {
    pub username: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Media {
    #[serde(default)]
    pub transcodings: Vec<Transcoding>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Transcoding {
    pub url: String,
    #[serde(default)]
    pub snipped: bool,
    pub format: Format,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Format {
    /// "progressive" (um arquivo só) ou "hls" (vários pedaços + playlist .m3u8).
    pub protocol: String,
    pub mime_type: String,
}

#[derive(Debug, Deserialize)]
pub struct ScPlaylist {
    pub id: u64,
    pub title: String,
    pub user: Option<TrackUser>,
    pub track_count: Option<u32>,
    pub artwork_url: Option<String>,
    #[serde(default)]
    pub tracks: Vec<ScTrack>,
}

#[derive(Debug, Deserialize)]
pub struct Like {
    pub track: Option<ScTrack>,
}

/// Resposta de `transcoding.url`: a URL final do áudio (ou da playlist HLS).
#[derive(Debug, Deserialize)]
pub struct StreamLocation {
    pub url: String,
}

/// Como tocar uma faixa.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StreamKind {
    /// MP3 inteiro numa URL: dá para tocar enquanto baixa.
    Progressive,
    /// Playlist HLS de pedaços MP3: baixamos os pedaços e juntamos.
    HlsMp3,
    /// Playlist HLS de AAC em MP4 fragmentado: `init.mp4` (`#EXT-X-MAP`) + pedaços `.m4s`.
    /// Juntos formam um MP4 fragmentado comum, que o decodificador lê como o m4a do YouTube.
    HlsAac,
}

impl StreamKind {
    /// Dica de formato para o decodificador.
    pub fn hint(self) -> &'static str {
        match self {
            StreamKind::Progressive | StreamKind::HlsMp3 => "mp3",
            StreamKind::HlsAac => "m4a",
        }
    }
}

impl ScTrack {
    /// Tocável por inteiro? (exclui bloqueadas e prévias de 30s)
    pub fn is_playable(&self) -> bool {
        self.policy.as_deref() != Some("BLOCK") && self.policy.as_deref() != Some("SNIP") && self.best_transcoding().is_some()
    }

    /// Ordem: MP3 progressivo (um arquivo só) → AAC em HLS (o que o SoundCloud mais oferece hoje;
    /// muitas faixas já não têm MP3) → MP3 em HLS. Fora: Opus (o decodificador não lê) e
    /// `audio/mpegurl` (playlist adaptativa), que **começa com "audio/mpeg"** e já foi confundida
    /// com MP3, dando 404.
    pub fn best_transcoding(&self) -> Option<(&Transcoding, StreamKind)> {
        let list = &self.media.as_ref()?.transcodings;
        let mime = |t: &Transcoding| t.format.mime_type.split(';').next().unwrap_or_default().trim().to_owned();
        let pick = |protocol: &str, mime_type: &str| {
            list.iter().find(|t| !t.snipped && t.format.protocol == protocol && mime(t) == mime_type)
        };
        pick("progressive", "audio/mpeg")
            .map(|t| (t, StreamKind::Progressive))
            .or_else(|| pick("hls", "audio/mp4").map(|t| (t, StreamKind::HlsAac)))
            .or_else(|| pick("hls", "audio/mpeg").map(|t| (t, StreamKind::HlsMp3)))
    }

    pub fn into_track(self) -> Option<Track> {
        let user = self.user?;
        // Sem capa própria, o SoundCloud mostra o avatar de quem publicou.
        let art = self.artwork_url.or(user.avatar_url);
        Some(Track {
            id: self.id.to_string(),
            source: Source::Soundcloud,
            title: self.title?,
            artists: vec![user.username],
            album: None,
            duration_ms: self.duration,
            artwork: art.as_deref().map(artwork::from_soundcloud),
            isrc: None,
        })
    }
}

impl ScPlaylist {
    pub fn into_playlist(self) -> Playlist {
        Playlist {
            id: self.id.to_string(),
            source: Source::Soundcloud,
            name: self.title,
            owner: self.user.map(|u| u.username),
            track_count: self.track_count,
            artwork: self.artwork_url.as_deref().map(artwork::from_soundcloud),
        }
    }
}

/// URLs dos pedaços de uma playlist HLS (`.m3u8`): as linhas que não começam com `#`.
/// Arquivos de uma playlist HLS, na ordem de tocar: o cabeçalho (`#EXT-X-MAP:URI="…"`, no AAC)
/// e depois os pedaços.
pub fn hls_parts(m3u8: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    for line in m3u8.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if let Some(map) = line.strip_prefix("#EXT-X-MAP:") {
            if let Some(uri) = map.split("URI=\"").nth(1).and_then(|rest| rest.split('"').next()) {
                parts.push(uri);
            }
        } else if !line.starts_with('#') {
            parts.push(line);
        }
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRACK: &str = r#"{
        "id": 123, "title": "love hate", "duration": 118000, "policy": "ALLOW",
        "artwork_url": null, "track_authorization": "tok",
        "user": { "username": "xaviersobased", "avatar_url": "https://i1.sndcdn.com/avatars-1-large.jpg" },
        "media": { "transcodings": [
            { "url": "https://api-v2.soundcloud.com/media/1/hls-opus", "snipped": false,
              "format": { "protocol": "hls", "mime_type": "audio/ogg; codecs=\"opus\"" } },
            { "url": "https://api-v2.soundcloud.com/media/1/hls-mp3", "snipped": false,
              "format": { "protocol": "hls", "mime_type": "audio/mpeg" } }
        ]}
    }"#;

    #[test]
    fn maps_track_using_avatar_when_there_is_no_artwork() {
        let track: ScTrack = serde_json::from_str(TRACK).unwrap();
        assert!(track.is_playable());
        let model = track.into_track().unwrap();
        assert_eq!(model.id, "123");
        assert_eq!(model.artists, ["xaviersobased"]);
        assert_eq!(model.duration_ms, 118_000);
        assert!(model.artwork.unwrap().full.ends_with("avatars-1-t500x500.jpg"));
    }

    #[test]
    fn prefers_progressive_mp3_then_hls_mp3_and_skips_opus() {
        let track: ScTrack = serde_json::from_str(TRACK).unwrap();
        let (chosen, kind) = track.best_transcoding().unwrap();
        assert_eq!(kind, StreamKind::HlsMp3);
        assert!(chosen.url.ends_with("hls-mp3"));
    }

    /// Formatos reais de uma faixa do SoundCloud em out/2026: só AAC e a playlist adaptativa.
    #[test]
    fn plays_aac_hls_and_never_mistakes_mpegurl_for_mp3() {
        let track: ScTrack = serde_json::from_str(
            r#"{ "id": 1274024050, "policy": "ALLOW", "media": { "transcodings": [
                { "url": "https://api-v2.soundcloud.com/media/x/aac160/stream/hls", "snipped": false,
                  "format": { "protocol": "hls", "mime_type": "audio/mp4; codecs=\"mp4a.40.2\"" } },
                { "url": "https://api-v2.soundcloud.com/media/x/aac96/stream/hls", "snipped": false,
                  "format": { "protocol": "hls", "mime_type": "audio/mp4; codecs=\"mp4a.40.2\"" } },
                { "url": "https://api-v2.soundcloud.com/media/x/abr/stream/hls", "snipped": false,
                  "format": { "protocol": "hls", "mime_type": "audio/mpegurl" } }
            ]}}"#,
        )
        .unwrap();
        let (chosen, kind) = track.best_transcoding().unwrap();
        assert_eq!(kind, StreamKind::HlsAac);
        assert!(chosen.url.contains("aac160"), "a melhor qualidade vem primeiro na lista");
        assert!(track.is_playable());
    }

    #[test]
    fn previews_and_blocked_tracks_are_not_playable() {
        let mut track: ScTrack = serde_json::from_str(TRACK).unwrap();
        track.policy = Some("SNIP".into());
        assert!(!track.is_playable());
        track.policy = Some("BLOCK".into());
        assert!(!track.is_playable());
    }

    #[test]
    fn stubs_without_title_are_not_tracks_yet() {
        let stub: ScTrack = serde_json::from_str(r#"{ "id": 9 }"#).unwrap();
        assert!(stub.into_track().is_none());
    }

    #[test]
    fn reads_hls_segment_urls() {
        let m3u8 = "#EXTM3U\n#EXT-X-TARGETDURATION:10\n#EXTINF:10.0,\nhttps://cf-hls-media.sndcdn.com/a.mp3\n#EXTINF:10.0,\nhttps://cf-hls-media.sndcdn.com/b.mp3\n#EXT-X-ENDLIST\n";
        assert_eq!(hls_parts(m3u8), ["https://cf-hls-media.sndcdn.com/a.mp3", "https://cf-hls-media.sndcdn.com/b.mp3"]);
    }

    #[test]
    fn aac_hls_starts_with_its_init_segment() {
        let m3u8 = "#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-MAP:URI=\"https://p.sc/aac/init.mp4?expires=1\"\n#EXTINF:10.0,\nhttps://p.sc/aac/data000.m4s?expires=1\n#EXTINF:9.9,\nhttps://p.sc/aac/data001.m4s?expires=1\n#EXT-X-ENDLIST\n";
        assert_eq!(
            hls_parts(m3u8),
            ["https://p.sc/aac/init.mp4?expires=1", "https://p.sc/aac/data000.m4s?expires=1", "https://p.sc/aac/data001.m4s?expires=1"]
        );
    }
}

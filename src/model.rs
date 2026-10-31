//! Typed wire models shared with the NPLAY control service.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Track {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub album: String,
    #[serde(default)]
    pub duration: f64,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub seekable: bool,
    #[serde(default)]
    pub has_art: bool,
    #[serde(default)]
    pub meta: serde_json::Value,
}

fn default_kind() -> String {
    "track".into()
}

impl Track {
    pub fn ident(&self) -> String {
        if !self.key.is_empty() {
            self.key.clone()
        } else {
            format!("{}:{}", self.source, self.id)
        }
    }

    pub fn is_track(&self) -> bool {
        matches!(self.kind.as_str(), "track" | "radio" | "podcast" | "")
    }

    pub fn subtitle(&self) -> String {
        let mut parts = Vec::new();
        if !self.artist.is_empty() {
            parts.push(self.artist.clone());
        }
        if !self.album.is_empty() && self.album != self.title {
            parts.push(self.album.clone());
        }
        parts.join(" · ")
    }

    pub fn bitrate(&self) -> Option<i64> {
        self.meta
            .get("bitrate")
            .and_then(|v| v.as_i64())
            .filter(|v| *v > 0)
    }

    pub fn year(&self) -> Option<i64> {
        self.meta
            .get("year")
            .and_then(|v| v.as_i64())
            .filter(|v| *v > 0)
    }

    pub fn source_label(&self) -> &'static str {
        match self.source.as_str() {
            "local" => "LOCAL",
            "navidrome" => "NAVIDROME",
            "spotify" => "SPOTIFY",
            "sr" => "SVERIGES RADIO",
            "youtube" => "YOUTUBE",
            "radio" => "RADIO",
            "library" => "LIBRARY",
            _ => "SOURCE",
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Playback {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub transport: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub position: f64,
    #[serde(default)]
    pub duration: f64,
    #[serde(default)]
    pub seekable: bool,
    #[serde(default)]
    pub volume: f64,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub shuffle: String,
    #[serde(default)]
    pub repeat: String,
    #[serde(default)]
    pub current: Option<Track>,
    #[serde(default)]
    pub context_index: i32,
    #[serde(default)]
    pub context: Vec<Track>,
    #[serde(default)]
    pub queue: Vec<Track>,
    #[serde(default)]
    pub later: Vec<Track>,
    #[serde(default)]
    pub radio: serde_json::Value,
    #[serde(default)]
    pub sampled_at: f64,
}

impl Playback {
    pub fn is_playing(&self) -> bool {
        self.status == "playing"
    }
    pub fn is_loading(&self) -> bool {
        self.status == "loading"
    }
    pub fn status_label(&self) -> &'static str {
        match self.status.as_str() {
            "playing" => "PLAYING",
            "paused" => "PAUSED",
            "loading" => "LOADING",
            "stopped" => "STOPPED",
            _ => "STOPPED",
        }
    }
    pub fn current_key(&self) -> Option<&str> {
        self.current
            .as_ref()
            .map(|t| t.key.as_str())
            .filter(|k| !k.is_empty())
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct SourceInfo {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub configured: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Capabilities {
    #[serde(default)]
    pub seek: bool,
    #[serde(default)]
    pub equalizer: bool,
    #[serde(default)]
    pub visualization: bool,
    #[serde(default)]
    pub artwork: bool,
    #[serde(default)]
    pub drag_drop: bool,
    #[serde(default)]
    pub queue_edit: bool,
    #[serde(default)]
    pub playlists: bool,
    #[serde(default)]
    pub favorites: bool,
    #[serde(default)]
    pub ratings: bool,
    #[serde(default)]
    pub spotify: bool,
    #[serde(default)]
    pub navidrome: bool,
    #[serde(default)]
    pub youtube: bool,
    #[serde(default)]
    pub cava: bool,
    #[serde(default)]
    pub mpv: bool,
    #[serde(default)]
    pub backend_version: String,
    #[serde(default)]
    pub protocol_version: u32,
    #[serde(default)]
    pub messages: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct BackendInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub pid: u32,
    #[serde(default)]
    pub socket: String,
    #[serde(default)]
    pub started: f64,
    #[serde(default)]
    pub instance: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Snapshot {
    #[serde(default)]
    pub protocol_version: u32,
    #[serde(default)]
    pub backend: BackendInfo,
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub server_time: f64,
    #[serde(default)]
    pub playback: Playback,
    #[serde(default)]
    pub sources: Vec<SourceInfo>,
    #[serde(default)]
    pub capabilities: Capabilities,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct StateChanged {
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub playback: Playback,
    #[serde(default)]
    pub sources: Vec<SourceInfo>,
    #[serde(default)]
    pub capabilities: Capabilities,
    #[serde(default)]
    pub server_time: f64,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct BrowseResponse {
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub section: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub items: Vec<Track>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct SearchGroup {
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub items: Vec<Track>,
    #[serde(default)]
    pub error: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct SearchResponse {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub groups: Vec<SearchGroup>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct PlaylistInfo {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub subtitle: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct PlaylistsResponse {
    #[serde(default)]
    pub items: Vec<PlaylistInfo>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct PlaylistTracks {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub items: Vec<Track>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct NavidromeStatus {
    #[serde(default)]
    pub configured: bool,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub user: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct SpotifyStatus {
    #[serde(default)]
    pub configured: bool,
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct LibraryInfo {
    #[serde(default)]
    pub roots: Vec<String>,
    #[serde(default)]
    pub tracks: u64,
    #[serde(default)]
    pub last_scan: serde_json::Value,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct SystemInfo {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub protocol: u32,
    #[serde(default)]
    pub log: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct SettingsResponse {
    #[serde(default)]
    pub settings: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub navidrome: NavidromeStatus,
    #[serde(default)]
    pub spotify: SpotifyStatus,
    #[serde(default)]
    pub library: LibraryInfo,
    #[serde(default)]
    pub system: SystemInfo,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ArtworkResponse {
    #[serde(default)]
    pub mime: String,
    #[serde(default)]
    pub data: String,
    #[serde(default)]
    pub empty: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct VisualizationData {
    #[serde(default)]
    pub bars: Vec<u8>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct QueueResponse {
    #[serde(default)]
    pub items: Vec<Track>,
    #[serde(default)]
    pub later: Vec<Track>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_key_falls_back_to_source_and_id() {
        let t = Track {
            source: "local".into(),
            id: "abc".into(),
            ..Default::default()
        };
        assert_eq!(t.ident(), "local:abc");
    }

    #[test]
    fn subtitle_does_not_repeat_the_title_as_album() {
        let t = Track {
            title: "Song".into(),
            artist: "Artist".into(),
            album: "Song".into(),
            ..Default::default()
        };
        assert_eq!(t.subtitle(), "Artist");
    }

    #[test]
    fn playback_status_labels_are_stable() {
        let mut p = Playback::default();
        assert_eq!(p.status_label(), "STOPPED");
        p.status = "paused".into();
        assert_eq!(p.status_label(), "PAUSED");
        p.status = "playing".into();
        assert!(p.is_playing());
    }
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct IptvPlaylist {
    /// The URL we loaded this from, so it can be refreshed later.
    pub source_url: String,
    pub channels: Vec<IptvChannel>,
}

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct IptvChannel {
    /// Stable id per (playlist, channel). We use `tvg-id` when present,
    /// else `name|url` so lists stay consistent across refreshes.
    pub id: String,
    pub name: String,
    pub stream_url: String,
    pub logo_url: Option<String>,
    pub group: Option<String>,
    /// EPG channel id (matches `channel id=` in an XMLTV feed).
    pub tvg_id: Option<String>,
    pub tvg_name: Option<String>,
    pub tvg_country: Option<String>,
    pub tvg_language: Option<String>,
}

//! Public types exposed to callers via UniFFI.
//!
//! These are the Swift/Kotlin-friendly shapes. The internal wire types
//! (closer to the raw Stremio JSON) live in `wire.rs` and get mapped into
//! these before leaving the FFI boundary.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct Manifest {
    pub id: String,
    pub version: String,
    pub name: String,
    pub description: Option<String>,
    pub logo: Option<String>,
    pub background: Option<String>,
    pub types: Vec<String>,
    pub resources: Vec<String>,
    pub catalogs: Vec<Catalog>,
    pub id_prefixes: Vec<String>,
    /// Transport URL (the `manifest.json` endpoint), preserved for later calls.
    pub transport_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct Catalog {
    pub r#type: String,
    pub id: String,
    pub name: String,
    pub extra: Vec<ExtraProp>,
}

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct ExtraProp {
    pub name: String,
    pub is_required: bool,
    pub options: Option<Vec<String>>,
    pub options_limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct MetaPreview {
    pub id: String,
    pub r#type: String,
    pub name: String,
    pub poster: Option<String>,
    pub poster_shape: Option<String>,
    pub background: Option<String>,
    pub logo: Option<String>,
    pub description: Option<String>,
    pub imdb_rating: Option<String>,
    pub release_info: Option<String>,
    pub genres: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct Meta {
    pub id: String,
    pub r#type: String,
    pub name: String,
    pub poster: Option<String>,
    pub poster_shape: Option<String>,
    pub background: Option<String>,
    pub logo: Option<String>,
    pub description: Option<String>,
    pub imdb_rating: Option<String>,
    pub release_info: Option<String>,
    pub runtime: Option<String>,
    pub genres: Vec<String>,
    pub cast: Vec<String>,
    pub director: Vec<String>,
    pub writer: Vec<String>,
    pub videos: Vec<Video>,
}

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct Video {
    pub id: String,
    pub title: String,
    pub season: Option<i32>,
    pub episode: Option<i32>,
    pub thumbnail: Option<String>,
    pub overview: Option<String>,
    pub released: Option<String>,
}

/// A playable stream returned by an addon. Mirrors the Stremio stream object
/// but flattens the discriminated "source" into an explicit enum so callers
/// can match on it cleanly.
#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct Stream {
    /// Human label shown in the stream picker (e.g. "Torrentio • 1080p • 7 seeds").
    pub name: Option<String>,
    /// Detailed title (filename, release group, etc.).
    pub title: Option<String>,
    pub description: Option<String>,
    pub source: StreamSource,
    /// Preferred subtitle track language codes (for when the addon knows).
    pub subtitle_languages: Vec<String>,
    /// Arbitrary behavior hints (bingeGroup, countryWhitelist, proxyHeaders, …).
    /// Raw JSON preserved so the app can pass it to the player untouched.
    pub behavior_hints_json: Option<String>,
}

/// The concrete thing you have to play. Either an HTTP(S) URL, a magnet/info-hash
/// for the torrent engine, or a YouTube id (for addons that expose trailers).
#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Enum)]
pub enum StreamSource {
    Url {
        url: String,
    },
    Torrent {
        info_hash: String,
        file_idx: Option<i32>,
        /// `tracker:…` / `dht:…` entries to seed the swarm faster.
        sources: Vec<String>,
    },
    YouTube {
        video_id: String,
    },
    External {
        external_url: String,
    },
}

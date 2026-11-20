//! Wire-level deserialization of the Stremio addon protocol.
//!
//! These structs stick close to the raw JSON so we can parse real-world
//! addons tolerantly (addon authors cut corners). Conversions to the public
//! types live here too, so the FFI boundary never leaks `serde_json::Value`.

use serde::{Deserialize, Deserializer};
use serde_json::Value;

use super::types::{
    Catalog as PubCatalog, ExtraProp as PubExtra, Manifest as PubManifest,
    Meta as PubMeta, MetaPreview as PubPreview, Stream as PubStream,
    StreamSource, Video as PubVideo,
};

// -------- tolerant deserializers --------
//
// Real-world Stremio addons are loose with types: Cinemeta sometimes
// returns `imdbRating: 8.4` (number) instead of `"8.4"` (string), TMDB
// addons send `season: "1"` (string) instead of `1` (int), and some put
// `cast: "actor"` (string) where the spec says array. One strict field
// fails the whole decode, so we normalise aggressively here — anything
// representable as the target type is accepted.

/// Accept string | number | bool | null → Option<String>.
fn string_or_any<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(match Option::<Value>::deserialize(d)? {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s),
        Some(Value::Number(n)) => Some(n.to_string()),
        Some(Value::Bool(b))   => Some(b.to_string()),
        // Arrays/objects aren't meaningful for a scalar text field; drop.
        Some(_) => None,
    })
}

/// Accept int | number | string-containing-int | null → Option<i32>.
fn int_or_any<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i32>, D::Error> {
    Ok(match Option::<Value>::deserialize(d)? {
        None | Some(Value::Null) => None,
        Some(Value::Number(n)) => n.as_i64().map(|v| v as i32),
        Some(Value::String(s)) => s.trim().parse::<i32>().ok(),
        _ => None,
    })
}

/// Accept array of strings | single string | missing → Vec<String>.
fn vec_string_tolerant<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    Ok(match Option::<Value>::deserialize(d)? {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::String(s)) if !s.is_empty() => vec![s],
        Some(Value::Array(items)) => items
            .into_iter()
            .filter_map(|v| match v {
                Value::String(s) => Some(s),
                Value::Number(n) => Some(n.to_string()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    })
}

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub version: String,
    pub name: String,
    pub description: Option<String>,
    pub logo: Option<String>,
    pub background: Option<String>,
    #[serde(default)]
    pub types: Vec<String>,
    #[serde(default)]
    pub resources: Vec<ResourceEntry>,
    #[serde(default)]
    pub catalogs: Vec<Catalog>,
    #[serde(default, rename = "idPrefixes")]
    pub id_prefixes: Vec<String>,
}

/// `resources` can be either a bare string (`"catalog"`) or a full object
/// (`{ "name": "catalog", "types": [...], "idPrefixes": [...] }`).
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ResourceEntry {
    Short(String),
    Full { name: String },
}

impl ResourceEntry {
    pub fn name(&self) -> &str {
        match self {
            ResourceEntry::Short(s) => s,
            ResourceEntry::Full { name } => name,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Catalog {
    #[serde(rename = "type")]
    pub type_: String,
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub extra: Vec<ExtraProp>,
    /// Older manifests use `extraRequired` / `extraSupported` instead of `extra`.
    #[serde(default, rename = "extraRequired")]
    pub extra_required: Vec<String>,
    #[serde(default, rename = "extraSupported")]
    pub extra_supported: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExtraProp {
    pub name: String,
    #[serde(default, rename = "isRequired")]
    pub is_required: bool,
    #[serde(default)]
    pub options: Option<Vec<String>>,
    #[serde(default, rename = "optionsLimit")]
    pub options_limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct CatalogResponse {
    #[serde(default)]
    pub metas: Vec<MetaPreview>,
}

#[derive(Debug, Deserialize)]
pub struct MetaResponse {
    pub meta: Meta,
}

#[derive(Debug, Deserialize)]
pub struct StreamsResponse {
    #[serde(default)]
    pub streams: Vec<Stream>,
}

#[derive(Debug, Deserialize)]
pub struct MetaPreview {
    pub id: String,
    #[serde(rename = "type", default, deserialize_with = "string_or_any")]
    pub type_: Option<String>,
    #[serde(default, deserialize_with = "string_or_any")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "string_or_any")]
    pub poster: Option<String>,
    #[serde(default, rename = "posterShape", deserialize_with = "string_or_any")]
    pub poster_shape: Option<String>,
    #[serde(default, deserialize_with = "string_or_any")]
    pub background: Option<String>,
    #[serde(default, deserialize_with = "string_or_any")]
    pub logo: Option<String>,
    #[serde(default, deserialize_with = "string_or_any")]
    pub description: Option<String>,
    #[serde(default, rename = "imdbRating", deserialize_with = "string_or_any")]
    pub imdb_rating: Option<String>,
    #[serde(default, rename = "releaseInfo", deserialize_with = "string_or_any")]
    pub release_info: Option<String>,
    #[serde(default, deserialize_with = "vec_string_tolerant")]
    pub genres: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Meta {
    pub id: String,
    #[serde(rename = "type", default, deserialize_with = "string_or_any")]
    pub type_: Option<String>,
    #[serde(default, deserialize_with = "string_or_any")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "string_or_any")]
    pub poster: Option<String>,
    #[serde(default, rename = "posterShape", deserialize_with = "string_or_any")]
    pub poster_shape: Option<String>,
    #[serde(default, deserialize_with = "string_or_any")]
    pub background: Option<String>,
    #[serde(default, deserialize_with = "string_or_any")]
    pub logo: Option<String>,
    #[serde(default, deserialize_with = "string_or_any")]
    pub description: Option<String>,
    #[serde(default, rename = "imdbRating", deserialize_with = "string_or_any")]
    pub imdb_rating: Option<String>,
    #[serde(default, rename = "releaseInfo", deserialize_with = "string_or_any")]
    pub release_info: Option<String>,
    #[serde(default, deserialize_with = "string_or_any")]
    pub runtime: Option<String>,
    #[serde(default, deserialize_with = "vec_string_tolerant")]
    pub genres: Vec<String>,
    #[serde(default, deserialize_with = "vec_string_tolerant")]
    pub cast: Vec<String>,
    #[serde(default, deserialize_with = "vec_string_tolerant")]
    pub director: Vec<String>,
    #[serde(default, deserialize_with = "vec_string_tolerant")]
    pub writer: Vec<String>,
    #[serde(default)]
    pub videos: Vec<Video>,
}

#[derive(Debug, Deserialize)]
pub struct Video {
    pub id: String,
    #[serde(default, deserialize_with = "string_or_any")]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "int_or_any")]
    pub season: Option<i32>,
    #[serde(default, deserialize_with = "int_or_any")]
    pub episode: Option<i32>,
    #[serde(default, deserialize_with = "string_or_any")]
    pub thumbnail: Option<String>,
    #[serde(default, deserialize_with = "string_or_any")]
    pub overview: Option<String>,
    #[serde(default, deserialize_with = "string_or_any")]
    pub released: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Stream {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default, rename = "ytId")]
    pub yt_id: Option<String>,
    #[serde(default, rename = "infoHash")]
    pub info_hash: Option<String>,
    #[serde(default, rename = "fileIdx")]
    pub file_idx: Option<i32>,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default, rename = "externalUrl")]
    pub external_url: Option<String>,
    #[serde(default, rename = "subtitles")]
    pub subtitles: Option<Value>,
    #[serde(default, rename = "behaviorHints")]
    pub behavior_hints: Option<Value>,
}

// --------- conversions ---------

impl Manifest {
    pub fn into_public(self, transport_url: String) -> PubManifest {
        PubManifest {
            id: self.id,
            version: self.version,
            name: self.name,
            description: self.description,
            logo: self.logo,
            background: self.background,
            types: self.types,
            resources: self
                .resources
                .into_iter()
                .map(|r| r.name().to_string())
                .collect(),
            catalogs: self.catalogs.into_iter().map(Catalog::into_public).collect(),
            id_prefixes: self.id_prefixes,
            transport_url,
        }
    }
}

impl Catalog {
    pub fn into_public(self) -> PubCatalog {
        let name = self.name.clone().unwrap_or_else(|| self.id.clone());
        let mut extra: Vec<PubExtra> = self.extra.into_iter().map(ExtraProp::into_public).collect();
        // merge legacy extraRequired/extraSupported.
        for req in self.extra_required {
            if !extra.iter().any(|e| e.name == req) {
                extra.push(PubExtra { name: req, is_required: true, options: None, options_limit: None });
            } else {
                for e in extra.iter_mut() {
                    if e.name == req {
                        e.is_required = true;
                    }
                }
            }
        }
        for sup in self.extra_supported {
            if !extra.iter().any(|e| e.name == sup) {
                extra.push(PubExtra { name: sup, is_required: false, options: None, options_limit: None });
            }
        }
        PubCatalog {
            r#type: self.type_,
            id: self.id,
            name,
            extra,
        }
    }
}

impl ExtraProp {
    pub fn into_public(self) -> PubExtra {
        PubExtra {
            name: self.name,
            is_required: self.is_required,
            options: self.options,
            options_limit: self.options_limit,
        }
    }
}

impl MetaPreview {
    pub fn into_public(self, fallback_type: &str) -> PubPreview {
        let name = unescape(self.name.unwrap_or_else(|| self.id.clone()));
        PubPreview {
            id: self.id,
            r#type: self.type_.unwrap_or_else(|| fallback_type.to_string()),
            name,
            poster: self.poster,
            poster_shape: self.poster_shape,
            background: self.background,
            logo: self.logo,
            description: self.description.map(unescape),
            imdb_rating: self.imdb_rating,
            release_info: self.release_info,
            genres: self.genres,
        }
    }
}

/// Metadata sometimes arrives HTML-escaped (Cinemeta: "Judith O&apos;Dea").
/// Decodes the named entities that show up and numeric ones.
pub(crate) fn unescape(s: String) -> String {
    if !s.contains('&') {
        return s;
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s.as_str();
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let decoded = tail.find(';').filter(|&end| end <= 10).and_then(|end| {
            let entity = &tail[1..end];
            let ch = match entity {
                "apos" => Some('\''),
                "quot" => Some('"'),
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "nbsp" => Some('\u{a0}'),
                _ => entity
                    .strip_prefix("#x")
                    .or_else(|| entity.strip_prefix("#X"))
                    .map(|hex| u32::from_str_radix(hex, 16).ok())
                    .unwrap_or_else(|| entity.strip_prefix('#').and_then(|dec| dec.parse().ok()))
                    .and_then(char::from_u32),
            };
            ch.map(|c| (c, end + 1))
        });
        match decoded {
            Some((c, len)) => {
                out.push(c);
                rest = &tail[len..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn unescape_all(v: Vec<String>) -> Vec<String> {
    v.into_iter().map(unescape).collect()
}

impl Meta {
    pub fn into_public(self, fallback_type: &str) -> PubMeta {
        let name = unescape(self.name.unwrap_or_else(|| self.id.clone()));
        PubMeta {
            id: self.id,
            r#type: self.type_.unwrap_or_else(|| fallback_type.to_string()),
            name,
            poster: self.poster,
            poster_shape: self.poster_shape,
            background: self.background,
            logo: self.logo,
            description: self.description.map(unescape),
            imdb_rating: self.imdb_rating,
            release_info: self.release_info,
            runtime: self.runtime,
            genres: self.genres,
            cast: unescape_all(self.cast),
            director: unescape_all(self.director),
            writer: unescape_all(self.writer),
            videos: self.videos.into_iter().map(Video::into_public).collect(),
        }
    }
}

impl Video {
    pub fn into_public(self) -> PubVideo {
        let title = unescape(self.title.unwrap_or_else(|| {
            match (self.season, self.episode) {
                (Some(s), Some(e)) => format!("S{s:02}E{e:02}"),
                _ => self.id.clone(),
            }
        }));
        PubVideo {
            id: self.id,
            title,
            season: self.season,
            episode: self.episode,
            thumbnail: self.thumbnail,
            overview: self.overview.map(unescape),
            released: self.released,
        }
    }
}

impl Stream {
    pub fn into_public(self) -> PubStream {
        let source = if let Some(hash) = self.info_hash.clone() {
            StreamSource::Torrent {
                info_hash: hash,
                file_idx: self.file_idx,
                sources: self.sources.clone(),
            }
        } else if let Some(url) = self.url.clone() {
            StreamSource::Url { url }
        } else if let Some(yt) = self.yt_id.clone() {
            StreamSource::YouTube { video_id: yt }
        } else if let Some(ext) = self.external_url.clone() {
            StreamSource::External { external_url: ext }
        } else {
            // Shouldn't happen on a well-formed addon, but keep the stream so
            // the UI can still show it (greyed out) rather than silently drop it.
            StreamSource::External { external_url: String::new() }
        };

        let subtitle_languages = match self.subtitles {
            Some(Value::Array(ref arr)) => arr
                .iter()
                .filter_map(|v| v.get("lang").and_then(Value::as_str).map(str::to_string))
                .collect(),
            _ => Vec::new(),
        };

        PubStream {
            name: self.name,
            title: self.title,
            description: self.description,
            source,
            subtitle_languages,
            behavior_hints_json: self.behavior_hints.map(|v| v.to_string()),
        }
    }
}

#[cfg(test)]
mod unescape_tests {
    #[test]
    fn html_entities_are_decoded() {
        assert_eq!(super::unescape("Judith O&apos;Dea".into()), "Judith O'Dea");
        assert_eq!(super::unescape("Tom &amp; Jerry &#233;t&#xE9;".into()), "Tom & Jerry été");
        assert_eq!(super::unescape("AT&T & co &unknown; &".into()), "AT&T & co &unknown; &");
    }
}

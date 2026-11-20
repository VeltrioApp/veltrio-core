//! Stremio addon HTTP client.
//!
//! The Stremio addon protocol is pure JSON over HTTPS:
//!
//!   {transport}/manifest.json
//!   {transport}/catalog/{type}/{id}[/{extra}].json
//!   {transport}/meta/{type}/{id}.json
//!   {transport}/stream/{type}/{id}.json
//!   {transport}/subtitles/{type}/{id}.json   (not wired here yet)
//!
//! The "transport URL" is the manifest URL itself; everything else hangs off
//! the same base. We accept either `https://host/manifest.json` or just
//! `https://host/` as input and normalize.

use std::sync::Arc;
use std::time::Duration;

use reqwest::Client as HttpClient;
use url::Url;

use crate::error::{VeltrioError, VeltrioResult};
use crate::runtime;

use super::types::{Manifest, Meta, MetaPreview, Stream};
use super::wire;

#[derive(uniffi::Object)]
pub struct StremioClient {
    http: HttpClient,
}

#[uniffi::export(async_runtime = "tokio")]
impl StremioClient {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        // Touch the runtime once up front so the first async call doesn't pay
        // the init cost.
        let _ = runtime();
        let http = crate::http_client_builder()
            .user_agent(concat!("Veltrio/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(20))
            .connect_timeout(Duration::from_secs(8))
            .build()
            .expect("failed to build reqwest client");
        Arc::new(Self { http })
    }

    /// Fetch and parse the manifest at `transport_url`. The returned manifest
    /// carries the normalized transport URL so the caller can store it and
    /// reuse it for subsequent calls.
    pub async fn fetch_manifest(&self, transport_url: String) -> VeltrioResult<Manifest> {
        let normalized = normalize_transport(&transport_url)?;
        let manifest_url = normalized.join("manifest.json")?;
        tracing::debug!(url = %manifest_url, "fetching manifest");
        let resp = self.http.get(manifest_url).send().await?.error_for_status()?;
        let wire: wire::Manifest = resp.json().await?;
        Ok(wire.into_public(normalized.to_string()))
    }

    /// Fetch a catalog. `extras` is a list of `name=value` pairs
    /// (`skip=50`, `search=matrix`, `genre=Comedy` …).
    pub async fn fetch_catalog(
        &self,
        transport_url: String,
        r#type: String,
        id: String,
        extras: Vec<String>,
    ) -> VeltrioResult<Vec<MetaPreview>> {
        let normalized = normalize_transport(&transport_url)?;
        let extra_segment = if extras.is_empty() {
            String::new()
        } else {
            format!("/{}", extras.join("&"))
        };
        let path = format!("catalog/{}/{}{extra_segment}.json", encode(&r#type), encode(&id));
        let url = normalized.join(&path)?;
        tracing::debug!(url = %url, "fetching catalog");
        let resp = self.http.get(url).send().await?.error_for_status()?;
        let wire: wire::CatalogResponse = resp.json().await?;
        Ok(wire
            .metas
            .into_iter()
            .map(|m| m.into_public(&r#type))
            .collect())
    }

    pub async fn fetch_meta(
        &self,
        transport_url: String,
        r#type: String,
        id: String,
    ) -> VeltrioResult<Meta> {
        let normalized = normalize_transport(&transport_url)?;
        let path = format!("meta/{}/{}.json", encode(&r#type), encode(&id));
        let url = normalized.join(&path)?;
        tracing::debug!(url = %url, "fetching meta");
        let resp = self.http.get(url).send().await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(VeltrioError::NotFound { message: id });
        }
        let resp = resp.error_for_status()?;
        let wire: wire::MetaResponse = resp.json().await?;
        Ok(wire.meta.into_public(&r#type))
    }

    pub async fn fetch_streams(
        &self,
        transport_url: String,
        r#type: String,
        id: String,
    ) -> VeltrioResult<Vec<Stream>> {
        let normalized = normalize_transport(&transport_url)?;
        let path = format!("stream/{}/{}.json", encode(&r#type), encode(&id));
        let url = normalized.join(&path)?;
        tracing::debug!(url = %url, "fetching streams");
        let resp = self.http.get(url).send().await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(VeltrioError::NotFound { message: id });
        }
        let resp = resp.error_for_status()?;
        let wire: wire::StreamsResponse = resp.json().await?;
        Ok(wire.streams.into_iter().map(wire::Stream::into_public).collect())
    }
}

fn normalize_transport(input: &str) -> VeltrioResult<Url> {
    let trimmed = input.trim();
    let url = Url::parse(trimmed)?;
    // We always want a base ending with `/` so `Url::join` concatenates
    // instead of replacing the last path segment.
    let mut normalized = url.clone();
    let path = url.path();
    let base_path = if let Some(stripped) = path.strip_suffix("/manifest.json") {
        if stripped.is_empty() { "/".to_string() } else { format!("{stripped}/") }
    } else if path.ends_with('/') {
        path.to_string()
    } else {
        format!("{path}/")
    };
    normalized.set_path(&base_path);
    Ok(normalized)
}

fn encode(segment: &str) -> String {
    // Stremio IDs can contain `:` (e.g. `tt0133093:1:1`) which must stay
    // literal; only encode characters that break the URL.
    url::form_urlencoded::byte_serialize(segment.as_bytes())
        .collect::<String>()
        .replace('+', "%20")
}

#[cfg(test)]
mod tests {
    use super::normalize_transport;

    #[test]
    fn normalize_strips_manifest_json() {
        let got = normalize_transport("https://v3-cinemeta.strem.io/manifest.json").unwrap();
        assert_eq!(got.as_str(), "https://v3-cinemeta.strem.io/");
    }

    #[test]
    fn normalize_adds_trailing_slash() {
        let got = normalize_transport("https://example.com/path").unwrap();
        assert_eq!(got.as_str(), "https://example.com/path/");
    }
}

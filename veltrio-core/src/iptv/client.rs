//! Public IPTV client: fetch a playlist URL (or read a `file://` path) and
//! return a parsed, deduplicated playlist ready for the UI.

use std::sync::Arc;
use std::time::Duration;

use reqwest::Client as HttpClient;

use crate::error::{VeltrioError, VeltrioResult};
use crate::iptv::m3u;
use crate::iptv::types::IptvPlaylist;

#[derive(uniffi::Object)]
pub struct IptvClient {
    http: HttpClient,
}

#[uniffi::export(async_runtime = "tokio")]
impl IptvClient {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        let http = crate::http_client_builder()
            .user_agent(concat!("Veltrio-IPTV/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(10))
            // IPTV playlists can be megabytes big; allow a generous budget.
            .build()
            .expect("failed to build reqwest client");
        Arc::new(Self { http })
    }

    /// Fetch a playlist from an `http(s)://` URL or a local `file://` path.
    pub async fn load_playlist(&self, source_url: String) -> VeltrioResult<IptvPlaylist> {
        let body = if let Some(path) = source_url.strip_prefix("file://") {
            tokio::fs::read_to_string(path)
                .await
                .map_err(|e| VeltrioError::Network { message: e.to_string() })?
        } else {
            let resp = self
                .http
                .get(&source_url)
                .send()
                .await?
                .error_for_status()?;
            resp.text().await?
        };

        let mut channels = m3u::parse(&body);
        if channels.is_empty() {
            return Err(VeltrioError::Parse {
                message: "no channels parsed — not an M3U playlist?".to_string(),
            });
        }
        // Deduplicate by id (common in bundled playlists).
        channels.sort_by(|a, b| a.id.cmp(&b.id));
        channels.dedup_by(|a, b| a.id == b.id);

        Ok(IptvPlaylist { source_url, channels })
    }

    /// Parse an inline playlist string. Useful for testing and for pasting
    /// a short list of channels straight into the UI.
    pub fn parse_inline(&self, content: String) -> VeltrioResult<IptvPlaylist> {
        let channels = m3u::parse(&content);
        if channels.is_empty() {
            return Err(VeltrioError::Parse {
                message: "no channels parsed from inline content".to_string(),
            });
        }
        Ok(IptvPlaylist {
            source_url: "inline".to_string(),
            channels,
        })
    }
}

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use librqbit::api::Api;
use librqbit::http_api::{HttpApi, HttpApiOptions};
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, Session, SessionOptions,
};
use librqbit_dualstack_sockets::{BindOpts, TcpListener as DsTcpListener};
use parking_lot::Mutex;

use crate::error::{VeltrioError, VeltrioResult};
use crate::runtime;

/// Common open trackers injected when the addon doesn't ship any. These are
/// the same list most Stremio addons include anyway; without them a bare
/// `magnet:?xt=urn:btih:…` is dead on arrival (DHT still works but is slow).
const FALLBACK_TRACKERS: &[&str] = &[
    "udp://tracker.opentrackr.org:1337/announce",
    "udp://open.tracker.cl:1337/announce",
    "udp://open.stealth.si:80/announce",
    "udp://tracker.torrent.eu.org:451/announce",
    "udp://exodus.desync.com:6969/announce",
    "udp://tracker.moeking.me:6969/announce",
    "udp://tracker.auctor.tv:6969/announce",
    "udp://explodie.org:6969/announce",
];

#[derive(uniffi::Object)]
pub struct TorrentEngine {
    inner: Mutex<Option<Arc<Inner>>>,
}

struct Inner {
    #[allow(dead_code)]
    session: Arc<Session>,
    base_url: String,
}

#[uniffi::export(async_runtime = "tokio")]
impl TorrentEngine {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        let _ = runtime();
        Arc::new(Self { inner: Mutex::new(None) })
    }

    /// Returns the base URL `http://127.0.0.1:{port}`. Spins the engine up
    /// on first call; subsequent calls are cheap.
    pub async fn ensure_started(&self) -> VeltrioResult<String> {
        if let Some(inner) = self.inner.lock().as_ref() {
            return Ok(inner.base_url.clone());
        }
        let inner = Self::start().await?;
        let base_url = inner.base_url.clone();
        *self.inner.lock() = Some(inner);
        Ok(base_url)
    }

    /// Add a torrent by infohash + optional sources (announce-list), and
    /// return a URL that streams `file_idx` (the largest video if `None`).
    ///
    /// The returned URL is backed by librqbit's HTTP API, which waits for
    /// metadata internally and then serves the file with Range support.
    pub async fn start_stream(
        &self,
        info_hash: String,
        sources: Vec<String>,
        file_idx: Option<i32>,
    ) -> VeltrioResult<String> {
        let base_url = self.ensure_started().await?;
        let inner_arc = self
            .inner
            .lock()
            .as_ref()
            .cloned()
            .ok_or_else(|| VeltrioError::Internal { message: "engine not started".into() })?;

        let hash = info_hash.trim().to_ascii_lowercase();
        if hash.len() != 40 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(VeltrioError::InvalidUrl {
                message: format!("expected a 40-char hex infohash, got {hash:?}"),
            });
        }

        let magnet = build_magnet(&hash, &sources);
        let only_files = file_idx.map(|i| vec![i as usize]);

        let add = AddTorrent::from_url(magnet);
        let opts = AddTorrentOptions {
            overwrite: true,
            only_files,
            // Without an index the file is picked once the metadata is in;
            // nothing downloads until then.
            paused: file_idx.is_none(),
            ..Default::default()
        };
        let resp = inner_arc
            .session
            .add_torrent(add, Some(opts))
            .await
            .map_err(|e| VeltrioError::Network { message: format!("add_torrent: {e}") })?;
        let (added, handle) = match resp {
            AddTorrentResponse::Added(_, h) => (true, h),
            AddTorrentResponse::AlreadyManaged(_, h) => (false, h),
            AddTorrentResponse::ListOnly(_) => {
                return Err(VeltrioError::Internal {
                    message: "unexpected ListOnly response".into(),
                })
            }
        };

        let idx = match file_idx {
            Some(i) => i as usize,
            None => {
                let files: Vec<(String, u64)> = handle
                    .with_metadata(|m| {
                        m.file_infos
                            .iter()
                            .map(|f| (f.relative_filename.to_string_lossy().into_owned(), f.len))
                            .collect()
                    })
                    .map_err(|e| VeltrioError::Internal { message: format!("metadata: {e}") })?;
                let idx = pick_main_file(&files);
                if added {
                    // Files already on disk are checked first; only a
                    // paused torrent can change its file selection.
                    handle
                        .wait_until_initialized()
                        .await
                        .map_err(|e| VeltrioError::Internal { message: format!("init: {e}") })?;
                    let session = &inner_arc.session;
                    session
                        .update_only_files(&handle, &HashSet::from([idx]))
                        .await
                        .map_err(|e| VeltrioError::Internal { message: format!("only_files: {e}") })?;
                    session
                        .unpause(&handle)
                        .await
                        .map_err(|e| VeltrioError::Internal { message: format!("unpause: {e}") })?;
                }
                idx
            }
        };
        Ok(format!("{base_url}/torrents/{hash}/stream/{idx}"))
    }

    /// Remove a torrent and stop seeding it. Idempotent; silently succeeds
    /// if we don't know about this hash.
    pub async fn stop_stream(&self, info_hash: String, delete_files: bool) -> VeltrioResult<()> {
        let inner_arc = match self.inner.lock().as_ref().cloned() {
            Some(a) => a,
            None => return Ok(()),
        };
        let hash = info_hash.trim().to_ascii_lowercase();
        let id = match librqbit::api::TorrentIdOrHash::try_from(hash.as_str()) {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };
        let _ = inner_arc.session.delete(id, delete_files).await;
        Ok(())
    }
}

impl TorrentEngine {
    async fn start() -> VeltrioResult<Arc<Inner>> {
        let output_dir = default_output_dir();
        tokio::fs::create_dir_all(&output_dir)
            .await
            .map_err(|e| VeltrioError::Internal { message: format!("mkdir: {e}") })?;
        let session = Session::new_with_opts(output_dir, SessionOptions::default())
            .await
            .map_err(|e| VeltrioError::Internal { message: format!("session: {e}") })?;

        let addr: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
        let listener = DsTcpListener::bind_tcp(
            addr,
            BindOpts { request_dualstack: false, reuseport: false, device: None },
        )
        .map_err(|e| VeltrioError::Internal { message: format!("bind: {e}") })?;
        let port = listener.bind_addr().port();
        let base_url = format!("http://127.0.0.1:{port}");

        let api = Api::new(session.clone(), None, None);
        let http_api = HttpApi::new(api, Some(HttpApiOptions::default()));

        // Run the API in a background task tied to the session's lifetime.
        tokio::spawn(async move {
            if let Err(e) = http_api.make_http_api_and_run(listener, None).await {
                tracing::error!(error = %e, "torrent http api stopped");
            }
        });

        tracing::info!(%base_url, "torrent engine listening");
        Ok(Arc::new(Inner { session, base_url }))
    }
}

fn default_output_dir() -> PathBuf {
    // ~/Library/Caches/Veltrio/torrents on macOS, $XDG_CACHE_HOME elsewhere.
    if let Some(home) = std::env::var_os("HOME") {
        let mut p = PathBuf::from(home);
        #[cfg(target_os = "macos")]
        {
            p.push("Library");
            p.push("Caches");
            p.push("Veltrio");
            p.push("torrents");
        }
        #[cfg(not(target_os = "macos"))]
        {
            p.push(".cache");
            p.push("veltrio");
            p.push("torrents");
        }
        return p;
    }
    std::env::temp_dir().join("veltrio-torrents")
}

fn build_magnet(info_hash: &str, sources: &[String]) -> String {
    let mut out = format!("magnet:?xt=urn:btih:{info_hash}");
    let mut added_trackers = 0usize;
    for src in sources {
        let s = src.trim();
        if s.is_empty() {
            continue;
        }
        if let Some(rest) = s.strip_prefix("tracker:") {
            out.push_str("&tr=");
            out.push_str(&url_encode(rest));
            added_trackers += 1;
        } else if let Some(rest) = s.strip_prefix("dht:") {
            // DHT-only entries aren't magnet attrs, but we can keep them as
            // trackers too — librqbit handles both.
            out.push_str("&x.pe=");
            out.push_str(&url_encode(rest));
        } else if s.starts_with("udp://") || s.starts_with("http://") || s.starts_with("https://") {
            out.push_str("&tr=");
            out.push_str(&url_encode(s));
            added_trackers += 1;
        }
    }
    if added_trackers == 0 {
        for tr in FALLBACK_TRACKERS {
            out.push_str("&tr=");
            out.push_str(&url_encode(tr));
        }
    }
    out
}

fn url_encode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magnet_has_all_sources() {
        let m = build_magnet(
            "c9e15763f722f23e98a29decdfae341b98d53056",
            &vec![
                "udp://tracker.opentrackr.org:1337/announce".to_string(),
                "tracker:udp://explodie.org:6969/announce".to_string(),
                "dht:router.bittorrent.com:6881".to_string(),
            ],
        );
        assert!(m.contains("xt=urn:btih:c9e15763f722f23e98a29decdfae341b98d53056"));
        assert!(m.contains("tr=udp%3A%2F%2Ftracker.opentrackr.org%3A1337%2Fannounce"));
        assert!(m.contains("tr=udp%3A%2F%2Fexplodie.org%3A6969%2Fannounce"));
        assert!(m.contains("x.pe=router.bittorrent.com%3A6881"));
    }

    #[test]
    fn magnet_falls_back_when_no_trackers() {
        let m = build_magnet("c9e15763f722f23e98a29decdfae341b98d53056", &[]);
        // At least one of the baked-in fallback trackers is present.
        assert!(m.contains("tracker.opentrackr.org"));
    }
}

const VIDEO_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "m4v", "avi", "mov", "webm", "ts", "m2ts", "wmv", "flv", "mpg", "mpeg", "ogv",
];

/// The file to stream when the addon gave no index: the largest video, or
/// the largest file if none looks like one (subtitles, posters and samples
/// are small).
fn pick_main_file(files: &[(String, u64)]) -> usize {
    let is_video = |name: &str| {
        name.rsplit_once('.')
            .is_some_and(|(_, ext)| VIDEO_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
    };
    let largest = |videos_only: bool| {
        files
            .iter()
            .enumerate()
            .filter(|(_, (name, _))| !videos_only || is_video(name))
            .max_by_key(|(_, (_, len))| *len)
            .map(|(i, _)| i)
    };
    largest(true).or_else(|| largest(false)).unwrap_or(0)
}

#[cfg(test)]
mod pick_tests {
    use super::pick_main_file;

    #[test]
    fn picks_the_largest_video() {
        let files = vec![
            ("Sintel/Sintel.de.srt".to_string(), 1_652),
            ("Sintel/poster.jpg".to_string(), 46_115),
            ("Sintel/Sample.mkv".to_string(), 9_000_000),
            ("Sintel/Sintel.MP4".to_string(), 129_241_752),
        ];
        assert_eq!(pick_main_file(&files), 3);
    }

    #[test]
    fn falls_back_to_the_largest_file() {
        let files = vec![("a.srt".to_string(), 10), ("b.bin".to_string(), 99)];
        assert_eq!(pick_main_file(&files), 1);
        assert_eq!(pick_main_file(&[]), 0);
    }
}

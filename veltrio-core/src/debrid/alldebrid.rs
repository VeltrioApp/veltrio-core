//! AllDebrid v4.1 client.
//!
//! Implements the subset Veltrio needs: PIN-based auth, availability check,
//! stream resolution. Ported & adapted from the Frenchio Python service,
//! with four upgrades:
//!
//! 1. **PIN auth** — the user never pastes an API key; they click a button,
//!    enter a 4-letter pin on alldebrid.com, and we poll for the key.
//! 2. **Credential-free API** — the key is injected by the Swift side at
//!    call time (stored in Keychain), never persisted in Rust.
//! 3. **Polite batching + cleanup** — magnets we upload to test availability
//!    are deleted in a detached task, 5 at a time with a short gap, exactly
//!    like Frenchio.
//! 4. **Typed errors** — `VeltrioError::Unsupported` for "AllDebrid isn't
//!    premium on this account", distinct from network failures.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use reqwest::Client as HttpClient;
use serde::Deserialize;
use serde_json::Value;

use crate::debrid::file_selection::{pick_best, Candidate};
use crate::error::{VeltrioError, VeltrioResult};
use crate::runtime;

const BASE_URL: &str = "https://api.alldebrid.com/v4.1";
const AGENT: &str = "veltrio";
const AVAILABILITY_BATCH: usize = 20;
const DELETE_BATCH: usize = 5;

#[derive(uniffi::Object)]
pub struct AllDebridClient {
    http: HttpClient,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AdPinStart {
    /// Human-readable PIN the user enters at `user_url`.
    pub pin: String,
    /// Opaque token for polling `poll_pin`.
    pub check_token: String,
    /// Where to send the user to validate (open in their browser).
    pub user_url: String,
    /// Seconds until the PIN expires (usually 600).
    pub expires_in_seconds: u32,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AdUserInfo {
    pub username: String,
    pub email: String,
    pub is_premium: bool,
    /// Unix seconds; 0 if not premium.
    pub premium_until: u64,
}

#[uniffi::export(async_runtime = "tokio")]
impl AllDebridClient {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        let _ = runtime();
        let http = HttpClient::builder()
            .user_agent(concat!("Veltrio-AllDebrid/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(8))
            .build()
            .expect("failed to build reqwest client");
        Arc::new(Self { http })
    }

    // ---------- PIN auth ----------

    /// Start the PIN flow. Send the user to `user_url`, show them `pin`,
    /// and start polling `poll_pin` every ~2 seconds until it returns an
    /// API key (or the PIN expires).
    pub async fn start_pin_auth(&self) -> VeltrioResult<AdPinStart> {
        let url = format!("{BASE_URL}/pin/get?agent={AGENT}");
        let resp = self.http.get(&url).send().await?;
        let envelope: ApiEnvelope<PinGetData> = parse_envelope(resp).await?;
        let data = envelope.into_data()?;
        Ok(AdPinStart {
            pin: data.pin,
            check_token: data.check,
            user_url: data.user_url,
            expires_in_seconds: data.expires_in,
        })
    }

    /// Returns `Some(apikey)` once the user has activated the PIN, `None`
    /// while still waiting. Error means expired or network trouble.
    pub async fn poll_pin(
        &self,
        pin: String,
        check_token: String,
    ) -> VeltrioResult<Option<String>> {
        let url = format!(
            "{BASE_URL}/pin/check?agent={AGENT}&check={check}&pin={pin}",
            check = urlencoded(&check_token),
            pin = urlencoded(&pin)
        );
        let resp = self.http.get(&url).send().await?;
        let envelope: ApiEnvelope<PinCheckData> = parse_envelope(resp).await?;
        let data = envelope.into_data()?;
        if data.activated {
            data.apikey
                .map(Some)
                .ok_or_else(|| VeltrioError::Internal {
                    message: "activated but no apikey".into(),
                })
        } else if data.expired {
            Err(VeltrioError::Unsupported {
                message: "PIN expired — start a new one".into(),
            })
        } else {
            Ok(None)
        }
    }

    // ---------- account ----------

    pub async fn user_info(&self, api_key: String) -> VeltrioResult<AdUserInfo> {
        let url = format!("{BASE_URL}/user?agent={AGENT}&apikey={}", urlencoded(&api_key));
        let resp = self.http.get(&url).send().await?;
        let envelope: ApiEnvelope<UserData> = parse_envelope(resp).await?;
        let user = envelope.into_data()?.user;
        Ok(AdUserInfo {
            username: user.username,
            email: user.email,
            is_premium: user.is_premium,
            premium_until: user.premium_until.unwrap_or(0),
        })
    }

    // ---------- availability ----------

    /// Check which of the given infohashes are cached-and-ready. Returns
    /// a map `hash -> cached`. Uploaded probe magnets are deleted in the
    /// background to keep the user's account clean.
    pub async fn check_availability(
        &self,
        api_key: String,
        hashes: Vec<String>,
    ) -> VeltrioResult<HashMap<String, bool>> {
        let mut result: HashMap<String, bool> = HashMap::new();
        let mut uploaded_ids: Vec<u64> = Vec::new();

        let cleaned: Vec<String> = hashes.iter().map(|h| clean_hash(h)).collect();
        if cleaned.is_empty() {
            return Ok(result);
        }

        let url = format!("{BASE_URL}/magnet/upload");
        for chunk in cleaned.chunks(AVAILABILITY_BATCH) {
            let mut form: Vec<(&str, String)> = vec![
                ("agent", AGENT.to_string()),
                ("apikey", api_key.clone()),
            ];
            for h in chunk {
                form.push(("magnets[]", h.clone()));
            }
            let resp = self.http.post(&url).form(&form).send().await?;
            let envelope: ApiEnvelope<MagnetUploadData> = parse_envelope(resp).await?;
            let data = match envelope.into_data() {
                Ok(d) => d,
                Err(VeltrioError::Unsupported { message }) => {
                    // AllDebrid returned an "error" status — propagate as a
                    // real error so the UI doesn't silently show "nothing
                    // cached".
                    return Err(VeltrioError::Unsupported { message });
                }
                Err(e) => return Err(e),
            };
            for m in data.magnets {
                if let Some(id) = m.id {
                    uploaded_ids.push(id);
                }
                // AllDebrid reports readiness two ways depending on firmware;
                // the Frenchio heuristic treats `statusCode == 4` as cached.
                let ready = m.ready.unwrap_or(false) || m.status_code == Some(4);
                let key = m
                    .hash
                    .clone()
                    .or_else(|| m.magnet.clone())
                    .map(|k| clean_hash(&k));
                if let Some(k) = key {
                    result.insert(k, ready);
                }
            }
        }

        // Fire-and-forget cleanup. We clone the client because the task
        // outlives this function.
        let http_clone = self.http.clone();
        let api_key_clone = api_key.clone();
        runtime().spawn(async move {
            Self::delete_magnets(&http_clone, &api_key_clone, uploaded_ids).await;
        });

        Ok(result)
    }

    // ---------- resolve stream ----------

    /// Upload `info_hash` to AllDebrid, pick the right file (episode match
    /// or largest video), unlock the link, and return the final direct URL.
    /// Returns `None` if the content is not cached and would need to be
    /// downloaded by AD first — the caller falls back to the local engine.
    pub async fn resolve_stream(
        &self,
        api_key: String,
        info_hash: String,
        season: Option<i32>,
        episode: Option<i32>,
    ) -> VeltrioResult<Option<String>> {
        let hash = clean_hash(&info_hash);

        // 1. Upload
        let upload_url = format!("{BASE_URL}/magnet/upload");
        let resp = self
            .http
            .post(&upload_url)
            .form(&[
                ("agent", AGENT),
                ("apikey", api_key.as_str()),
                ("magnets[]", hash.as_str()),
            ])
            .send()
            .await?;
        let upload: ApiEnvelope<MagnetUploadData> = parse_envelope(resp).await?;
        let mut magnets = upload.into_data()?.magnets;
        let magnet = magnets.pop().ok_or_else(|| VeltrioError::Parse {
            message: "AllDebrid returned no magnet entry".into(),
        })?;
        let magnet_id = magnet.id.ok_or_else(|| VeltrioError::Parse {
            message: "AllDebrid magnet has no id".into(),
        })?;
        let ready = magnet.ready.unwrap_or(false) || magnet.status_code == Some(4);

        // 2a. Instant: /magnet/upload sometimes returns `links` inline.
        if ready && !magnet.links.is_empty() {
            let picked = pick_best(
                &magnet
                    .links
                    .iter()
                    .map(|l| Candidate {
                        link: l.link.clone(),
                        filename: l.filename.clone().unwrap_or_default(),
                        size: l.size.unwrap_or(0),
                    })
                    .collect::<Vec<_>>(),
                season,
                episode,
            );
            if let Some(link) = picked {
                return self.unlock_link(&api_key, &link).await.map(Some);
            }
        }

        // 2b. Not cached? Caller handles fallback.
        if !ready {
            return Ok(None);
        }

        // 3. Cached, no inline links → fetch structure.
        let files_url = format!("{BASE_URL}/magnet/files");
        let resp = self
            .http
            .post(&files_url)
            .form(&[
                ("agent", AGENT),
                ("apikey", api_key.as_str()),
                ("id[]", magnet_id.to_string().as_str()),
            ])
            .send()
            .await?;
        let files: ApiEnvelope<MagnetFilesData> = parse_envelope(resp).await?;
        let files_data = files.into_data()?;
        let magnet_entry = files_data
            .magnets
            .into_iter()
            .find(|m| m.id == magnet_id)
            .ok_or_else(|| VeltrioError::Parse {
                message: "magnet not found in /magnet/files response".into(),
            })?;

        let flat = flatten_files(&magnet_entry.files, "");
        let candidates: Vec<Candidate<String>> = flat
            .iter()
            .map(|f| Candidate { link: f.link.clone(), filename: f.filename.clone(), size: f.size })
            .collect();
        let picked = pick_best(&candidates, season, episode).ok_or_else(|| VeltrioError::NotFound {
            message: "no playable file found in torrent".into(),
        })?;
        let unlocked = self.unlock_link(&api_key, &picked).await?;
        Ok(Some(unlocked))
    }

    async fn unlock_link(&self, api_key: &str, link: &str) -> VeltrioResult<String> {
        let url = format!("{BASE_URL}/link/unlock");
        let resp = self
            .http
            .post(&url)
            .form(&[
                ("agent", AGENT),
                ("apikey", api_key),
                ("link", link),
            ])
            .send()
            .await?;
        let envelope: ApiEnvelope<UnlockData> = parse_envelope(resp).await?;
        Ok(envelope.into_data()?.link)
    }
}

// Plain impl — associated functions can live here (uniffi::export doesn't
// cover this block).
impl AllDebridClient {
    async fn delete_magnets(http: &HttpClient, api_key: &str, ids: Vec<u64>) {
        let url = format!("{BASE_URL}/magnet/delete");
        for chunk in ids.chunks(DELETE_BATCH) {
            let futures = chunk.iter().map(|id| {
                http.post(&url).form(&[
                    ("agent", AGENT),
                    ("apikey", api_key),
                    ("id", &id.to_string()),
                ]).send()
            });
            let _ = futures::future::join_all(futures).await;
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
}

// ---------------- helpers ----------------

fn clean_hash(h: &str) -> String {
    let trimmed = h.trim().to_lowercase();
    // Frenchio also handled 80-char hex-encoded hashes (hex of ASCII hash).
    if trimmed.len() == 80 {
        if let Ok(bytes) = hex_decode(&trimmed) {
            if let Ok(decoded) = std::str::from_utf8(&bytes) {
                if decoded.len() == 40 && decoded.chars().all(|c| c.is_ascii_hexdigit()) {
                    return decoded.to_string();
                }
            }
        }
    }
    trimmed
}

fn hex_decode(s: &str) -> Result<Vec<u8>, ()> {
    if s.len() % 2 != 0 {
        return Err(());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| ()))
        .collect()
}

fn urlencoded(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

/// Flatten AllDebrid's recursive `files` structure into
/// `{link, filename-with-path, size}`.
fn flatten_files(nodes: &[FileNode], path: &str) -> Vec<FlatFile> {
    let mut out = Vec::new();
    for node in nodes {
        let name = node.name.clone().unwrap_or_default();
        let my_path = if path.is_empty() { name.clone() } else { format!("{path}/{name}") };
        if let Some(link) = &node.link {
            out.push(FlatFile {
                link: link.clone(),
                filename: my_path,
                size: node.size.unwrap_or(0),
            });
        } else if let Some(children) = &node.children {
            out.extend(flatten_files(children, &my_path));
        }
    }
    out
}

struct FlatFile {
    link: String,
    filename: String,
    size: u64,
}

// ---------------- API envelopes ----------------

async fn parse_envelope<T: for<'a> Deserialize<'a>>(
    resp: reqwest::Response,
) -> VeltrioResult<ApiEnvelope<T>> {
    let status = resp.status();
    if !status.is_success() {
        return Err(VeltrioError::Network {
            message: format!("AllDebrid HTTP {status}"),
        });
    }
    // Read the body as Value first so an error payload can carry its
    // AllDebrid-specific `error.message` into our typed error.
    let v: Value = resp.json().await?;
    match v.get("status").and_then(Value::as_str) {
        Some("success") => Ok(serde_json::from_value(v)?),
        Some("error") => {
            let msg = v
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("unknown AllDebrid error")
                .to_string();
            Err(VeltrioError::Unsupported { message: msg })
        }
        _ => Err(VeltrioError::Parse {
            message: "AllDebrid response missing status".into(),
        }),
    }
}

#[derive(Debug, Deserialize)]
struct ApiEnvelope<T> {
    #[allow(dead_code)]
    status: String,
    data: Option<T>,
}

impl<T> ApiEnvelope<T> {
    fn into_data(self) -> VeltrioResult<T> {
        self.data.ok_or_else(|| VeltrioError::Parse {
            message: "missing `data` in AllDebrid response".into(),
        })
    }
}

#[derive(Debug, Deserialize)]
struct PinGetData {
    pin: String,
    check: String,
    #[serde(rename = "user_url")]
    user_url: String,
    #[serde(rename = "expires_in")]
    expires_in: u32,
}

#[derive(Debug, Deserialize)]
struct PinCheckData {
    activated: bool,
    #[serde(default)]
    expired: bool,
    apikey: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UserData {
    user: UserFields,
}

#[derive(Debug, Deserialize)]
struct UserFields {
    #[serde(default)]
    username: String,
    #[serde(default)]
    email: String,
    #[serde(default, rename = "isPremium")]
    is_premium: bool,
    #[serde(default, rename = "premiumUntil")]
    premium_until: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct MagnetUploadData {
    magnets: Vec<MagnetEntry>,
}

#[derive(Debug, Deserialize)]
struct MagnetEntry {
    #[serde(default)]
    id: Option<u64>,
    #[serde(default)]
    hash: Option<String>,
    #[serde(default)]
    magnet: Option<String>,
    #[serde(default)]
    ready: Option<bool>,
    #[serde(default, rename = "statusCode")]
    status_code: Option<i32>,
    #[serde(default)]
    links: Vec<MagnetLink>,
}

#[derive(Debug, Deserialize)]
struct MagnetLink {
    link: String,
    #[serde(default)]
    filename: Option<String>,
    #[serde(default)]
    size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct MagnetFilesData {
    magnets: Vec<MagnetFilesEntry>,
}

#[derive(Debug, Deserialize)]
struct MagnetFilesEntry {
    id: u64,
    #[serde(default)]
    files: Vec<FileNode>,
}

/// AllDebrid's file tree: `n` = name, `l` = link (leaf), `e` = children, `s` = size.
#[derive(Debug, Deserialize)]
struct FileNode {
    #[serde(rename = "n", default)]
    name: Option<String>,
    #[serde(rename = "l", default)]
    link: Option<String>,
    #[serde(rename = "s", default)]
    size: Option<u64>,
    #[serde(rename = "e", default)]
    children: Option<Vec<FileNode>>,
}

#[derive(Debug, Deserialize)]
struct UnlockData {
    link: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_hex_encoded_hash() {
        // 80-char hex is the hex of ASCII hash — shouldn't happen on well-
        // behaved addons but we accept it defensively.
        let ascii_hash = "c9e15763f722f23e98a29decdfae341b98d53056";
        let doubly = ascii_hash
            .bytes()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert_eq!(clean_hash(&doubly), ascii_hash);
    }

    #[test]
    fn preserves_normal_hash() {
        let h = "C9E15763F722F23E98A29DECDFAE341B98D53056";
        assert_eq!(clean_hash(h), h.to_lowercase());
    }

    #[test]
    fn flattens_two_level_tree() {
        let tree = vec![FileNode {
            name: Some("Show".into()),
            link: None,
            size: None,
            children: Some(vec![
                FileNode {
                    name: Some("E01.mkv".into()),
                    link: Some("https://link/1".into()),
                    size: Some(1_000),
                    children: None,
                },
                FileNode {
                    name: Some("E02.mkv".into()),
                    link: Some("https://link/2".into()),
                    size: Some(2_000),
                    children: None,
                },
            ]),
        }];
        let flat = flatten_files(&tree, "");
        assert_eq!(flat.len(), 2);
        assert_eq!(flat[0].filename, "Show/E01.mkv");
        assert_eq!(flat[1].filename, "Show/E02.mkv");
    }
}

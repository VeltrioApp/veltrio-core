//! veltrio-core: shared Rust core for the Veltrio Stremio client.
//!
//! Exposes a Swift/Kotlin-friendly API via UniFFI. Everything async runs on
//! a dedicated multi-threaded Tokio runtime owned by the library, so callers
//! never have to care about executors.

use once_cell::sync::OnceCell;
use tokio::runtime::Runtime;

pub mod addon;
pub mod debrid;
pub mod error;
pub mod iptv;
pub mod torrent;

pub use addon::{
    client::StremioClient,
    types::{
        Catalog, ExtraProp, Manifest, Meta, MetaPreview, Stream, StreamSource, Video,
    },
};
pub use error::VeltrioError;
pub use iptv::{
    client::IptvClient,
    types::{IptvChannel, IptvPlaylist},
};
pub use torrent::TorrentEngine;
pub use debrid::AllDebridClient;

static RUNTIME: OnceCell<Runtime> = OnceCell::new();

/// The HTTP client builder every client starts from. On Android, reqwest's
/// rustls checks certificates through the OS, which needs a JNI setup
/// (rustls-platform-verifier's init_hosted) the apps don't do: there the
/// client trusts Mozilla's root certificates, bundled, instead.
pub(crate) fn http_client_builder() -> reqwest::ClientBuilder {
    let builder = reqwest::Client::builder();
    #[cfg(target_os = "android")]
    let builder = builder.tls_certs_only(
        webpki_root_certs::TLS_SERVER_ROOT_CERTS
            .iter()
            .filter_map(|der| reqwest::Certificate::from_der(der).ok()),
    );
    builder
}

pub(crate) fn runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .thread_name("veltrio-rt")
            .build()
            .expect("failed to build tokio runtime")
    })
}

#[uniffi::export]
pub fn init_logging() {
    use tracing_subscriber::{fmt, EnvFilter};
    let _ = fmt()
        .with_env_filter(
            EnvFilter::try_from_env("VELTRIO_LOG").unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(true)
        .try_init();
    tracing::info!("veltrio-core {} initialized", env!("CARGO_PKG_VERSION"));
}

#[uniffi::export]
pub fn core_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

uniffi::setup_scaffolding!("veltrio");

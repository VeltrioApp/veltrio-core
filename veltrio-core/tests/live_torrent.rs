//! Torrent engine smoke test — confirms the engine starts, the embedded
//! HTTP server responds, and `start_stream` returns a usable URL. We stop
//! short of waiting for pieces so the test stays fast and deterministic.
//!
//! Run with: `cargo test --release --test live_torrent -- --ignored --nocapture`

use veltrio_core::TorrentEngine;

#[tokio::test]
#[ignore]
async fn engine_starts_and_serves_api() {
    let engine = TorrentEngine::new();
    let base = engine.ensure_started().await.expect("start engine");
    println!("engine listening on {base}");
    assert!(base.starts_with("http://127.0.0.1:"));

    // /torrents is the list endpoint; expect a 200 with empty list.
    let resp = reqwest::get(format!("{base}/torrents"))
        .await
        .expect("GET /torrents");
    assert!(resp.status().is_success(), "status = {}", resp.status());
}

#[tokio::test]
#[ignore]
async fn start_stream_returns_valid_url() {
    // Sintel infohash — Blender Foundation, public domain.
    let engine = TorrentEngine::new();
    let url = engine
        .start_stream(
            "08ada5a7a6183aae1e09d831df6748d566095a10".into(),
            vec![],
            Some(0),
        )
        .await
        .expect("start_stream");
    println!("stream url: {url}");
    assert!(url.contains("/stream/0"));
}

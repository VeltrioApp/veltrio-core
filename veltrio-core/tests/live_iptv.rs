//! Live IPTV smoke test using the iptv-org open playlist.
//! Run with: `cargo test --release --test live_iptv -- --ignored --nocapture`

use veltrio_core::IptvClient;

#[tokio::test]
#[ignore]
async fn loads_iptv_org_playlist() {
    let client = IptvClient::new();
    let playlist = client
        .load_playlist("https://iptv-org.github.io/iptv/index.m3u".into())
        .await
        .expect("playlist");
    println!("Loaded {} channels", playlist.channels.len());
    assert!(playlist.channels.len() > 100, "expected many channels");
    let first = &playlist.channels[0];
    println!("First: name={:?} group={:?} url={}", first.name, first.group, first.stream_url);
    assert!(!first.name.is_empty());
}

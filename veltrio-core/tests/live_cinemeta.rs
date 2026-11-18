//! Live network smoke test. Run manually with:
//!
//!   cargo test --test live_cinemeta -- --ignored --nocapture
//!
//! Off by default so CI doesn't flake on a third-party.

use veltrio_core::StremioClient;

#[tokio::test]
#[ignore]
async fn fetches_cinemeta_manifest_and_a_catalog() {
    let client = StremioClient::new();
    let manifest = client
        .fetch_manifest("https://v3-cinemeta.strem.io/manifest.json".into())
        .await
        .expect("manifest");
    assert!(!manifest.catalogs.is_empty(), "cinemeta has catalogs");
    assert!(manifest.types.contains(&"movie".to_string()));

    let first = manifest.catalogs.first().unwrap().clone();
    let items = client
        .fetch_catalog(manifest.transport_url, first.r#type.clone(), first.id, vec![])
        .await
        .expect("catalog");
    println!("{} items in `{}`", items.len(), first.name);
    assert!(!items.is_empty(), "catalog should return items");
}

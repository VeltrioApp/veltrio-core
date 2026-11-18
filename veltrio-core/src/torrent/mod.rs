//! Torrent streaming engine.
//!
//! Wraps `librqbit` and exposes a small, FFI-friendly surface: feed it an
//! `info_hash` + optional `sources` + `file_idx`, get back an `http://127.0.0.1:…`
//! URL that AVPlayer (or any player) can pull with Range requests.
//!
//! We rely on librqbit's built-in HTTP API (axum router) to serve the file;
//! that endpoint already does Range support, waits for metadata to resolve,
//! and prioritises pieces for sequential playback. Our job is to set up the
//! session on a local port, map `info_hash → URL`, and keep the lot alive.

pub mod engine;

pub use engine::TorrentEngine;

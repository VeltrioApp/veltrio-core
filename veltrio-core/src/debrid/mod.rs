//! Debrid providers.
//!
//! Scope of this module: route a Stremio `infoHash` through a debrid service
//! (AllDebrid, later Real-Debrid / TorBox / Premiumize) and return a direct
//! HTTPS URL the player can pull without seeding.
//!
//! Each provider exposes the same shape:
//!   - a PIN / token auth entry point (so the user never pastes an API key)
//!   - `check_availability(hashes)` for the "already cached" badge
//!   - `resolve_stream(hash, season?, episode?)` for playback
//!   - `user_info()` for the account display
//!
//! For now only AllDebrid is implemented; the module boundary is kept so
//! another provider is a small file next to this one.

pub mod alldebrid;
pub mod file_selection;

pub use alldebrid::AllDebridClient;

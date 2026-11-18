//! IPTV support.
//!
//! Scope of this module:
//!   - `m3u`: parse M3U / M3U8 playlists with `#EXTINF` attributes
//!   - `types`: public types exposed over UniFFI
//!   - `client`: `IptvClient` — fetches a playlist URL (http or file),
//!     parses it, and returns structured channels
//!
//! Next up (not in this slice):
//!   - `xtream`: Xtream Codes API (`player_api.php`)
//!   - `xmltv`: EPG parser with now/next lookup

pub mod client;
pub mod m3u;
pub mod types;

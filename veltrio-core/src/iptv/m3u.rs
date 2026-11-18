//! Tiny, dependency-free M3U parser.
//!
//! The format in practice:
//!
//!   #EXTM3U
//!   #EXTINF:-1 tvg-id="bbc1.uk" tvg-logo="https://…" group-title="UK",BBC One HD
//!   http://example.com/live/bbc1.m3u8
//!   #EXTINF:-1 tvg-logo="https://…" group-title="News",France 24
//!   #EXTVLCOPT:http-user-agent=Mozilla/5.0
//!   http://example.com/live/france24.ts
//!
//! Real-world playlists are messy — attribute order shifts, quotes vary,
//! random `#EXTVLCOPT` lines slip in — so we parse tolerantly: any line
//! starting with `#` is metadata, anything else is a candidate stream URL.

use std::collections::HashMap;

use crate::iptv::types::IptvChannel;

/// Parse an M3U buffer into a list of channels. Never fails: malformed
/// entries are silently dropped (we keep what we understood).
pub fn parse(content: &str) -> Vec<IptvChannel> {
    let mut channels = Vec::new();
    let mut current: Option<Pending> = None;

    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("#EXTINF:") {
            current = Some(Pending::from_extinf(rest));
        } else if line.starts_with('#') {
            // #EXTVLCOPT, #EXTM3U, #EXTGRP, comments — ignored.
            if let Some(group) = line.strip_prefix("#EXTGRP:") {
                if let Some(p) = current.as_mut() {
                    p.attrs
                        .entry("group-title".to_string())
                        .or_insert_with(|| group.trim().to_string());
                }
            }
        } else if let Some(pending) = current.take() {
            if let Some(channel) = pending.finish(line.to_string()) {
                channels.push(channel);
            }
        }
    }

    channels
}

struct Pending {
    name: String,
    attrs: HashMap<String, String>,
}

impl Pending {
    fn from_extinf(line: &str) -> Self {
        // `#EXTINF:` → `-1 tvg-id="x" group-title="y",Channel Name`
        //
        // Split on the first *unquoted* comma first (that's the attrs/name
        // divider), then skip the leading duration token in the left half.
        let (head, name_part) = split_outside_quotes(line, ',');
        let attrs_part = match split_once(head.trim_start(), ' ') {
            Some((_duration, attrs)) => attrs,
            None => "", // only a duration, no attributes
        };
        Self {
            name: name_part.trim().to_string(),
            attrs: parse_attributes(attrs_part),
        }
    }

    fn finish(mut self, stream_url: String) -> Option<IptvChannel> {
        if stream_url.is_empty() {
            return None;
        }
        let tvg_id = self.attrs.remove("tvg-id").filter(|s| !s.is_empty());
        let tvg_name = self.attrs.remove("tvg-name").filter(|s| !s.is_empty());
        let logo = self.attrs.remove("tvg-logo").filter(|s| !s.is_empty());
        let group = self.attrs.remove("group-title").filter(|s| !s.is_empty());
        let tvg_country = self.attrs.remove("tvg-country").filter(|s| !s.is_empty());
        let tvg_language = self.attrs.remove("tvg-language").filter(|s| !s.is_empty());

        let name = if self.name.is_empty() {
            tvg_name.clone().unwrap_or_else(|| "Unknown".to_string())
        } else {
            self.name
        };
        let id = tvg_id
            .clone()
            .unwrap_or_else(|| format!("{}|{}", name, stream_url));

        Some(IptvChannel {
            id,
            name,
            stream_url,
            logo_url: logo,
            group,
            tvg_id,
            tvg_name,
            tvg_country,
            tvg_language,
        })
    }
}

fn parse_attributes(part: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let bytes = part.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // skip whitespace
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        // find key up to '='
        let key_start = i;
        while i < bytes.len() && bytes[i] != b'=' && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b'=' {
            break;
        }
        let key = part[key_start..i].to_ascii_lowercase();
        i += 1; // skip '='
        // quoted value?
        if i < bytes.len() && bytes[i] == b'"' {
            i += 1;
            let val_start = i;
            while i < bytes.len() && bytes[i] != b'"' {
                i += 1;
            }
            let value = part[val_start..i].to_string();
            if i < bytes.len() {
                i += 1; // skip closing quote
            }
            map.insert(key, value);
        } else {
            let val_start = i;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            let value = part[val_start..i].to_string();
            map.insert(key, value);
        }
    }
    map
}

fn split_once<'a>(s: &'a str, sep: char) -> Option<(&'a str, &'a str)> {
    s.find(sep).map(|idx| (&s[..idx], &s[idx + sep.len_utf8()..]))
}

/// Returns (before, after) at the first `sep` found **outside** any `"…"`
/// quoted region. Used because the `,name` separator sits after attributes
/// whose values can legitimately contain a comma.
fn split_outside_quotes(s: &str, sep: char) -> (&str, &str) {
    let mut in_quotes = false;
    for (idx, ch) in s.char_indices() {
        if ch == '"' {
            in_quotes = !in_quotes;
        } else if ch == sep && !in_quotes {
            return (&s[..idx], &s[idx + ch.len_utf8()..]);
        }
    }
    (s, "")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "#EXTM3U\n\
#EXTINF:-1 tvg-id=\"bbc1.uk\" tvg-logo=\"https://logos/bbc1.png\" group-title=\"UK\",BBC One HD\n\
#EXTVLCOPT:http-user-agent=Mozilla/5.0\n\
http://ex.com/bbc1.m3u8\n\
#EXTINF:-1 tvg-logo=\"https://logos/f24.png\" group-title=\"News, World\",France 24\n\
http://ex.com/f24.ts\n\
#EXTINF:-1,Bare Channel\n\
http://ex.com/bare.ts\n";

    #[test]
    fn parses_three_channels() {
        let channels = parse(SAMPLE);
        assert_eq!(channels.len(), 3, "expected 3 channels, got {}", channels.len());
    }

    #[test]
    fn extracts_attributes() {
        let channels = parse(SAMPLE);
        let bbc = &channels[0];
        assert_eq!(bbc.name, "BBC One HD");
        assert_eq!(bbc.tvg_id.as_deref(), Some("bbc1.uk"));
        assert_eq!(bbc.logo_url.as_deref(), Some("https://logos/bbc1.png"));
        assert_eq!(bbc.group.as_deref(), Some("UK"));
        assert_eq!(bbc.stream_url, "http://ex.com/bbc1.m3u8");
    }

    #[test]
    fn preserves_commas_in_quoted_attribute() {
        let channels = parse(SAMPLE);
        let f24 = &channels[1];
        assert_eq!(f24.name, "France 24");
        assert_eq!(f24.group.as_deref(), Some("News, World"));
    }

    #[test]
    fn falls_back_without_attributes() {
        let channels = parse(SAMPLE);
        let bare = &channels[2];
        assert_eq!(bare.name, "Bare Channel");
        assert!(bare.tvg_id.is_none());
    }
}

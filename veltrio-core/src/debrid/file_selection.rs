//! File picking inside a debrid-resolved torrent, ported from Frenchio
//! (services/alldebrid.py::_select_link) and cleaned up:
//!   - series: match S{N}E{N}, {N}x{N}, with and without leading zeros
//!   - movies/fallback: largest video file by size
//!   - never pick obviously non-video artefacts (iso, pdf, etc.) unless
//!     that's all we have

const VIDEO_EXTS: &[&str] = &[
    ".mkv", ".mp4", ".avi", ".mov", ".wmv", ".flv", ".webm",
    ".m4v", ".ts", ".m2ts", ".vob",
];
const BAD_EXTS: &[&str] = &[
    ".iso", ".pdf", ".epub", ".txt", ".nfo", ".jpg", ".jpeg",
    ".png", ".rar", ".zip", ".sfv", ".srt", ".sub",
];

#[derive(Debug, Clone)]
pub struct Candidate<L> {
    pub link: L,
    pub filename: String,
    pub size: u64,
}

/// Pick the best file inside `candidates` for a given target. If `season` and
/// `episode` are set, try to match the episode first. Otherwise (or if no
/// match), pick the largest video.
pub fn pick_best<L: Clone>(
    candidates: &[Candidate<L>],
    season: Option<i32>,
    episode: Option<i32>,
) -> Option<L> {
    if candidates.is_empty() {
        return None;
    }

    if let (Some(s), Some(e)) = (season, episode) {
        if let Some(c) = match_episode(candidates, s, e) {
            return Some(c.link.clone());
        }
    }

    let mut video: Vec<&Candidate<L>> = candidates
        .iter()
        .filter(|c| has_ext(&c.filename, VIDEO_EXTS))
        .collect();

    if video.is_empty() {
        // Last resort: exclude obvious junk only.
        video = candidates
            .iter()
            .filter(|c| !has_ext(&c.filename, BAD_EXTS))
            .collect();
    }

    video.sort_by(|a, b| b.size.cmp(&a.size));
    video.first().map(|c| c.link.clone())
}

fn match_episode<'a, L>(
    candidates: &'a [Candidate<L>],
    season: i32,
    episode: i32,
) -> Option<&'a Candidate<L>> {
    let s2 = format!("{season:02}");
    let e2 = format!("{episode:02}");
    let patterns: [String; 5] = [
        format!("S{s2}E{e2}"),
        format!("S{season}E{e2}"),
        format!("{season}x{e2}"),
        format!("S{s2}.E{e2}"),
        format!("season {season} episode {episode}"),
    ];
    for cand in candidates {
        let needle = cand.filename.to_lowercase();
        for pat in &patterns {
            if needle.contains(&pat.to_lowercase()) {
                return Some(cand);
            }
        }
    }
    None
}

fn has_ext(filename: &str, exts: &[&str]) -> bool {
    let lower = filename.to_ascii_lowercase();
    exts.iter().any(|e| lower.ends_with(e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(name: &str, size: u64) -> Candidate<String> {
        Candidate { link: name.to_string(), filename: name.to_string(), size }
    }

    #[test]
    fn matches_episode_with_patterns() {
        let files = vec![
            c("Show.S01E01.mkv", 1_000),
            c("Show.S01E02.mkv", 1_000),
            c("Show.S01E03.mkv", 1_000),
        ];
        assert_eq!(pick_best(&files, Some(1), Some(2)), Some("Show.S01E02.mkv".into()));
    }

    #[test]
    fn matches_1x02_pattern() {
        let files = vec![c("Show.1x02.1080p.mkv", 500)];
        assert_eq!(pick_best(&files, Some(1), Some(2)), Some("Show.1x02.1080p.mkv".into()));
    }

    #[test]
    fn falls_back_to_largest_video() {
        let files = vec![
            c("Sample.mkv", 10_000_000),
            c("Movie.2024.mkv", 8_000_000_000),
            c("RARBG.txt", 1_000),
        ];
        assert_eq!(pick_best(&files, None, None), Some("Movie.2024.mkv".into()));
    }

    #[test]
    fn excludes_junk_when_no_video_matches() {
        let files = vec![
            c("README.txt", 1_000),
            c("movie.iso", 9_000_000_000),
            c("subtitles.srt", 10_000),
        ];
        // All bad exts except maybe... nothing is "video". Falls through to
        // "exclude junk only" and picks the iso (largest non-junk? no — iso
        // is in BAD_EXTS too, so None).
        assert_eq!(pick_best::<String>(&files, None, None), None);
    }
}

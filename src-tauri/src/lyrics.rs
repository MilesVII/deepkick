//! Lyrics client for lrclib.net.
//!
//! Strategy (mirrors SLM's [`ProcessorAPI.pullLyricsIterator`]):
//! 1. Try `GET /api/search?q=<artist> <title>&limit=1`.
//! 2. If that misses, strip any parenthesised suffix from the title and retry.
//! 3. As a last resort, hit `GET /api/get?artist_name=...&track_name=...`.
//!
//! For each hit we prefer synced LRC if present, otherwise the plain
//! `plainLyrics` field.

use std::time::Duration;

use serde::Deserialize;

use crate::model::LyricsError;

const DEFAULT_BASE: &str = "https://lrclib.net";

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(rename = "plainLyrics")]
    plain_lyrics: Option<String>,
    #[serde(rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GetResponse {
    #[serde(rename = "plainLyrics")]
    plain_lyrics: Option<String>,
    #[serde(rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
}

pub struct LyricsClient {
    base: String,
    agent: ureq::Agent,
}

impl Default for LyricsClient {
    fn default() -> Self {
        Self::new(DEFAULT_BASE.to_string())
    }
}

impl LyricsClient {
    pub fn new(base: String) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(15))
            .user_agent("DeepKickLyrics/0.1")
            .build();
        Self { base, agent }
    }

    /// Fetch lyrics for a given artist/title. Returns plain text without
    /// LRC timestamps; returns [`LyricsError::NotFound`] if nothing matches.
    pub fn fetch(&self, artist: &str, title: &str) -> Result<String, LyricsError> {
        let variants = title_variants(title);
        for variant in &variants {
            if let Some(text) = self.try_search(artist, variant)? {
                return Ok(text);
            }
        }
        for variant in &variants {
            if let Some(text) = self.try_get(artist, variant)? {
                return Ok(text);
            }
        }
        Err(LyricsError::NotFound)
    }

    fn try_search(&self, artist: &str, title: &str) -> Result<Option<String>, LyricsError> {
        let q = format!("{} {}", artist, title);
        let url = format!("{}/api/search", self.base);
        let resp = self
            .agent
            .get(&url)
            .query("q", q.as_str())
            .query("limit", "1")
            .call();
        match resp {
            Ok(r) => {
                let arr: Vec<SearchResponse> = r
                    .into_json()
                    .map_err(|e| LyricsError::Invalid(e.to_string()))?;
                Ok(arr.into_iter().next().and_then(pick_text))
            }
            Err(ureq::Error::Status(404, _)) => Ok(None),
            Err(ureq::Error::Status(code, r)) => {
                let body = r.into_string().unwrap_or_default();
                Err(LyricsError::Http(format!("{}: {}", code, body)))
            }
            Err(e) => Err(LyricsError::Http(e.to_string())),
        }
    }

    fn try_get(&self, artist: &str, title: &str) -> Result<Option<String>, LyricsError> {
        let url = format!("{}/api/get", self.base);
        let resp = self
            .agent
            .get(&url)
            .query("artist_name", artist)
            .query("track_name", title)
            .call();
        match resp {
            Ok(r) => {
                let parsed: GetResponse = r
                    .into_json()
                    .map_err(|e| LyricsError::Invalid(e.to_string()))?;
                Ok(pick_text(parsed))
            }
            Err(ureq::Error::Status(404, _)) => Ok(None),
            Err(ureq::Error::Status(code, r)) => {
                let body = r.into_string().unwrap_or_default();
                Err(LyricsError::Http(format!("{}: {}", code, body)))
            }
            Err(e) => Err(LyricsError::Http(e.to_string())),
        }
    }
}

/// Pick the best available lyrics: synced first, then plain.
fn pick_text<T: HasLyrics>(t: T) -> Option<String> {
    let synced = t.synced();
    if let Some(s) = synced {
        if !s.trim().is_empty() {
            // We strip LRC timestamps here; the `USLT` frame is unsynced.
            return Some(strip_lrc_timestamps(&s));
        }
    }
    t.plain().filter(|s| !s.trim().is_empty())
}

trait HasLyrics {
    fn plain(&self) -> Option<String>;
    fn synced(&self) -> Option<String>;
}

impl HasLyrics for SearchResponse {
    fn plain(&self) -> Option<String> {
        self.plain_lyrics.clone()
    }
    fn synced(&self) -> Option<String> {
        self.synced_lyrics.clone()
    }
}

impl HasLyrics for GetResponse {
    fn plain(&self) -> Option<String> {
        self.plain_lyrics.clone()
    }
    fn synced(&self) -> Option<String> {
        self.synced_lyrics.clone()
    }
}

fn strip_lrc_timestamps(input: &str) -> String {
    // LRC lines look like: "[00:12.34] Lyric text". Drop the timestamp.
    let mut out = String::with_capacity(input.len());
    for line in input.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix('[') {
            if let Some(close) = rest.find(']') {
                let head = &rest[..close];
                if head.chars().all(|c| c.is_ascii_digit() || c == ':' || c == '.') {
                    let text = rest[close + 1..].trim_start();
                    out.push_str(text);
                    out.push('\n');
                    continue;
                }
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    out.trim_end().to_string()
}

/// Produce a list of title variants to try: the original, then progressively
/// shorter forms stripping parenthesised segments like "(feat. X)",
/// "(Remix)", "(Radio Edit)", etc. Mirrors SLM's parenthesised-title retry.
fn title_variants(title: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut variants = Vec::new();
    let mut current = title.to_string();
    loop {
        if seen.insert(current.clone()) {
            variants.push(current.clone());
        }
        match find_last_paren(&current) {
            Some(idx) => {
                let mut next = current[..idx].to_string();
                // Trim trailing space and dash separators.
                while next.ends_with(' ') || next.ends_with('-') {
                    next.pop();
                }
                if next.is_empty() {
                    break;
                }
                current = next;
            }
            None => break,
        }
    }
    variants
}

fn find_last_paren(s: &str) -> Option<usize> {
    s.rfind('(')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_parenthesised_suffixes() {
        let v = title_variants("Song Name (feat. X) (Remix)");
        assert_eq!(v[0], "Song Name (feat. X) (Remix)");
        assert!(v.contains(&"Song Name (feat. X)".to_string()));
        assert!(v.contains(&"Song Name".to_string()));
    }

    #[test]
    fn strips_lrc() {
        let input = "[00:12.34] First line\n[00:13.00]Second line";
        let out = strip_lrc_timestamps(input);
        assert_eq!(out, "First line\nSecond line");
    }
}

use crate::error::{AppError, Result};
use crate::types::LyricsResult;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

const LRCLIB_BASE_URL: &str = "https://lrclib.net/api";
const USER_AGENT: &str = "DeepKickLyrics/1.0";

#[derive(Deserialize)]
struct LrcLibTrack {
    id: u64,
    #[serde(rename = "trackName")]
    track_name: String,
    #[serde(rename = "artistName")]
    artist_name: String,
    #[serde(rename = "albumName")]
    album_name: String,
    duration: u64,
    instrumental: bool,
    #[serde(rename = "plainLyrics")]
    plain_lyrics: Option<String>,
    #[serde(rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
}

pub struct LyricsFetcher {
    client: Client,
    base_url: String,
}

impl LyricsFetcher {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent(USER_AGENT)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: LRCLIB_BASE_URL.to_string(),
        }
    }

    pub fn with_base_url(mut self, url: String) -> Self {
        self.base_url = url;
        self
    }

    pub async fn fetch_lyrics(&self, artist: &str, title: &str) -> Result<LyricsResult> {
        // Try exact match first
        if let Ok(result) = self.fetch_exact(artist, title).await {
            return Ok(result);
        }

        // Fallback to search
        self.fetch_search(artist, title).await
    }

    async fn fetch_exact(&self, artist: &str, title: &str) -> Result<LyricsResult> {
        let url = format!(
            "{}/get?artist_name={}&track_name={}",
            self.base_url,
            urlencoding::encode(artist),
            urlencoding::encode(title)
        );

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(AppError::LyricsFetch(format!(
                "API returned status: {}",
                response.status()
            )));
        }

        let track: LrcLibTrack = response.json().await?;

        if track.instrumental {
            return Err(AppError::LyricsFetch("Track is instrumental".into()));
        }

        Ok(LyricsResult {
            plain_lyrics: track.plain_lyrics,
            synced_lyrics: track.synced_lyrics,
            source_url: format!("https://lrclib.net/lyrics/{}", track.id),
            instrumental: track.instrumental,
        })
    }

    async fn fetch_search(&self, artist: &str, title: &str) -> Result<LyricsResult> {
        let query = format!("{} {}", artist, title);
        let url = format!(
            "{}/search?q={}",
            self.base_url,
            urlencoding::encode(&query)
        );

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(AppError::LyricsFetch(format!(
                "Search API returned status: {}",
                response.status()
            )));
        }

        let tracks: Vec<LrcLibTrack> = response.json().await?;

        // Find best match
        let best_match = tracks
            .into_iter()
            .filter(|t| !t.instrumental)
            .min_by_key(|t| {
                // Simple scoring: prefer exact artist/title matches
                let artist_diff = levenshtein_distance(&t.artist_name.to_lowercase(), &artist.to_lowercase());
                let title_diff = levenshtein_distance(&t.track_name.to_lowercase(), &title.to_lowercase());
                artist_diff + title_diff
            });

        match best_match {
            Some(track) => Ok(LyricsResult {
                plain_lyrics: track.plain_lyrics,
                synced_lyrics: track.synced_lyrics,
                source_url: format!("https://lrclib.net/lyrics/{}", track.id),
                instrumental: track.instrumental,
            }),
            None => Err(AppError::LyricsNotFound),
        }
    }
}

impl Default for LyricsFetcher {
    fn default() -> Self {
        Self::new()
    }
}

// Simple Levenshtein distance for fuzzy matching
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let (m, n) = (a_chars.len(), b_chars.len());

    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }

    let mut dp = vec![vec![0; n + 1]; m + 1];

    for i in 0..=m {
        dp[i][0] = i;
    }
    for j in 0..=n {
        dp[0][j] = j;
    }

    for i in 1..=m {
        for j in 1..=n {
            let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
            dp[i][j] = std::cmp::min(
                std::cmp::min(dp[i - 1][j] + 1, dp[i][j - 1] + 1),
                dp[i - 1][j - 1] + cost,
            );
        }
    }

    dp[m][n]
}
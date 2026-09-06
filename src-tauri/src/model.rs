//! Shared data types exchanged with the frontend.

use serde::{Deserialize, Serialize};

/// Kind of audio file we support.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AudioKind {
    Mp3,
    Flac,
}

impl AudioKind {
    pub fn from_uri(uri: &str) -> Option<Self> {
        let lower = uri.to_lowercase();
        if lower.ends_with(".mp3") {
            Some(AudioKind::Mp3)
        } else if lower.ends_with(".flac") {
            Some(AudioKind::Flac)
        } else {
            None
        }
    }
}

/// A single audio file surfaced to the UI after picking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioFile {
    /// Content URI (or filesystem path on desktop).
    pub uri: String,
    /// Filename only.
    pub name: String,
    /// File kind.
    pub kind: AudioKind,
    /// Optional size in bytes (populated by the scanner).
    pub size_bytes: Option<u64>,
}

/// Metadata for a single audio file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Metadata {
    pub artist: Option<String>,
    pub title: Option<String>,
    pub album: Option<String>,
    pub has_lyrics: bool,
    pub lyrics_len: Option<usize>,
}

/// Per-file processing outcome.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PerFileStatus {
    Ok,
    NotFound,
    NoTag,
    Existing,
    Err,
}

/// Aggregated report produced by `process_queue`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Report {
    pub total: usize,
    pub ok: usize,
    pub not_found: usize,
    pub no_tag: usize,
    pub existing: usize,
    pub errors: usize,
    /// Detailed per-file outcomes for the UI.
    pub entries: Vec<ReportEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportEntry {
    pub uri: String,
    pub name: String,
    pub status: PerFileStatus,
    pub source_link: Option<String>,
    pub error: Option<String>,
}

/// Specification for `process_queue`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueSpec {
    pub files: Vec<AudioFile>,
    /// Overwrite existing lyrics if present.
    pub overwrite: bool,
    /// Optional API base URL override (defaults to lrclib).
    pub api_base: Option<String>,
}

/// Lyrics lookup errors.
#[derive(Debug, thiserror::Error)]
pub enum LyricsError {
    #[error("Lyrics not found")]
    NotFound,
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("Invalid response: {0}")]
    Invalid(String),
}

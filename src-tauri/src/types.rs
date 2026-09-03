use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackMetadata {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub year: Option<i32>,
    pub track_number: Option<u32>,
    pub duration: Option<u64>, // in milliseconds
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsResult {
    pub plain_lyrics: Option<String>,
    pub synced_lyrics: Option<String>,
    pub source_url: String,
    pub instrumental: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackInfo {
    pub id: String,
    pub path: String,
    pub name: String,
    pub format: AudioFormat,
    pub metadata: TrackMetadata,
    pub lyrics_status: LyricsStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioFormat {
    Mp3,
    Flac,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsStatus {
    pub has_lyrics: bool,
    pub lyrics: Option<String>,
    pub source: Option<String>,
    pub synced: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackTask {
    pub track: TrackInfo,
    pub overwrite_existing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessingProgress {
    pub session_id: String,
    pub current_index: usize,
    pub total: usize,
    pub current_track: Option<TrackInfo>,
    pub status: ProcessingStatus,
    pub stats: ProcessingStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProcessingStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProcessingStats {
    pub total: usize,
    pub processed: usize,
    pub found: usize,
    pub not_found: usize,
    pub errors: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub path: String,
    pub name: String,
    pub is_directory: bool,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchResult {
    pub session_id: String,
    pub stats: ProcessingStats,
    pub tracks: Vec<TrackInfo>,
}

impl TrackInfo {
    pub fn new(path: PathBuf, format: AudioFormat) -> Result<Self, crate::error::AppError> {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| crate::error::AppError::InvalidPath("Invalid file name".into()))?
            .to_string();

        let id = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(path.to_string_lossy().as_bytes());
            format!("{:x}", hasher.finalize())[..16].to_string()
        };

        Ok(Self {
            id,
            path: path.to_string_lossy().to_string(),
            name,
            format,
            metadata: TrackMetadata {
                title: String::new(),
                artist: String::new(),
                album: None,
                year: None,
                track_number: None,
                duration: None,
            },
            lyrics_status: LyricsStatus {
                has_lyrics: false,
                lyrics: None,
                source: None,
                synced: false,
            },
        })
    }
}
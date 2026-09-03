use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Metadata read error: {0}")]
    MetadataRead(String),

    #[error("Metadata write error: {0}")]
    MetadataWrite(String),

    #[error("Lyrics fetch error: {0}")]
    LyricsFetch(String),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Walkdir error: {0}")]
    Walkdir(#[from] walkdir::Error),

    #[error("ID3 error: {0}")]
    Id3(#[from] id3::Error),

    #[error("Metaflac error: {0}")]
    Metaflac(#[from] metaflac::Error),

    #[error("Lofty error: {0}")]
    Lofty(String),

    #[error("Invalid file format: {0}")]
    InvalidFormat(String),

    #[error("No metadata tags found")]
    NoTags,

    #[error("Lyrics not found")]
    LyricsNotFound,

    #[error("Operation cancelled")]
    Cancelled,

    #[error("Invalid path: {0}")]
    InvalidPath(String),
}

impl From<AppError> for String {
    fn from(err: AppError) -> Self {
        err.to_string()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
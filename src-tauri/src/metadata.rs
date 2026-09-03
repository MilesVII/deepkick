use crate::error::{AppError, Result};
use crate::types::{AudioFormat, TrackInfo, TrackMetadata};
use lofty::prelude::*;
use lofty::probe::Probe;
use std::path::Path;

pub fn read_metadata(path: &Path) -> Result<TrackMetadata> {
    let tagged_file = Probe::open(path)
        .map_err(|e| AppError::MetadataRead(format!("Failed to open file: {}", e)))?
        .read()
        .map_err(|e| AppError::MetadataRead(format!("Failed to read tags: {}", e)))?;

    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag())
        .ok_or(AppError::NoTags)?;

    let properties = tagged_file.properties();

    Ok(TrackMetadata {
        title: tag.title().map(|s| s.into_owned()).unwrap_or_default(),
        artist: tag.artist().map(|s| s.into_owned()).unwrap_or_default(),
        album: tag.album().map(|s| s.into_owned()),
        year: None,
        track_number: tag.track(),
        duration: Some(properties.duration().as_millis() as u64),
    })
}

pub fn detect_format(path: &Path) -> Result<AudioFormat> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match extension.as_str() {
        "mp3" => Ok(AudioFormat::Mp3),
        "flac" => Ok(AudioFormat::Flac),
        _ => Err(AppError::InvalidFormat(format!(
            "Unsupported format: {}",
            extension
        ))),
    }
}

pub fn read_track_info(path: &Path) -> Result<TrackInfo> {
    let format = detect_format(path)?;
    let mut track = TrackInfo::new(path.to_path_buf(), format)?;
    track.metadata = read_metadata(path)?;
    track.lyrics_status = check_existing_lyrics(path, track.format.clone())?;
    Ok(track)
}

fn check_existing_lyrics(path: &Path, format: AudioFormat) -> Result<crate::types::LyricsStatus> {
    match format {
        AudioFormat::Mp3 => check_mp3_lyrics(path),
        AudioFormat::Flac => check_flac_lyrics(path),
    }
}

fn check_mp3_lyrics(path: &Path) -> Result<crate::types::LyricsStatus> {
    use id3::Tag;

    let tag = Tag::read_from_path(path).map_err(|e| AppError::MetadataRead(e.to_string()))?;

    for frame in tag.frames() {
        // Check for lyrics frames by frame ID
        if frame.id() == "USLT" || frame.id() == "ULT" {
            if let Some(content) = frame.content().text() {
                return Ok(crate::types::LyricsStatus {
                    has_lyrics: true,
                    lyrics: Some(content.to_string()),
                    source: None,
                    synced: false,
                });
            }
        }
    }

    Ok(crate::types::LyricsStatus {
        has_lyrics: false,
        lyrics: None,
        source: None,
        synced: false,
    })
}

fn check_flac_lyrics(path: &Path) -> Result<crate::types::LyricsStatus> {
    use metaflac::Tag;

    let tag = Tag::read_from_path(path).map_err(|e| AppError::MetadataRead(e.to_string()))?;

    if let Some(vorbis) = tag.vorbis_comments() {
        if let Some(lyrics) = vorbis.get("LYRICS").and_then(|v| v.first()) {
            return Ok(crate::types::LyricsStatus {
                has_lyrics: true,
                lyrics: Some(lyrics.clone()),
                source: None,
                synced: false,
            });
        }
    }

    Ok(crate::types::LyricsStatus {
        has_lyrics: false,
        lyrics: None,
        source: None,
        synced: false,
    })
}
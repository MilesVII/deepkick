use crate::error::{AppError, Result};
use crate::types::AudioFormat;
use id3::{Content, Frame, Tag, Version};
use metaflac::Tag as FlacTag;
use std::fs;
use std::path::Path;

pub fn write_lyrics(path: &Path, lyrics: &str, format: AudioFormat) -> Result<()> {
    match format {
        AudioFormat::Mp3 => write_mp3_lyrics(path, lyrics),
        AudioFormat::Flac => write_flac_lyrics(path, lyrics),
    }
}

pub fn remove_lyrics(path: &Path, format: AudioFormat) -> Result<()> {
    match format {
        AudioFormat::Mp3 => remove_mp3_lyrics(path),
        AudioFormat::Flac => remove_flac_lyrics(path),
    }
}

fn write_mp3_lyrics(path: &Path, lyrics: &str) -> Result<()> {
    let mut tag = Tag::read_from_path(path).unwrap_or_else(|_| Tag::new());

    // Remove existing USLT frames by iterating and collecting IDs
    let mut ids_to_remove = Vec::new();
    for frame in tag.frames() {
        let id = frame.id().to_string();
        if id == "USLT" || id == "ULT" {
            ids_to_remove.push(id);
        }
    }
    for id in ids_to_remove {
        tag.remove(&id);
    }

    // Add new USLT frame using raw frame creation
    let mut frame_data = Vec::new();
    frame_data.push(0x03); // encoding: UTF-8
    frame_data.extend_from_slice(b"eng"); // language: eng
    frame_data.push(0x00); // null terminator
    frame_data.push(0x00); // content descriptor: empty
    frame_data.extend_from_slice(lyrics.as_bytes());

    let lyrics_frame = Frame::with_content("USLT", Content::Unknown(frame_data));
    tag.add_frame(lyrics_frame);

    // Write to a temporary file first, then atomically replace
    let temp_path = path.with_extension("tmp.mp3");
    tag.write_to_path(&temp_path, Version::Id3v24)
        .map_err(|e| AppError::MetadataWrite(format!("Failed to write ID3 tag: {}", e)))?;

    // Atomic replace
    fs::rename(&temp_path, path).map_err(|e| AppError::MetadataWrite(e.to_string()))?;

    Ok(())
}

fn remove_mp3_lyrics(path: &Path) -> Result<()> {
    let mut tag = Tag::read_from_path(path).map_err(|e| AppError::MetadataRead(e.to_string()))?;

    let mut ids_to_remove = Vec::new();
    for frame in tag.frames() {
        let id = frame.id().to_string();
        if id == "USLT" || id == "ULT" {
            ids_to_remove.push(id);
        }
    }
    for id in ids_to_remove {
        tag.remove(&id);
    }

    let temp_path = path.with_extension("tmp.mp3");
    tag.write_to_path(&temp_path, Version::Id3v24)
        .map_err(|e| AppError::MetadataWrite(format!("Failed to write ID3 tag: {}", e)))?;

    fs::rename(&temp_path, path).map_err(|e| AppError::MetadataWrite(e.to_string()))?;

    Ok(())
}

fn write_flac_lyrics(path: &Path, lyrics: &str) -> Result<()> {
    let mut tag = FlacTag::read_from_path(path).map_err(|e| AppError::MetadataRead(e.to_string()))?;

    // Get vorbis comments
    let vorbis = tag.vorbis_comments_mut();

    // Remove existing LYRICS
    vorbis.remove("LYRICS");

    // Add new LYRICS
    vorbis.set("LYRICS".to_string(), vec![lyrics.to_string()]);

    // Write to temporary file first
    let temp_path = path.with_extension("tmp.flac");
    tag.write_to_path(&temp_path)
        .map_err(|e| AppError::MetadataWrite(format!("Failed to write FLAC tag: {}", e)))?;

    fs::rename(&temp_path, path).map_err(|e| AppError::MetadataWrite(e.to_string()))?;

    Ok(())
}

fn remove_flac_lyrics(path: &Path) -> Result<()> {
    let mut tag = FlacTag::read_from_path(path).map_err(|e| AppError::MetadataRead(e.to_string()))?;

    let vorbis = tag.vorbis_comments_mut();
    vorbis.remove("LYRICS");

    let temp_path = path.with_extension("tmp.flac");
    tag.write_to_path(&temp_path)
        .map_err(|e| AppError::MetadataWrite(format!("Failed to write FLAC tag: {}", e)))?;

    fs::rename(&temp_path, path).map_err(|e| AppError::MetadataWrite(e.to_string()))?;

    Ok(())
}
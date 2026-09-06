//! Read and write audio tag metadata.
//!
//! For MP3 files we use the `id3` crate, writing a `USLT` (unsynced lyrics)
//! frame. For FLAC files we use `metaflac` to set the Vorbis comment
//! `LYRICS=...`.
//!
//! All operations are performed on a temporary copy in the app's cache dir and
//! the result is then written back through the same [`TagIo`] abstraction,
//! which on Android uses a `content://` URI via `ContentResolver` and on other
//! platforms uses regular filesystem IO.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use id3::frame::Lyrics;
use id3::{no_tag_ok, Tag, TagLike, Version};
use metaflac::block::VorbisComment;
use tempfile::NamedTempFile;

use crate::model::{AudioKind, AudioFile, Metadata};

/// Trait abstracting the underlying IO mechanism for a single audio file.
///
/// On Android this bridges to `ContentResolver`; on desktop it is a thin
/// wrapper over the local filesystem. The trait exists so we can stub it in
/// tests and so the tag-handling logic stays platform-agnostic.
pub trait TagIo {
    /// Materialize the file to a local temp path the metadata libraries can
    /// operate on.
    fn materialize(&self) -> Result<NamedTempFile, TagError>;
    /// Write the (mutated) temp file back to the original location.
    fn write_back(&self, tmp: &NamedTempFile) -> Result<(), TagError>;
}

/// Filesystem-based implementation for desktop and tests.
pub struct FsTagIo {
    pub path: PathBuf,
}

impl TagIo for FsTagIo {
    fn materialize(&self) -> Result<NamedTempFile, TagError> {
        let mut tmp = tempfile::Builder::new()
            .prefix("deepkick-")
            .suffix(".audio")
            .tempfile()
            .map_err(TagError::Io)?;
        let mut src = fs::File::open(&self.path).map_err(TagError::Io)?;
        std::io::copy(&mut src, tmp.as_file_mut()).map_err(TagError::Io)?;
        tmp.as_file_mut().flush().map_err(TagError::Io)?;
        Ok(tmp)
    }

    fn write_back(&self, tmp: &NamedTempFile) -> Result<(), TagError> {
        let tmp_path = tmp.path().to_path_buf();
        let mut src = fs::File::open(&tmp_path).map_err(TagError::Io)?;
        let mut dst = fs::File::create(&self.path).map_err(TagError::Io)?;
        std::io::copy(&mut src, &mut dst).map_err(TagError::Io)?;
        dst.flush().map_err(TagError::Io)?;
        Ok(())
    }
}

/// Errors that can occur while reading or writing tags.
#[derive(Debug, thiserror::Error)]
pub enum TagError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("ID3 error: {0}")]
    Id3(String),
    #[error("FLAC error: {0}")]
    Flac(#[from] metaflac::Error),
    #[error("Unsupported kind: {0:?}")]
    Unsupported(AudioKind),
    #[error("Not a valid FLAC file")]
    NotFlac,
    #[error("Not a valid MP3 file")]
    NotMp3,
}

impl From<id3::Error> for TagError {
    fn from(e: id3::Error) -> Self {
        TagError::Id3(e.to_string())
    }
}

/// Read metadata for a single audio file.
pub fn read_metadata(io: &dyn TagIo, kind: AudioKind) -> Result<Metadata, TagError> {
    match kind {
        AudioKind::Mp3 => read_mp3(io),
        AudioKind::Flac => read_flac(io),
    }
}

/// Write (or remove) lyrics for a single audio file.
pub fn set_lyrics(io: &dyn TagIo, kind: AudioKind, lyrics: &str) -> Result<(), TagError> {
    match kind {
        AudioKind::Mp3 => write_mp3_lyrics(io, Some(lyrics)),
        AudioKind::Flac => write_flac_lyrics(io, Some(lyrics)),
    }
}

/// Remove lyrics from a single audio file. No-op if absent.
pub fn clear_lyrics(io: &dyn TagIo, kind: AudioKind) -> Result<(), TagError> {
    match kind {
        AudioKind::Mp3 => write_mp3_lyrics(io, None),
        AudioKind::Flac => write_flac_lyrics(io, None),
    }
}

/// Read the existing lyrics (if any) from a file.
pub fn read_lyrics(io: &dyn TagIo, kind: AudioKind) -> Result<Option<String>, TagError> {
    match kind {
        AudioKind::Mp3 => read_mp3_lyrics(io),
        AudioKind::Flac => read_flac_lyrics(io),
    }
}

// ---------- MP3 ----------

fn load_mp3_tag(io: &dyn TagIo) -> Result<Option<Tag>, TagError> {
    let tmp = io.materialize()?;
    let res = Tag::read_from_path(tmp.path());
    match no_tag_ok(res) {
        Ok(opt) => Ok(opt),
        Err(e) => Err(TagError::Id3(e.to_string())),
    }
}

fn read_mp3(io: &dyn TagIo) -> Result<Metadata, TagError> {
    let tag = match load_mp3_tag(io)? {
        Some(t) => t,
        None => return Ok(Metadata::default()),
    };
    let lyrics_opt = tag.lyrics().next().map(|l| l.text.to_string());
    let artist = tag.artist().map(|s| s.to_string());
    let title = tag.title().map(|s| s.to_string());
    let album = tag.album().map(|s| s.to_string());
    Ok(Metadata {
        artist,
        title,
        album,
        has_lyrics: lyrics_opt.as_ref().map(|s| !s.is_empty()).unwrap_or(false),
        lyrics_len: lyrics_opt.as_ref().map(|s| s.len()),
    })
}

fn read_mp3_lyrics(io: &dyn TagIo) -> Result<Option<String>, TagError> {
    let tag = match load_mp3_tag(io)? {
        Some(t) => t,
        None => return Ok(None),
    };
    let lyrics = tag.lyrics().next().map(|l| l.text.to_string());
    Ok(lyrics)
}

fn write_mp3_lyrics(io: &dyn TagIo, lyrics: Option<&str>) -> Result<(), TagError> {
    let tmp = io.materialize()?;
    let mut tag = Tag::read_from_path(tmp.path()).unwrap_or_else(|_| Tag::new());
    // Remove any existing USLT frames first.
    tag.remove("USLT");
    if let Some(text) = lyrics {
        if !text.is_empty() {
            let frame = Lyrics {
                lang: "eng".into(),
                description: String::new(),
                text: text.to_string(),
            };
            tag.add_frame(frame);
        }
    }
    tag.write_to_path(tmp.path(), Version::Id3v24)?;
    io.write_back(&tmp)?;
    Ok(())
}

// ---------- FLAC ----------

fn first_str(v: Option<&Vec<String>>) -> Option<String> {
    v.and_then(|list| list.first().cloned())
}

fn read_flac(io: &dyn TagIo) -> Result<Metadata, TagError> {
    let tmp = io.materialize()?;
    let tag = metaflac::Tag::read_from_path(tmp.path())?;
    let vorbis = match tag.vorbis_comments() {
        Some(v) => v,
        None => return Ok(Metadata::default()),
    };
    let lyrics = first_str(vorbis.lyrics());
    Ok(Metadata {
        artist: first_str(vorbis.artist()),
        title: first_str(vorbis.title()),
        album: first_str(vorbis.album()),
        has_lyrics: lyrics.as_ref().map(|s| !s.is_empty()).unwrap_or(false),
        lyrics_len: lyrics.as_ref().map(|s| s.len()),
    })
}

fn read_flac_lyrics(io: &dyn TagIo) -> Result<Option<String>, TagError> {
    let tmp = io.materialize()?;
    let tag = metaflac::Tag::read_from_path(tmp.path())?;
    Ok(tag.vorbis_comments().and_then(|v| first_str(v.lyrics())))
}

fn write_flac_lyrics(io: &dyn TagIo, lyrics: Option<&str>) -> Result<(), TagError> {
    let tmp = io.materialize()?;
    let mut tag = metaflac::Tag::read_from_path(tmp.path())?;
    let vorbis: &mut VorbisComment = tag.vorbis_comments_mut();
    match lyrics {
        Some(text) if !text.is_empty() => vorbis.set_lyrics(vec![text.to_string()]),
        _ => vorbis.remove_lyrics(),
    }
    tag.write_to_path(tmp.path())?;
    io.write_back(&tmp)?;
    Ok(())
}

/// Convenience: build an `FsTagIo` from an `AudioFile` (URI form `file://...` or plain path).
pub fn fs_io_for(file: &AudioFile) -> FsTagIo {
    let path = uri_to_path(&file.uri);
    FsTagIo { path }
}

/// Decode a `file://` URI or a plain path to a `PathBuf`.
pub fn uri_to_path(uri: &str) -> PathBuf {
    if let Some(rest) = uri.strip_prefix("file://") {
        let trimmed = rest.trim_start_matches('/');
        if let Some(idx) = trimmed.find('/') {
            // skip the storage-volume segment (e.g. `primary:` or `1234-5678:`)
            let after_volume = &trimmed[idx + 1..];
            return PathBuf::from(format!("/{}", after_volume));
        }
        PathBuf::from(trimmed)
    } else {
        PathBuf::from(uri)
    }
}

/// Locate file by URI/path on disk and read its metadata. Convenience wrapper
/// for desktop / tests.
pub fn read_metadata_for_path(path: &Path) -> Result<Metadata, TagError> {
    let uri = path.to_string_lossy().into_owned();
    let kind = AudioKind::from_uri(&uri)
        .ok_or(AudioKind::Mp3)
        .map_err(TagError::Unsupported)?;
    read_metadata(&FsTagIo { path: path.to_path_buf() }, kind)
}

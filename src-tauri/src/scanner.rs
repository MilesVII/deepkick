//! File and directory scanning utilities.

use std::fs;
use std::path::Path;

use walkdir::WalkDir;

use crate::model::{AudioFile, AudioKind};

/// Walk a directory recursively, returning every MP3/FLAC file found.
pub fn scan_dir(path: &Path) -> Result<Vec<AudioFile>, std::io::Error> {
    let mut out = Vec::new();
    for entry in WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let p = entry.path();
        let uri = p.to_string_lossy().into_owned();
        if let Some(kind) = AudioKind::from_uri(&uri) {
            let size_bytes = fs::metadata(p).ok().map(|m| m.len());
            out.push(AudioFile {
                uri,
                name: p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                kind,
                size_bytes,
            });
        }
    }
    Ok(out)
}

/// Convert a single file path into an `AudioFile` (filters by extension).
pub fn file_to_audio(path: &Path) -> Option<AudioFile> {
    let uri = path.to_string_lossy().into_owned();
    let kind = AudioKind::from_uri(&uri)?;
    let size_bytes = fs::metadata(path).ok().map(|m| m.len());
    Some(AudioFile {
        uri,
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        kind,
        size_bytes,
    })
}

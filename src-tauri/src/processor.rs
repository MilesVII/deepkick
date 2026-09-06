//! Queue processing: fetch lyrics for a list of files and embed them in place.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::{AppHandle, Emitter};

use crate::lyrics::LyricsClient;
use crate::model::{
    AudioFile, PerFileStatus, QueueSpec, Report, ReportEntry,
};
use crate::tags::{self, FsTagIo, TagError, TagIo};

/// Shared cancellation flag for the currently running queue.
pub struct CancelState(pub Arc<AtomicBool>);

impl Default for CancelState {
    fn default() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }
}

impl CancelState {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
    pub fn reset(&self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ProgressEvent {
    Started { total: usize },
    File { index: usize, name: String, status: PerFileStatus, preview: Option<String>, error: Option<String> },
    Done { report: Report },
    Aborted,
}

/// Process the entire queue. Emits progress events on the `processor://` channel.
pub fn process_queue(
    app: &AppHandle,
    cancel: Arc<AtomicBool>,
    spec: QueueSpec,
) -> Report {
    cancel.store(false, Ordering::SeqCst);

    let total = spec.files.len();
    let _ = app.emit(
        "processor://event",
        ProgressEvent::Started { total },
    );

    let client = LyricsClient::new(
        spec.api_base
            .clone()
            .unwrap_or_else(|| "https://lrclib.net".to_string()),
    );

    let mut report = Report {
        total,
        ..Default::default()
    };

    for (index, file) in spec.files.iter().enumerate() {
        if cancel.load(Ordering::SeqCst) {
            let _ = app.emit("processor://event", ProgressEvent::Aborted);
            return report;
        }
        let entry = process_one(&client, file, spec.overwrite, cancel.clone());
        match entry.status {
            PerFileStatus::Ok => report.ok += 1,
            PerFileStatus::NotFound => report.not_found += 1,
            PerFileStatus::NoTag => report.no_tag += 1,
            PerFileStatus::Existing => report.existing += 1,
            PerFileStatus::Err => report.errors += 1,
        }
        let preview = if entry.status == PerFileStatus::Ok {
            // Re-read lyrics to surface a snippet.
            read_lyrics_snippet(file).ok().flatten()
        } else {
            None
        };
        let _ = app.emit(
            "processor://event",
            ProgressEvent::File {
                index,
                name: entry.name.clone(),
                status: entry.status,
                preview,
                error: entry.error.clone(),
            },
        );
        report.entries.push(entry);
    }

    let _ = app.emit("processor://event", ProgressEvent::Done { report: report.clone() });
    report
}

fn process_one(
    client: &LyricsClient,
    file: &AudioFile,
    overwrite: bool,
    cancel: Arc<AtomicBool>,
) -> ReportEntry {
    let path = tags::uri_to_path(&file.uri);
    let io: Box<dyn TagIo> = Box::new(FsTagIo { path });

    // 1) Read existing metadata.
    let meta = match tags::read_metadata(io.as_ref(), file.kind) {
        Ok(m) => m,
        Err(e) => {
            return report_entry(file, PerFileStatus::Err, None, Some(format!("read: {e}")));
        }
    };

    // 2) Need artist + title to search.
    let (artist, title) = match (meta.artist, meta.title) {
        (Some(a), Some(t)) if !a.trim().is_empty() && !t.trim().is_empty() => (a, t),
        _ => {
            return report_entry(file, PerFileStatus::NoTag, None, None);
        }
    };

    // 3) If existing lyrics and not overwriting, skip.
    if meta.has_lyrics && !overwrite {
        return report_entry(file, PerFileStatus::Existing, None, None);
    }

    // 4) Fetch lyrics.
    if cancel.load(Ordering::SeqCst) {
        return report_entry(file, PerFileStatus::Err, None, Some("cancelled".into()));
    }
    let lyrics = match client.fetch(&artist, &title) {
        Ok(l) => l,
        Err(crate::model::LyricsError::NotFound) => {
            return report_entry(file, PerFileStatus::NotFound, None, None);
        }
        Err(e) => {
            return report_entry(file, PerFileStatus::Err, None, Some(format!("fetch: {e}")));
        }
    };

    // 5) Write back.
    if let Err(e) = tags::set_lyrics(io.as_ref(), file.kind, &lyrics) {
        return report_entry(file, PerFileStatus::Err, None, Some(format!("write: {e}")));
    }

    report_entry(file, PerFileStatus::Ok, None, None)
}

fn report_entry(
    file: &AudioFile,
    status: PerFileStatus,
    source_link: Option<String>,
    error: Option<String>,
) -> ReportEntry {
    ReportEntry {
        uri: file.uri.clone(),
        name: file.name.clone(),
        status,
        source_link,
        error,
    }
}

fn read_lyrics_snippet(file: &AudioFile) -> Result<Option<String>, TagError> {
    let path = tags::uri_to_path(&file.uri);
    let io = FsTagIo { path };
    tags::read_lyrics(&io, file.kind)
}

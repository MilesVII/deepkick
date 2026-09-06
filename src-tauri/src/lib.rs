// DeepKick Lyrics — Tauri 2 backend.
//
// Modules:
//   * `model`     — shared types exchanged with the frontend.
//   * `tags`      — read/write ID3v2 USLT (MP3) and Vorbis LYRICS (FLAC).
//   * `scanner`   — directory walking for picking folders.
//   * `lyrics`    — lrclib.net HTTP client with search-variant fallback.
//   * `processor` — async queue processor with cancellable progress events.

mod lyrics;
mod model;
mod processor;
mod scanner;
mod tags;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::model::{AudioFile, Metadata, QueueSpec, Report};
use crate::processor::CancelState;

#[derive(Serialize)]
struct CommandError {
    message: String,
}

impl<E: std::fmt::Display> From<E> for CommandError {
    fn from(e: E) -> Self {
        Self { message: e.to_string() }
    }
}

// ---------- Commands ----------

/// Scan a directory recursively and return every MP3/FLAC file found.
#[tauri::command]
async fn scan_directory(path: String) -> Result<Vec<AudioFile>, CommandError> {
    let p = PathBuf::from(&path);
    let files = scanner::scan_dir(&p).map_err(CommandError::from)?;
    Ok(files)
}

/// Read metadata (artist/title/album + lyrics presence) for a file path.
#[tauri::command]
async fn read_metadata(file: AudioFile) -> Result<Metadata, CommandError> {
    let path = tags::uri_to_path(&file.uri);
    let io = tags::FsTagIo { path };
    let meta = tags::read_metadata(&io, file.kind).map_err(CommandError::from)?;
    Ok(meta)
}

/// Set lyrics for a single file. Overwrites any existing lyrics.
#[tauri::command]
async fn set_lyrics(file: AudioFile, lyrics: String) -> Result<(), CommandError> {
    let path = tags::uri_to_path(&file.uri);
    let io = tags::FsTagIo { path };
    tags::set_lyrics(&io, file.kind, &lyrics).map_err(CommandError::from)?;
    Ok(())
}

/// Remove lyrics from a single file.
#[tauri::command]
async fn clear_lyrics(file: AudioFile) -> Result<(), CommandError> {
    let path = tags::uri_to_path(&file.uri);
    let io = tags::FsTagIo { path };
    tags::clear_lyrics(&io, file.kind).map_err(CommandError::from)?;
    Ok(())
}

/// Read current lyrics from a file (used by the manual-edit modal).
#[tauri::command]
async fn read_lyrics(file: AudioFile) -> Result<Option<String>, CommandError> {
    let path = tags::uri_to_path(&file.uri);
    let io = tags::FsTagIo { path };
    tags::read_lyrics(&io, file.kind).map_err(CommandError::from)
}

/// Fetch lyrics for a given artist/title without writing them anywhere.
#[tauri::command]
async fn fetch_lyrics(
    artist: String,
    title: String,
    api_base: Option<String>,
) -> Result<String, CommandError> {
    let base = api_base.unwrap_or_else(|| "https://lrclib.net".to_string());
    let client = lyrics::LyricsClient::new(base);
    client
        .fetch(&artist, &title)
        .map_err(|e| CommandError { message: e.to_string() })
}

/// Start processing a queue. Returns once the queue is finished (cancelled or
/// completed). Progress is emitted on `processor://event`.
#[tauri::command]
async fn process_queue(
    app: AppHandle,
    cancel: State<'_, CancelState>,
    spec: QueueSpec,
) -> Result<Report, CommandError> {
    let cancel_flag: Arc<AtomicBool> = cancel.0.clone();
    let app2 = app.clone();
    let spec_clone = spec.clone();
    // Run on a blocking thread to keep the async runtime free.
    let report = tauri::async_runtime::spawn_blocking(move || {
        processor::process_queue(&app2, cancel_flag, spec_clone)
    })
    .await
    .map_err(|e| CommandError { message: e.to_string() })?;
    Ok(report)
}

/// Cancel the currently running queue (no-op if not running).
#[tauri::command]
async fn cancel_queue(cancel: State<'_, CancelState>) -> Result<(), CommandError> {
    cancel.cancel();
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = env_logger::try_init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(CancelState::default())
        .setup(|_app| {
            // Ensure window is shown properly on Android.
            #[cfg(target_os = "android")]
            {
                use tauri::Manager;
                if let Some(win) = _app.get_webview_window("main") {
                    let _ = win.show();
                }
            }
            let _ = _app;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            scan_directory,
            read_metadata,
            set_lyrics,
            clear_lyrics,
            read_lyrics,
            fetch_lyrics,
            process_queue,
            cancel_queue,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

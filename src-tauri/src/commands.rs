use crate::batch_processor::{remove_lyrics_batch, BatchProcessor};
use crate::error::AppError;
use crate::lyrics_fetcher::LyricsFetcher;
use crate::lyrics_writer::{remove_lyrics, write_lyrics};
use crate::metadata::{detect_format, read_track_info};
use crate::types::{
    AudioFormat, BatchResult, FileInfo, LyricsResult, ProcessingProgress, TrackInfo, TrackTask,
};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use tokio::sync::mpsc;
use walkdir::WalkDir;

// Global state for active batch processing sessions
type SessionMap = Arc<Mutex<HashMap<String, mpsc::UnboundedSender<()>>>>;

#[derive(Default)]
pub struct BatchState {
    sessions: SessionMap,
}

#[tauri::command]
pub async fn pick_files(app: AppHandle) -> Result<Vec<FileInfo>, String> {
    use tauri_plugin_dialog::DialogExt;

    let window = app.get_webview_window("main").ok_or_else(|| "No main window".to_string())?;
    
    // Use a channel to handle the callback-based API
    let (tx, rx) = tokio::sync::oneshot::channel();
    
    window
        .dialog()
        .file()
        .add_filter("Audio Files", &["mp3", "flac"])
        .pick_files(move |files| {
            let _ = tx.send(files);
        });

    let files = rx.await.map_err(|_| "Dialog cancelled".to_string())?
        .ok_or_else(|| "No files selected".to_string())?;

    let mut result = Vec::new();
    for path in files {
        let path_buf = path.into_path().unwrap();
        if let Some(name) = path_buf.file_name().and_then(|n| n.to_str()) {
            result.push(FileInfo {
                path: path_buf.to_string_lossy().to_string(),
                name: name.to_string(),
                is_directory: false,
                size: std::fs::metadata(&path_buf).ok().map(|m| m.len()),
            });
        }
    }
    Ok(result)
}

#[tauri::command]
pub async fn pick_directory(app: AppHandle) -> Result<Vec<FileInfo>, String> {
    use tauri_plugin_dialog::DialogExt;

    let window = app.get_webview_window("main").ok_or_else(|| "No main window".to_string())?;
    
    let (tx, rx) = tokio::sync::oneshot::channel();
    
    window
        .dialog()
        .file()
        .pick_folder(move |dir| {
            let _ = tx.send(dir);
        });

    let dir = rx.await.map_err(|_| "Dialog cancelled".to_string())?
        .ok_or_else(|| "No directory selected".to_string())?;

    let dir_path = dir.into_path().unwrap();
    let mut result = Vec::new();

    for entry in WalkDir::new(&dir_path).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let ext = ext.to_lowercase();
                if ext == "mp3" || ext == "flac" {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        result.push(FileInfo {
                            path: path.to_string_lossy().to_string(),
                            name: name.to_string(),
                            is_directory: false,
                            size: std::fs::metadata(path).ok().map(|m| m.len()),
                        });
                    }
                }
            }
        }
    }

    Ok(result)
}

#[tauri::command]
pub async fn read_metadata(path: String) -> Result<TrackInfo, String> {
    let path = Path::new(&path);
    read_track_info(path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn fetch_lyrics(artist: String, title: String) -> Result<LyricsResult, String> {
    let fetcher = LyricsFetcher::new();
    fetcher.fetch_lyrics(&artist, &title).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn write_lyrics_cmd(path: String, lyrics: String) -> Result<(), String> {
    let path = Path::new(&path);
    let format = detect_format(path).map_err(|e| e.to_string())?;
    write_lyrics(path, &lyrics, format).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_lyrics_cmd(path: String) -> Result<(), String> {
    let path = Path::new(&path);
    let format = detect_format(path).map_err(|e| e.to_string())?;
    remove_lyrics(path, format).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_batch_processing(
    app: AppHandle,
    state: State<'_, BatchState>,
    tasks: Vec<TrackTask>,
) -> Result<String, String> {
    let (progress_tx, mut progress_rx) = mpsc::unbounded_channel::<ProcessingProgress>();
    let (cancel_tx, cancel_rx) = mpsc::unbounded_channel::<()>();

    let mut processor = BatchProcessor::new(progress_tx, cancel_rx);
    let session_id = processor.session_id().to_string();

    // Store cancel sender
    {
        let mut sessions = state.sessions.lock().unwrap();
        sessions.insert(session_id.clone(), cancel_tx);
    }

    // Clone state for the spawned task
    let sessions = state.inner().sessions.clone();

    // Spawn progress forwarding task
    let app_handle = app.clone();
    let session_id_clone = session_id.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(progress) = progress_rx.recv().await {
            let _ = app_handle.emit(&format!("batch-progress-{}", session_id_clone), &progress);
        }
    });

    // Spawn processing task
    let app_handle = app.clone();
    let session_id_clone = session_id.clone();
    tauri::async_runtime::spawn(async move {
        let result = processor.process(tasks).await;

        // Clean up session
        {
            let mut sessions_guard = sessions.lock().unwrap();
            sessions_guard.remove(&session_id_clone);
        }

        // Emit completion event
        match result {
            Ok(batch_result) => {
                let _ = app_handle.emit(
                    &format!("batch-complete-{}", session_id_clone),
                    &batch_result,
                );
            }
            Err(e) => {
                let _ = app_handle.emit(
                    &format!("batch-error-{}", session_id_clone),
                    &e.to_string(),
                );
            }
        }
    });

    Ok(session_id)
}

#[tauri::command]
pub async fn cancel_batch_processing(state: State<'_, BatchState>, session_id: String) -> Result<(), String> {
    let mut sessions = state.sessions.lock().unwrap();
    if let Some(cancel_tx) = sessions.remove(&session_id) {
        let _ = cancel_tx.send(());
    }
    Ok(())
}

#[tauri::command]
pub async fn remove_lyrics_batch_cmd(tasks: Vec<TrackTask>) -> Result<BatchResult, String> {
    remove_lyrics_batch(tasks).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn scan_directory_for_audio(path: String) -> Result<Vec<FileInfo>, String> {
    let dir_path = Path::new(&path);
    let mut result = Vec::new();

    for entry in WalkDir::new(dir_path).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let ext = ext.to_lowercase();
                if ext == "mp3" || ext == "flac" {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        result.push(FileInfo {
                            path: path.to_string_lossy().to_string(),
                            name: name.to_string(),
                            is_directory: false,
                            size: std::fs::metadata(path).ok().map(|m| m.len()),
                        });
                    }
                }
            }
        }
    }

    Ok(result)
}
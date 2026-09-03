mod batch_processor;
mod commands;
mod error;
mod lyrics_fetcher;
mod lyrics_writer;
mod metadata;
mod types;

use commands::*;
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(crate::commands::BatchState::default())
        .invoke_handler(tauri::generate_handler![
            pick_files,
            pick_directory,
            read_metadata,
            fetch_lyrics,
            write_lyrics_cmd,
            remove_lyrics_cmd,
            start_batch_processing,
            cancel_batch_processing,
            remove_lyrics_batch_cmd,
            scan_directory_for_audio,
        ])
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                use tracing_subscriber::{EnvFilter, FmtSubscriber};
                let subscriber = FmtSubscriber::builder()
                    .with_env_filter(EnvFilter::from_default_env())
                    .finish();
                tracing::subscriber::set_global_default(subscriber).ok();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

mod commands;
mod persist;
mod state;
mod worker;

use deepkick_core::{lyrics, tags};

#[cfg(test)]
mod tests;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
	tauri::Builder::default()
		.plugin(
			tauri_plugin_log::Builder::new()
				.level(log::LevelFilter::Info)
				.build(),
		)
		.plugin(tauri_plugin_opener::init())
		.plugin(tauri_plugin_deepkick::init())
		.manage(state::AppState::default())
		.setup(|app| {
			persist::load(app.handle());
			Ok(())
		})
		.invoke_handler(tauri::generate_handler![
			commands::queue_state,
			commands::queue_add_files,
			commands::queue_add_directory,
			commands::queue_remove,
			commands::queue_clear,
			commands::queue_start,
			commands::queue_stop,
			commands::report_get,
			commands::report_clear,
			commands::file_info,
			commands::file_set_lyrics,
			commands::lyrics_search,
			commands::log_message,
			commands::launch_action,
			commands::insets,
			commands::notifications_request,
		])
		.run(tauri::generate_context!())
		.expect("error while running tauri application");
}

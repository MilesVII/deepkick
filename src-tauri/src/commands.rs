use std::sync::atomic::Ordering;

use tauri::{async_runtime::spawn_blocking, AppHandle, Manager, Runtime, State};
use tauri_plugin_deepkick::{DeepkickExt, Insets, PickedFile};

use crate::lyrics::{self, Candidate};
use crate::persist;
use crate::state::*;
use crate::tags::{self, Info};
use crate::worker;

fn snapshot(state: &AppState) -> QueueState {
	state.queue.lock().unwrap().clone()
}

fn is_running(state: &AppState) -> bool {
	state
		.queue
		.lock()
		.unwrap()
		.run
		.as_ref()
		.map(|r| r.active)
		.unwrap_or(false)
}

fn add_items<R: Runtime>(app: &AppHandle<R>, picked: Vec<PickedFile>) -> QueueState {
	let state = app.state::<AppState>();
	{
		let mut queue = state.queue.lock().unwrap();
		for file in picked {
			if queue.items.iter().any(|i| i.uri == file.uri) {
				continue;
			}
			let id = state.next_id.fetch_add(1, Ordering::SeqCst) + 1;
			queue.items.push(Item {
				id,
				uri: file.uri,
				name: file.name,
				folder: file.folder,
				size: file.size,
				status: Status::Pending,
				artist: None,
				title: None,
				detail: None,
			});
		}
	}
	persist::save(app);
	worker::emit_update(app);
	snapshot(&state)
}

#[tauri::command]
pub async fn queue_state(state: State<'_, AppState>) -> Result<QueueState, String> {
	Ok(snapshot(&state))
}

#[tauri::command]
pub async fn queue_add_files<R: Runtime>(app: AppHandle<R>) -> Result<QueueState, String> {
	let handle = app.clone();
	let picked = spawn_blocking(move || handle.deepkick().pick_files())
		.await
		.map_err(|e| e.to_string())?
		.map_err(|e| e.to_string())?;
	Ok(add_items(&app, picked))
}

#[tauri::command]
pub async fn queue_add_directory<R: Runtime>(app: AppHandle<R>) -> Result<QueueState, String> {
	let handle = app.clone();
	let picked = spawn_blocking(move || handle.deepkick().pick_directory())
		.await
		.map_err(|e| e.to_string())?
		.map_err(|e| e.to_string())?;
	Ok(add_items(&app, picked))
}

#[tauri::command]
pub async fn queue_remove<R: Runtime>(
	app: AppHandle<R>,
	state: State<'_, AppState>,
	ids: Vec<u32>,
) -> Result<QueueState, String> {
	if is_running(&state) {
		return Err("Cannot edit the queue while it is running".into());
	}
	{
		let mut queue = state.queue.lock().unwrap();
		queue.items.retain(|i| !ids.contains(&i.id));
	}
	persist::save(&app);
	worker::emit_update(&app);
	Ok(snapshot(&state))
}

#[tauri::command]
pub async fn queue_clear<R: Runtime>(
	app: AppHandle<R>,
	state: State<'_, AppState>,
) -> Result<QueueState, String> {
	if is_running(&state) {
		return Err("Cannot edit the queue while it is running".into());
	}
	{
		let mut queue = state.queue.lock().unwrap();
		queue.items.clear();
		queue.run = None;
	}
	persist::save(&app);
	worker::emit_update(&app);
	Ok(snapshot(&state))
}

#[tauri::command]
pub async fn queue_start<R: Runtime>(
	app: AppHandle<R>,
	mode: Mode,
	options: RunOptions,
) -> Result<(), String> {
	// Ask for the notification permission first so the progress notification is visible
	let handle = app.clone();
	let _ = spawn_blocking(move || handle.deepkick().request_notifications()).await;
	worker::start(app, mode, options)
}

#[tauri::command]
pub async fn queue_stop(state: State<'_, AppState>) -> Result<(), String> {
	state.cancel.store(true, Ordering::SeqCst);
	Ok(())
}

#[tauri::command]
pub async fn report_get(state: State<'_, AppState>) -> Result<Option<Report>, String> {
	Ok(state.report.lock().unwrap().clone())
}

#[tauri::command]
pub async fn report_clear<R: Runtime>(
	app: AppHandle<R>,
	state: State<'_, AppState>,
) -> Result<(), String> {
	*state.report.lock().unwrap() = None;
	persist::save(&app);
	Ok(())
}

#[tauri::command]
pub async fn file_info<R: Runtime>(app: AppHandle<R>, uri: String) -> Result<Info, String> {
	spawn_blocking(move || {
		let mut file = app
			.deepkick()
			.open_file(&uri, false)
			.map_err(|e| format!("cannot open: {e}"))?;
		let kind = tags::probe(&mut file)?.ok_or("Not an MP3 or FLAC file")?;
		tags::read(&mut file, kind)
	})
	.await
	.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn file_set_lyrics<R: Runtime>(
	app: AppHandle<R>,
	uri: String,
	lyrics: Option<String>,
	fix_encoding: Option<bool>,
) -> Result<(), String> {
	let handle = app.clone();
	let target = uri.clone();
	let lyrics = lyrics
		.map(|l| l.replace("\r\n", "\n").trim().to_string())
		.filter(|l| !l.is_empty());
	let removing = lyrics.is_none();
	spawn_blocking(move || {
		let mut file = handle
			.deepkick()
			.open_file(&target, true)
			.map_err(|e| format!("cannot open: {e}"))?;
		let kind = tags::probe(&mut file)?.ok_or("Not an MP3 or FLAC file")?;
		tags::write_lyrics(&mut file, kind, lyrics.as_deref(), fix_encoding.unwrap_or(true)).map(|_| ())
	})
	.await
	.map_err(|e| e.to_string())??;

	// Reflect the manual edit in the queue and the report
	let state = app.state::<AppState>();
	let status = if removing { Status::Removed } else { Status::Done };
	{
		let mut queue = state.queue.lock().unwrap();
		if let Some(item) = queue.items.iter_mut().find(|i| i.uri == uri) {
			item.status = status;
			item.detail = Some("edited manually".into());
		}
	}
	if let Some(report) = state.report.lock().unwrap().as_mut() {
		if let Some(item) = report.items.iter_mut().find(|i| i.uri == uri) {
			let old = item.status;
			item.status = status;
			item.detail = Some("edited manually".into());
			*report.summary.entry(old).or_insert(1) -= 1;
			*report.summary.entry(status).or_insert(0) += 1;
		}
	}
	persist::save(&app);
	worker::emit_update(&app);
	Ok(())
}

#[tauri::command]
pub async fn lyrics_search(
	artist: String,
	title: String,
	album: Option<String>,
	duration: Option<f64>,
) -> Result<Vec<Candidate>, String> {
	spawn_blocking(move || {
		lyrics::Client::new().search(&artist, &title, album.as_deref(), duration)
	})
	.await
	.map_err(|e| e.to_string())?
}

/// Lets the page forward uncaught errors to the native log (logcat on Android).
#[tauri::command]
pub async fn log_message(level: String, message: String) -> Result<(), String> {
	match level.as_str() {
		"error" => log::error!(target: "webview", "{message}"),
		"warn" => log::warn!(target: "webview", "{message}"),
		_ => log::info!(target: "webview", "{message}"),
	}
	Ok(())
}

#[tauri::command]
pub async fn launch_action<R: Runtime>(app: AppHandle<R>) -> Result<Option<String>, String> {
	spawn_blocking(move || app.deepkick().take_launch_action())
		.await
		.map_err(|e| e.to_string())?
		.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn insets<R: Runtime>(app: AppHandle<R>) -> Result<Insets, String> {
	spawn_blocking(move || app.deepkick().insets())
		.await
		.map_err(|e| e.to_string())?
		.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn notifications_request<R: Runtime>(app: AppHandle<R>) -> Result<bool, String> {
	spawn_blocking(move || app.deepkick().request_notifications())
		.await
		.map_err(|e| e.to_string())?
		.map_err(|e| e.to_string())
}

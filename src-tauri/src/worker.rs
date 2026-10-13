//! The queue runner: a plain thread that walks the queue one file at a time while the
//! Android foreground service keeps the process alive.

use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_deepkick::{DeepkickExt, DoneNotification, ProgressNotification};

use deepkick_core::{lyrics, process_file, Outcome};

use crate::persist;
use crate::state::*;

pub const EVENT_UPDATE: &str = "queue-update";
pub const EVENT_FINISHED: &str = "queue-finished";

pub fn emit_update<R: Runtime>(app: &AppHandle<R>) {
	let state = app.state::<AppState>();
	let snapshot = state.queue.lock().unwrap().clone();
	let _ = app.emit(EVENT_UPDATE, snapshot);
}

pub fn start<R: Runtime>(app: AppHandle<R>, mode: Mode, options: RunOptions) -> Result<(), String> {
	let state = app.state::<AppState>();
	{
		let mut queue = state.queue.lock().unwrap();
		if queue.run.as_ref().map(|r| r.active).unwrap_or(false) {
			return Err("A run is already in progress".into());
		}
		if queue.items.is_empty() {
			return Err("The queue is empty".into());
		}
		for item in queue.items.iter_mut() {
			item.status = Status::Pending;
			item.detail = None;
		}
		queue.run = Some(Run {
			mode,
			active: true,
			cancelled: false,
			started_at: now(),
			finished_at: None,
			current: 0,
			total: queue.items.len(),
		});
		state.cancel.store(false, std::sync::atomic::Ordering::SeqCst);
	}
	emit_update(&app);

	std::thread::Builder::new()
		.name("deepkick-worker".into())
		.spawn(move || run(app, mode, options))
		.map_err(|e| e.to_string())?;
	Ok(())
}

fn mode_title(mode: Mode) -> &'static str {
	match mode {
		Mode::Fetch => "Fetching lyrics",
		Mode::Remove => "Removing lyrics",
	}
}

fn run<R: Runtime>(app: AppHandle<R>, mode: Mode, options: RunOptions) {
	let state = app.state::<AppState>();
	let bridge = app.deepkick();
	let total = state.queue.lock().unwrap().items.len();

	if let Err(e) = bridge.service_start(&ProgressNotification {
		title: mode_title(mode).into(),
		text: format!("0 / {total}"),
		max: total as u32,
		progress: 0,
	}) {
		log::warn!("foreground service could not be started: {e}");
	}

	let client = lyrics::Client::new();
	let mut index = 0usize;
	let mut cancelled = false;

	loop {
		if state.cancel.load(std::sync::atomic::Ordering::SeqCst) {
			cancelled = true;
			break;
		}
		let item = {
			let mut queue = state.queue.lock().unwrap();
			if index >= queue.items.len() {
				break;
			}
			queue.items[index].status = Status::Processing;
			if let Some(run) = queue.run.as_mut() {
				run.current = index;
			}
			queue.items[index].clone()
		};
		emit_update(&app);

		let outcome = match bridge.open_file(&item.uri, true) {
			Ok(mut file) => process_file(&mut file, mode, &options, &client),
			Err(e) => Outcome::error(format!("cannot open: {e}")),
		};
		log::info!("[{}] {} -> {:?} {}", index + 1, item.name, outcome.status, outcome.detail.clone().unwrap_or_default());

		{
			let mut queue = state.queue.lock().unwrap();
			if let Some(entry) = queue.items.iter_mut().find(|i| i.id == item.id) {
				entry.status = outcome.status;
				entry.detail = outcome.detail;
				if outcome.artist.is_some() {
					entry.artist = outcome.artist;
				}
				if outcome.title.is_some() {
					entry.title = outcome.title;
				}
			}
		}
		emit_update(&app);

		let _ = bridge.service_update(&ProgressNotification {
			title: mode_title(mode).into(),
			text: format!("{} / {total} · {}", index + 1, item.name),
			max: total as u32,
			progress: (index + 1) as u32,
		});

		index += 1;
		if mode == Mode::Fetch {
			// be gentle with the public API
			std::thread::sleep(Duration::from_millis(120));
		}
	}

	let report = {
		let mut queue = state.queue.lock().unwrap();
		if let Some(run) = queue.run.as_mut() {
			run.active = false;
			run.cancelled = cancelled;
			run.finished_at = Some(now());
			run.current = index.min(total);
		}
		Report::from_queue(&queue)
	};

	let summary_text = report
		.as_ref()
		.map(|r| summarize(r))
		.unwrap_or_default();
	if let Some(report) = report {
		*state.report.lock().unwrap() = Some(report);
	}
	persist::save(&app);

	let done_title = match (mode, cancelled) {
		(Mode::Fetch, false) => "Lyrics fetched",
		(Mode::Fetch, true) => "Fetching stopped",
		(Mode::Remove, false) => "Lyrics removed",
		(Mode::Remove, true) => "Removal stopped",
	};
	let _ = bridge.service_stop(Some(DoneNotification {
		title: done_title.into(),
		text: summary_text,
	}));

	emit_update(&app);
	let _ = app.emit(EVENT_FINISHED, ());
}

pub fn summarize(report: &Report) -> String {
	deepkick_core::summary_text(report.run.mode, &report.summary)
}

//! The queue runner: a plain thread that walks the queue one file at a time while the
//! Android foreground service keeps the process alive.

use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_deepkick::{DeepkickExt, DoneNotification, ProgressNotification};

use crate::lyrics::{self, Found};
use crate::persist;
use crate::state::*;
use crate::tags;

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

		let outcome = process(&app, &client, &item, mode, &options);
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
	let count = |s: Status| report.summary.get(&s).copied().unwrap_or(0);
	let mut parts: Vec<String> = vec![];
	let mut push = |n: usize, label: &str| {
		if n > 0 {
			parts.push(format!("{n} {label}"));
		}
	};
	match report.run.mode {
		Mode::Fetch => {
			push(count(Status::Done), "embedded");
			push(count(Status::Existing), "already had lyrics");
			push(count(Status::NotFound), "not found");
			push(count(Status::Instrumental), "instrumental");
			push(count(Status::NoTag), "untagged");
		},
		Mode::Remove => {
			push(count(Status::Removed), "cleaned");
			push(count(Status::Empty), "had no lyrics");
		},
	}
	push(count(Status::Unsupported), "unsupported");
	push(count(Status::Error), "failed");
	push(count(Status::Pending), "skipped");
	if parts.is_empty() {
		"Nothing to do".into()
	} else {
		parts.join(", ")
	}
}

struct Outcome {
	status: Status,
	detail: Option<String>,
	artist: Option<String>,
	title: Option<String>,
}

impl Outcome {
	fn new(status: Status) -> Self {
		Outcome { status, detail: None, artist: None, title: None }
	}
	fn error(message: impl Into<String>) -> Self {
		Outcome { status: Status::Error, detail: Some(message.into()), artist: None, title: None }
	}
}

fn process<R: Runtime>(
	app: &AppHandle<R>,
	client: &lyrics::Client,
	item: &Item,
	mode: Mode,
	options: &RunOptions,
) -> Outcome {
	let bridge = app.deepkick();
	let mut file = match bridge.open_file(&item.uri, true) {
		Ok(f) => f,
		Err(e) => return Outcome::error(format!("cannot open: {e}")),
	};
	let kind = match tags::probe(&mut file) {
		Ok(Some(kind)) => kind,
		Ok(None) => return Outcome::new(Status::Unsupported),
		Err(e) => return Outcome::error(format!("cannot read: {e}")),
	};
	let info = match tags::read(&mut file, kind) {
		Ok(info) => info,
		Err(e) => return Outcome::error(format!("cannot read tags: {e}")),
	};

	let mut outcome = match mode {
		Mode::Fetch => {
			if info.lyrics.is_some() && !options.overwrite {
				Outcome::new(Status::Existing)
			} else {
				match (&info.artist, &info.title) {
					(Some(artist), Some(title)) => match client.find(
						artist,
						title,
						info.album.as_deref(),
						info.duration_secs,
						options.prefer_synced,
						options.use_fallback,
					) {
						Ok(Some(Found::Instrumental)) => Outcome::new(Status::Instrumental),
						Ok(Some(Found::Lyrics { text, source })) => {
							match tags::write_lyrics(&mut file, kind, Some(&text)) {
								Ok(()) => Outcome {
									status: Status::Done,
									detail: Some(source),
									artist: None,
									title: None,
								},
								Err(e) => Outcome::error(format!("cannot write: {e}")),
							}
						},
						Ok(None) => Outcome::new(Status::NotFound),
						Err(e) => Outcome::error(e),
					},
					_ => Outcome::new(Status::NoTag),
				}
			}
		},
		Mode::Remove => {
			if info.lyrics.is_none() {
				Outcome::new(Status::Empty)
			} else {
				match tags::write_lyrics(&mut file, kind, None) {
					Ok(()) => Outcome::new(Status::Removed),
					Err(e) => Outcome::error(format!("cannot write: {e}")),
				}
			}
		},
	};
	outcome.artist = info.artist;
	outcome.title = info.title;
	outcome
}

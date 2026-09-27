//! Keeps the queue and the last report on disk so they survive process death
//! (e.g. when the completion notification is tapped hours later).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

use crate::state::*;

#[derive(Serialize, Deserialize, Default)]
struct Snapshot {
	items: Vec<Item>,
	report: Option<Report>,
	next_id: u32,
}

fn path<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
	let dir = app.path().app_data_dir().ok()?;
	std::fs::create_dir_all(&dir).ok()?;
	Some(dir.join("deepkick-state.json"))
}

pub fn save<R: Runtime>(app: &AppHandle<R>) {
	let Some(path) = path(app) else { return };
	let state = app.state::<AppState>();
	let snapshot = Snapshot {
		items: state.queue.lock().unwrap().items.clone(),
		report: state.report.lock().unwrap().clone(),
		next_id: state.next_id.load(std::sync::atomic::Ordering::SeqCst),
	};
	match serde_json::to_vec(&snapshot) {
		Ok(bytes) => {
			if let Err(e) = std::fs::write(&path, bytes) {
				log::warn!("cannot persist state: {e}");
			}
		},
		Err(e) => log::warn!("cannot serialize state: {e}"),
	}
}

pub fn load<R: Runtime>(app: &AppHandle<R>) {
	let Some(path) = path(app) else { return };
	let Ok(bytes) = std::fs::read(&path) else { return };
	let Ok(snapshot) = serde_json::from_slice::<Snapshot>(&bytes) else {
		log::warn!("ignoring unreadable state file");
		return;
	};
	let state = app.state::<AppState>();
	let mut queue = state.queue.lock().unwrap();
	queue.items = snapshot.items;
	for item in queue.items.iter_mut() {
		if item.status == Status::Processing {
			item.status = Status::Pending;
		}
	}
	queue.run = snapshot.report.as_ref().map(|r| Run { active: false, ..r.run.clone() });
	*state.report.lock().unwrap() = snapshot.report;
	state
		.next_id
		.store(snapshot.next_id, std::sync::atomic::Ordering::SeqCst);
}

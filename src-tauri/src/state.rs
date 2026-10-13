use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

pub use deepkick_core::{Mode, RunOptions, Status};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
	pub id: u32,
	pub uri: String,
	pub name: String,
	#[serde(default)]
	pub folder: String,
	#[serde(default)]
	pub size: u64,
	pub status: Status,
	#[serde(default)]
	pub artist: Option<String>,
	#[serde(default)]
	pub title: Option<String>,
	/// Extra information for the report: error text, provider, ...
	#[serde(default)]
	pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
	pub mode: Mode,
	pub active: bool,
	pub cancelled: bool,
	pub started_at: u64,
	pub finished_at: Option<u64>,
	/// Index of the entry being processed
	pub current: usize,
	pub total: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueState {
	pub items: Vec<Item>,
	pub run: Option<Run>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
	pub run: Run,
	pub items: Vec<Item>,
	pub summary: BTreeMap<Status, usize>,
}

impl Report {
	pub fn from_queue(queue: &QueueState) -> Option<Report> {
		let run = queue.run.clone()?;
		let mut summary = BTreeMap::new();
		for item in &queue.items {
			*summary.entry(item.status).or_insert(0) += 1;
		}
		Some(Report { run, items: queue.items.clone(), summary })
	}
}

#[derive(Default)]
pub struct AppState {
	pub queue: Mutex<QueueState>,
	pub report: Mutex<Option<Report>>,
	pub cancel: AtomicBool,
	pub next_id: AtomicU32,
}

pub fn now() -> u64 {
	std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.map(|d| d.as_millis() as u64)
		.unwrap_or(0)
}

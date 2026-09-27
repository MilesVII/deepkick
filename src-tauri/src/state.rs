use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// Outcome of processing one queue entry (or its pending/processing state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
	Pending,
	Processing,
	/// Fetch mode: lyrics were found and written into the file
	Done,
	/// Fetch mode: no provider knew the song
	NotFound,
	/// Fetch mode: the file has no artist/title to search with
	NoTag,
	/// Fetch mode: the file already had lyrics and overwriting is off
	Existing,
	/// Fetch mode: the track is marked as instrumental by the provider
	Instrumental,
	/// Remove mode: lyrics were removed
	Removed,
	/// Remove mode: there were no lyrics to remove
	Empty,
	/// Not an MP3 or FLAC file
	Unsupported,
	Error,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
	Fetch,
	Remove,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RunOptions {
	/// Replace lyrics that are already embedded
	pub overwrite: bool,
	/// Prefer synchronised (LRC) lyrics when the provider has them
	pub prefer_synced: bool,
	/// Try the secondary provider when LRCLIB has nothing
	pub use_fallback: bool,
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

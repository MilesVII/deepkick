//! Queue entries and the background runner that processes them one by one.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use deepkick_core::lyrics::Client;
use deepkick_core::{process_file, Mode, Outcome, RunOptions, Status};

pub struct Entry {
	pub path: PathBuf,
	pub status: Status,
	pub artist: Option<String>,
	pub title: Option<String>,
	pub detail: Option<String>,
}

impl Entry {
	pub fn new(path: PathBuf) -> Self {
		Entry { path, status: Status::Pending, artist: None, title: None, detail: None }
	}

	pub fn name(&self) -> String {
		self.path
			.file_name()
			.map(|n| n.to_string_lossy().into_owned())
			.unwrap_or_else(|| self.path.display().to_string())
	}

	/// The last two folders above the file, e.g. `Sybreed/Slave Design`.
	pub fn folder(&self) -> String {
		let Some(parent) = self.path.parent() else {
			return String::new();
		};
		let parts: Vec<String> = parent
			.components()
			.rev()
			.take(2)
			.map(|c| c.as_os_str().to_string_lossy().trim_end_matches(['\\', '/']).to_string())
			.filter(|s| !s.is_empty())
			.collect();
		parts.into_iter().rev().collect::<Vec<_>>().join("/")
	}

	pub fn track(&self) -> String {
		match (&self.artist, &self.title) {
			(Some(a), Some(t)) => format!("{a} — {t}"),
			(Some(x), None) | (None, Some(x)) => x.clone(),
			(None, None) => String::new(),
		}
	}

	pub fn apply(&mut self, outcome: Outcome) {
		self.status = outcome.status;
		self.detail = outcome.detail;
		if outcome.artist.is_some() {
			self.artist = outcome.artist;
		}
		if outcome.title.is_some() {
			self.title = outcome.title;
		}
	}
}

pub enum Event {
	Started(usize),
	Finished(usize, Outcome),
	Done { cancelled: bool },
}

/// Opens a file for reading and writing and fetches or removes its lyrics.
pub fn process_path(path: &Path, mode: Mode, options: &RunOptions, client: &Client) -> Outcome {
	match File::options().read(true).write(true).open(path) {
		Ok(mut file) => process_file(&mut file, mode, options, client),
		Err(e) => Outcome::error(format!("cannot open: {e}")),
	}
}

/// A run over a list of queue entries, processed on a worker thread. Cancelling lets the
/// current file finish, so a file is never left half-written.
pub struct Runner {
	pub mode: Mode,
	/// Queue indices this run covers
	pub jobs: Vec<usize>,
	pub completed: usize,
	pub current: Option<usize>,
	pub started: Instant,
	cancel: Arc<AtomicBool>,
	rx: Receiver<Event>,
	handle: Option<JoinHandle<()>>,
}

impl Runner {
	pub fn start(jobs: Vec<(usize, PathBuf)>, mode: Mode, options: RunOptions) -> Runner {
		let cancel = Arc::new(AtomicBool::new(false));
		let (tx, rx) = mpsc::channel();
		let indices = jobs.iter().map(|(i, _)| *i).collect();
		let flag = cancel.clone();
		let handle = std::thread::spawn(move || {
			let client = Client::new();
			let total = jobs.len();
			for (n, (index, path)) in jobs.into_iter().enumerate() {
				if flag.load(Ordering::SeqCst) {
					let _ = tx.send(Event::Done { cancelled: true });
					return;
				}
				let _ = tx.send(Event::Started(index));
				let outcome = process_path(&path, mode, &options, &client);
				let _ = tx.send(Event::Finished(index, outcome));
				if mode == Mode::Fetch && n + 1 < total {
					// be gentle with the public API
					std::thread::sleep(Duration::from_millis(120));
				}
			}
			let _ = tx.send(Event::Done { cancelled: false });
		});
		Runner {
			mode,
			jobs: indices,
			completed: 0,
			current: None,
			started: Instant::now(),
			cancel,
			rx,
			handle: Some(handle),
		}
	}

	pub fn cancel(&self) {
		self.cancel.store(true, Ordering::SeqCst);
	}

	pub fn cancel_flag(&self) -> Arc<AtomicBool> {
		self.cancel.clone()
	}

	pub fn is_cancelling(&self) -> bool {
		self.cancel.load(Ordering::SeqCst)
	}

	pub fn total(&self) -> usize {
		self.jobs.len()
	}

	pub fn try_next(&self) -> Option<Event> {
		match self.rx.try_recv() {
			Ok(event) => Some(event),
			Err(TryRecvError::Empty) => None,
			Err(TryRecvError::Disconnected) => Some(Event::Done { cancelled: self.is_cancelling() }),
		}
	}

	pub fn next_blocking(&self) -> Event {
		self.rx
			.recv()
			.unwrap_or(Event::Done { cancelled: self.is_cancelling() })
	}

	/// Waits for the worker thread (after the current file is done).
	pub fn join(mut self) {
		if let Some(handle) = self.handle.take() {
			let _ = handle.join();
		}
	}
}

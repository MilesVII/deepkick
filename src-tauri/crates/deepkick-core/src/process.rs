use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::lyrics::{Client, Found};
use crate::tags::{self, LyricsChange};

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

impl Status {
	/// Short human-readable label, the same wording the app uses.
	pub fn label(self) -> &'static str {
		match self {
			Status::Pending => "pending",
			Status::Processing => "working",
			Status::Done => "embedded",
			Status::NotFound => "not found",
			Status::NoTag => "no tag",
			Status::Existing => "has lyrics",
			Status::Instrumental => "instrumental",
			Status::Removed => "removed",
			Status::Empty => "no lyrics",
			Status::Unsupported => "unsupported",
			Status::Error => "error",
		}
	}

	pub const ALL: [Status; 11] = [
		Status::Done,
		Status::Removed,
		Status::Existing,
		Status::NotFound,
		Status::Instrumental,
		Status::NoTag,
		Status::Empty,
		Status::Unsupported,
		Status::Error,
		Status::Processing,
		Status::Pending,
	];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
	Fetch,
	Remove,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RunOptions {
	/// Replace lyrics that are already embedded
	pub overwrite: bool,
	/// Prefer synchronised (LRC) lyrics when the provider has them
	pub prefer_synced: bool,
	/// Try the secondary provider when LRCLIB has nothing
	pub use_fallback: bool,
	/// Rewrite tags whose text was stored in the wrong character set (CP1251 or UTF-8
	/// bytes labelled Latin-1), even when no lyrics are written
	pub fix_encoding: bool,
}

impl Default for RunOptions {
	fn default() -> Self {
		RunOptions { overwrite: false, prefer_synced: false, use_fallback: false, fix_encoding: true }
	}
}

/// Result of processing one file.
#[derive(Debug, Clone)]
pub struct Outcome {
	pub status: Status,
	/// Extra information for the report: error text, the provider that matched, ...
	pub detail: Option<String>,
	pub artist: Option<String>,
	pub title: Option<String>,
}

impl Outcome {
	pub fn new(status: Status) -> Self {
		Outcome { status, detail: None, artist: None, title: None }
	}

	pub fn error(message: impl Into<String>) -> Self {
		Outcome { status: Status::Error, detail: Some(message.into()), artist: None, title: None }
	}
}

/// `true` for file names DeepKick handles (`.mp3`, `.flac`), by extension.
pub fn is_supported_name(name: &str) -> bool {
	Path::new(name)
		.extension()
		.and_then(|e| e.to_str())
		.map(|e| e.eq_ignore_ascii_case("mp3") || e.eq_ignore_ascii_case("flac"))
		.unwrap_or(false)
}

/// Fetches or removes the lyrics of one already opened (read-write) file.
pub fn process_file(file: &mut File, mode: Mode, options: &RunOptions, client: &Client) -> Outcome {
	let kind = match tags::probe(file) {
		Ok(Some(kind)) => kind,
		Ok(None) => return Outcome::new(Status::Unsupported),
		Err(e) => return Outcome::error(format!("cannot read: {e}")),
	};
	let info = match tags::read(file, kind) {
		Ok(info) => info,
		Err(e) => return Outcome::error(format!("cannot read tags: {e}")),
	};

	// Decide first, write once: the lyrics change and the encoding repair go into the same
	// rewrite of the file.
	let (mut outcome, change) = match mode {
		Mode::Fetch => {
			if info.lyrics.is_some() && !options.overwrite {
				(Outcome::new(Status::Existing), LyricsChange::Keep)
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
						Ok(Some(Found::Instrumental)) => {
							(Outcome::new(Status::Instrumental), LyricsChange::Keep)
						},
						Ok(Some(Found::Lyrics { text, source })) => (
							Outcome { status: Status::Done, detail: Some(source), artist: None, title: None },
							LyricsChange::Set(text),
						),
						Ok(None) => (Outcome::new(Status::NotFound), LyricsChange::Keep),
						Err(e) => return Outcome::error(e),
					},
					_ => (Outcome::new(Status::NoTag), LyricsChange::Keep),
				}
			}
		},
		Mode::Remove => {
			if info.lyrics.is_none() {
				(Outcome::new(Status::Empty), LyricsChange::Keep)
			} else {
				(Outcome::new(Status::Removed), LyricsChange::Remove)
			}
		},
	};

	let repair = options.fix_encoding && info.repaired.is_some();
	if change != LyricsChange::Keep || repair {
		match tags::write(file, kind, change, options.fix_encoding) {
			Ok(report) => {
				if report.repaired_fields > 0 {
					let how = info.repaired.map(|r| r.label()).unwrap_or("Latin-1");
					let n = report.repaired_fields;
					let note = format!("{n} tag field{} repaired from {how}", if n == 1 { "" } else { "s" });
					outcome.detail = Some(match outcome.detail.take() {
						Some(d) => format!("{d} · {note}"),
						None => note,
					});
				}
			},
			Err(e) => return Outcome::error(format!("cannot write: {e}")),
		}
	}
	outcome.artist = info.artist;
	outcome.title = info.title;
	outcome
}

/// One-line summary of a finished run, e.g. "11 embedded, 1 not found".
pub fn summary_text(mode: Mode, counts: &BTreeMap<Status, usize>) -> String {
	let count = |s: Status| counts.get(&s).copied().unwrap_or(0);
	let mut parts: Vec<String> = vec![];
	let mut push = |n: usize, label: &str| {
		if n > 0 {
			parts.push(format!("{n} {label}"));
		}
	};
	match mode {
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

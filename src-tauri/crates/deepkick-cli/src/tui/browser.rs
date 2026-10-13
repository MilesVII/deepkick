//! A small file browser for picking files and folders.

use std::path::{Path, PathBuf};

use deepkick_core::is_supported_name;
use ratatui::widgets::ListState;

use super::input::Input;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
	Parent,
	Drive,
	Dir,
	File,
}

pub struct BEntry {
	pub name: String,
	pub path: PathBuf,
	pub kind: Kind,
}

pub struct Browser {
	/// `None` lists the drives (Windows only)
	pub dir: Option<PathBuf>,
	pub entries: Vec<BEntry>,
	pub state: ListState,
	pub show_hidden: bool,
	pub audio_count: usize,
	pub error: Option<String>,
	/// "Go to path" field
	pub goto: Option<Input>,
}

fn drives() -> Vec<BEntry> {
	(b'C'..=b'Z')
		.map(|c| format!("{}:\\", c as char))
		.filter(|root| Path::new(root).exists())
		.map(|root| BEntry { name: root.clone(), path: PathBuf::from(root), kind: Kind::Drive })
		.collect()
}

impl Browser {
	pub fn open(start: Option<&Path>) -> Browser {
		let dir = start
			.filter(|p| p.is_dir())
			.map(Path::to_path_buf)
			.or_else(|| std::env::current_dir().ok())
			.or_else(dirs::home_dir);
		let mut browser = Browser {
			dir,
			entries: vec![],
			state: ListState::default(),
			show_hidden: false,
			audio_count: 0,
			error: None,
			goto: None,
		};
		browser.load(None);
		browser
	}

	pub fn title(&self) -> String {
		match &self.dir {
			Some(dir) => dir.display().to_string(),
			None => "Drives".into(),
		}
	}

	/// Re-reads the current folder and selects `select` (or the first entry).
	pub fn load(&mut self, select: Option<&Path>) {
		self.error = None;
		self.entries.clear();
		self.audio_count = 0;
		match self.dir.clone() {
			None => self.entries = drives(),
			Some(dir) => {
				if dir.parent().is_some() || cfg!(windows) {
					self.entries.push(BEntry {
						name: "..".into(),
						path: dir.parent().map(Path::to_path_buf).unwrap_or_default(),
						kind: Kind::Parent,
					});
				}
				match std::fs::read_dir(&dir) {
					Ok(read) => {
						let mut dirs = vec![];
						let mut files = vec![];
						for entry in read.filter_map(|e| e.ok()) {
							let name = entry.file_name().to_string_lossy().into_owned();
							if name.starts_with('.') && !self.show_hidden {
								continue;
							}
							let path = entry.path();
							// follow symlinks so linked folders can be browsed
							let is_dir = std::fs::metadata(&path).map(|m| m.is_dir()).unwrap_or(false);
							if is_dir {
								dirs.push(BEntry { name, path, kind: Kind::Dir });
							} else if is_supported_name(&name) {
								files.push(BEntry { name, path, kind: Kind::File });
							}
						}
						dirs.sort_by_key(|e| e.name.to_lowercase());
						files.sort_by_key(|e| e.name.to_lowercase());
						self.audio_count = files.len();
						self.entries.extend(dirs);
						self.entries.extend(files);
					},
					Err(e) => self.error = Some(e.to_string()),
				}
			},
		}
		let index = select
			.and_then(|p| self.entries.iter().position(|e| e.path == p))
			.or_else(|| {
				// land on the first real entry rather than ".."
				let first = usize::from(self.entries.first().map(|e| e.kind == Kind::Parent).unwrap_or(false));
				(first < self.entries.len()).then_some(first).or(Some(0))
			});
		self.state.select(if self.entries.is_empty() { None } else { index });
	}

	pub fn selected(&self) -> Option<&BEntry> {
		self.state.selected().and_then(|i| self.entries.get(i))
	}

	pub fn move_by(&mut self, delta: isize) {
		if self.entries.is_empty() {
			return;
		}
		let current = self.state.selected().unwrap_or(0) as isize;
		let next = (current + delta).clamp(0, self.entries.len() as isize - 1);
		self.state.select(Some(next as usize));
	}

	pub fn select_edge(&mut self, end: bool) {
		if !self.entries.is_empty() {
			self.state.select(Some(if end { self.entries.len() - 1 } else { 0 }));
		}
	}

	pub fn up(&mut self) {
		let Some(dir) = self.dir.clone() else {
			return;
		};
		match dir.parent() {
			Some(parent) => {
				self.dir = Some(parent.to_path_buf());
				self.load(Some(&dir));
			},
			None if cfg!(windows) => {
				self.dir = None;
				self.load(Some(&dir));
			},
			None => {},
		}
	}

	/// Opens the selected folder; returns the selected file instead when it is a file.
	pub fn enter(&mut self) -> Option<PathBuf> {
		let (kind, path) = {
			let entry = self.selected()?;
			(entry.kind, entry.path.clone())
		};
		match kind {
			Kind::Parent => self.up(),
			Kind::Dir | Kind::Drive => {
				self.dir = Some(path);
				self.load(None);
			},
			Kind::File => return Some(path),
		}
		None
	}

	pub fn go(&mut self, path: &Path) -> Result<(), String> {
		let path = crate::scan::expand_home(path);
		let path = std::path::absolute(&path).unwrap_or(path);
		if path.is_dir() {
			self.dir = Some(path);
			self.load(None);
			Ok(())
		} else if path.is_file() {
			self.dir = path.parent().map(Path::to_path_buf);
			self.load(Some(&path));
			Ok(())
		} else {
			Err(format!("{} does not exist", path.display()))
		}
	}
}

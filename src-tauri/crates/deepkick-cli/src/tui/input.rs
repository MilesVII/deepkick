//! A single-line text input with cursor movement and path completion.

use std::path::{Path, PathBuf};

use deepkick_core::is_supported_name;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::scan::expand_home;

pub enum InputResult {
	Submit(String),
	Cancel,
	Continue,
}

#[derive(Default)]
pub struct Input {
	pub chars: Vec<char>,
	pub cursor: usize,
	/// Completion candidates or other feedback shown under the field
	pub hint: Option<String>,
	/// Complete paths on Tab
	pub paths: bool,
}

impl Input {
	pub fn new(text: &str, paths: bool) -> Self {
		let chars: Vec<char> = text.chars().collect();
		let cursor = chars.len();
		Input { chars, cursor, hint: None, paths }
	}

	pub fn text(&self) -> String {
		self.chars.iter().collect()
	}

	pub fn insert_str(&mut self, text: &str) {
		for c in text.chars().filter(|c| !c.is_control()) {
			self.chars.insert(self.cursor, c);
			self.cursor += 1;
		}
	}

	pub fn handle(&mut self, key: KeyEvent) -> InputResult {
		let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
		match key.code {
			KeyCode::Enter => return InputResult::Submit(self.text()),
			KeyCode::Esc => return InputResult::Cancel,
			KeyCode::Char('u') if ctrl => {
				self.chars.drain(..self.cursor);
				self.cursor = 0;
			},
			KeyCode::Char('w') if ctrl => {
				let mut start = self.cursor;
				while start > 0 && self.chars[start - 1] == ' ' {
					start -= 1;
				}
				while start > 0 && !matches!(self.chars[start - 1], ' ' | '/' | '\\') {
					start -= 1;
				}
				self.chars.drain(start..self.cursor);
				self.cursor = start;
			},
			KeyCode::Char(c) if !ctrl => {
				self.chars.insert(self.cursor, c);
				self.cursor += 1;
			},
			KeyCode::Backspace if self.cursor > 0 => {
				self.cursor -= 1;
				self.chars.remove(self.cursor);
			},
			KeyCode::Delete if self.cursor < self.chars.len() => {
				self.chars.remove(self.cursor);
			},
			KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
			KeyCode::Right => self.cursor = (self.cursor + 1).min(self.chars.len()),
			KeyCode::Home => self.cursor = 0,
			KeyCode::End => self.cursor = self.chars.len(),
			KeyCode::Tab if self.paths => self.complete(),
			_ => {},
		}
		if key.code != KeyCode::Tab {
			self.hint = None;
		}
		InputResult::Continue
	}

	/// Completes the path being typed: the whole text when it has no quotes, or the part
	/// after an unclosed quote.
	fn complete(&mut self) {
		let text = self.text();
		let quotes = text.matches('"').count();
		let start = if quotes % 2 == 1 {
			text.rfind('"').map(|i| i + 1).unwrap_or(0)
		} else if quotes == 0 {
			0
		} else {
			self.hint = Some("put the path in quotes to complete it".into());
			return;
		};
		let typed = &text[start..];
		let (completed, alternatives) = complete_path(typed);
		if let Some(completed) = completed {
			let new_text = format!("{}{}", &text[..start], completed);
			self.chars = new_text.chars().collect();
			self.cursor = self.chars.len();
		}
		self.hint = match alternatives.len() {
			0 => Some("no matches".into()),
			1 => None,
			n => {
				let mut shown: Vec<String> = alternatives.into_iter().take(8).collect();
				if n > 8 {
					shown.push(format!("… {} more", n - 8));
				}
				Some(shown.join("   "))
			},
		};
	}
}

fn separator_index(text: &str) -> Option<usize> {
	if cfg!(windows) {
		text.rfind(['/', '\\'])
	} else {
		text.rfind('/')
	}
}

/// Returns the extended text (if anything could be added) and the matching names.
pub fn complete_path(typed: &str) -> (Option<String>, Vec<String>) {
	let (dir_part, name_part) = match separator_index(typed) {
		Some(i) => (&typed[..=i], &typed[i + 1..]),
		None => ("", typed),
	};
	let dir: PathBuf = if dir_part.is_empty() {
		PathBuf::from(".")
	} else {
		expand_home(Path::new(dir_part))
	};
	let Ok(read) = std::fs::read_dir(&dir) else {
		return (None, vec![]);
	};
	let fold = |s: &str| if cfg!(windows) { s.to_lowercase() } else { s.to_string() };
	let wanted = fold(name_part);
	let mut matches: Vec<(String, bool)> = read
		.filter_map(|e| e.ok())
		.filter_map(|e| {
			let name = e.file_name().to_string_lossy().into_owned();
			let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
			let relevant = is_dir || is_supported_name(&name);
			let visible = !name.starts_with('.') || name_part.starts_with('.');
			(relevant && visible && fold(&name).starts_with(&wanted)).then_some((name, is_dir))
		})
		.collect();
	matches.sort_by_key(|(name, _)| name.to_lowercase());
	let names: Vec<String> = matches
		.iter()
		.map(|(n, d)| if *d { format!("{n}/") } else { n.clone() })
		.collect();
	let sep = if cfg!(windows) && dir_part.contains('\\') { '\\' } else { '/' };
	match matches.as_slice() {
		[] => (None, names),
		[(name, is_dir)] => {
			let mut out = format!("{dir_part}{name}");
			if *is_dir {
				out.push(sep);
			}
			(Some(out), names)
		},
		many => {
			let first = &many[0].0;
			let mut common = first.len();
			for (name, _) in &many[1..] {
				common = common.min(
					first
						.char_indices()
						.zip(name.chars())
						.take_while(|((_, a), b)| fold(&a.to_string()) == fold(&b.to_string()))
						.last()
						.map(|((i, a), _)| i + a.len_utf8())
						.unwrap_or(0),
				);
			}
			let prefix = &first[..common];
			let extended = (prefix.len() > name_part.len()).then(|| format!("{dir_part}{prefix}"));
			(extended, names)
		},
	}
}

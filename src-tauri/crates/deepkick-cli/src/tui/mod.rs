//! The interactive interface.

mod browser;
mod input;
mod ui;

use std::collections::{BTreeMap, HashSet};
use std::io::{stdout, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use anyhow::Result;
use deepkick_core::lyrics::{Candidate, Client};
use deepkick_core::{summary_text, tags, Mode, Status};
use ratatui::crossterm::event::{
	self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
	KeyModifiers,
};
use ratatui::crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::crossterm::{cursor, execute};
use ratatui::widgets::{ListState, TableState};
use ratatui::DefaultTerminal;

use crate::batch;
use crate::config::Config;
use crate::editor;
use crate::queue::{Entry, Event as RunEvent, Runner};
use crate::scan;
use browser::{Browser, Kind as BrowserKind};
use input::{Input, InputResult};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Level {
	Info,
	Ok,
	Warn,
	Error,
}

pub struct Message {
	pub text: String,
	pub level: Level,
	pub at: Instant,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InputPurpose {
	AddPaths,
	Editor,
}

#[derive(Clone, Copy)]
pub enum Confirm {
	RemoveAll,
	ClearQueue,
	StopRun,
	Quit,
	RemoveLyrics(usize),
}

pub struct Search {
	pub index: usize,
	pub query: String,
	pub candidates: Vec<Candidate>,
	pub state: ListState,
	/// Use synchronised lyrics instead of plain ones
	pub synced: bool,
}

pub struct View {
	pub index: usize,
	pub title: String,
	pub text: Option<String>,
	pub scroll: u16,
}

pub struct Summary {
	pub mode: Mode,
	pub text: String,
	pub cancelled: bool,
	pub elapsed: Duration,
}

impl Summary {
	pub fn title(&self) -> &'static str {
		match (self.mode, self.cancelled) {
			(Mode::Fetch, false) => "Lyrics fetched",
			(Mode::Fetch, true) => "Fetching stopped",
			(Mode::Remove, false) => "Lyrics removed",
			(Mode::Remove, true) => "Removal stopped",
		}
	}
}

/// What a key press in command mode turned out to be.
enum Burst {
	/// Dropped or pasted text (paths)
	Paste(String),
	/// Ordinary key presses, in order
	Keys(Vec<KeyEvent>),
}

pub enum Popup {
	None,
	Help,
	Input(InputPurpose, Input),
	Browser(Browser),
	Confirm(Confirm, String),
	Searching(String),
	Search(Search),
	View(View),
	Config(ListState),
	Summary,
}

pub struct App {
	pub cfg: Config,
	pub entries: Vec<Entry>,
	pub table: TableState,
	pub filter: Option<Status>,
	pub runner: Option<Runner>,
	pub last_run: Option<Summary>,
	pub popup: Popup,
	pub message: Option<Message>,
	search_rx: Option<Receiver<(usize, String, Result<Vec<Candidate>, String>)>>,
	/// Editing must happen outside of drawing, so it is queued for the event loop.
	pending_edit: Option<(usize, Option<String>)>,
	quit: bool,
}

/// Runs the interface; returns the summary of the last run to print after leaving it.
pub fn run(cfg: Config, paths: Vec<PathBuf>) -> Result<Option<String>> {
	let mut app = App::new(cfg);
	if !paths.is_empty() {
		app.add_paths(&paths);
	}
	let mut terminal = ratatui::try_init()?;
	let _ = execute!(stdout(), EnableBracketedPaste);
	let result = app.event_loop(&mut terminal);
	let _ = execute!(stdout(), DisableBracketedPaste);
	ratatui::restore();

	if let Some(runner) = app.runner.take() {
		runner.cancel();
		println!("Stopping after the current file...");
		runner.join();
	}
	let _ = app.cfg.save();
	result?;
	Ok(app.last_run.map(|s| format!("{}: {}", s.title().to_lowercase(), s.text)))
}

impl App {
	pub fn new(cfg: Config) -> App {
		App {
			cfg,
			entries: vec![],
			table: TableState::default(),
			filter: None,
			runner: None,
			last_run: None,
			popup: Popup::None,
			message: None,
			search_rx: None,
			pending_edit: None,
			quit: false,
		}
	}

	fn event_loop(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
		loop {
			self.tick();
			terminal.draw(|frame| ui::draw(frame, self))?;
			if self.quit {
				return Ok(());
			}
			if let Some((index, text)) = self.pending_edit.take() {
				self.edit_entry(terminal, index, text)?;
				continue;
			}
			if !event::poll(Duration::from_millis(100))? {
				continue;
			}
			match event::read()? {
				Event::Key(key) if key.kind == KeyEventKind::Press => match self.read_burst(key)? {
					Burst::Paste(text) => self.on_paste(&text),
					Burst::Keys(keys) => {
						for key in keys {
							self.on_key(key);
						}
					},
				},
				Event::Paste(text) => self.on_paste(&text),
				_ => {},
			}
		}
	}

	/// Dragging files onto a console without bracketed paste (conhost, and some terminals)
	/// types the path as a burst of key presses. In command mode a printable key therefore
	/// waits briefly for followers: nobody types faster than the gap, so a burst that forms
	/// existing paths is a drop and is added to the queue instead of being run as commands.
	fn read_burst(&mut self, first: KeyEvent) -> Result<Burst> {
		const GAP: Duration = Duration::from_millis(40);
		let commands_mode = match &self.popup {
			Popup::None => true,
			Popup::Browser(browser) => browser.goto.is_none(),
			_ => false,
		};
		let plain = |k: &KeyEvent| !k.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
		if !commands_mode || !matches!(first.code, KeyCode::Char(_)) || !plain(&first) {
			return Ok(Burst::Keys(vec![first]));
		}
		let mut keys = vec![first];
		let mut text = String::new();
		if let KeyCode::Char(c) = first.code {
			text.push(c);
		}
		while event::poll(GAP)? {
			match event::read()? {
				Event::Key(k) if k.kind == KeyEventKind::Press => match k.code {
					KeyCode::Char(c) if plain(&k) => {
						text.push(c);
						keys.push(k);
					},
					// a drop often ends with Enter; a human pressing another key ends the burst
					KeyCode::Enter => break,
					_ => {
						keys.push(k);
						break;
					},
				},
				Event::Paste(pasted) => {
					text.push_str(&pasted);
					keys.clear();
				},
				_ => {},
			}
		}
		if text.chars().count() >= 3 {
			let paths = scan::parse_path_list(&text);
			if !paths.is_empty() && paths.iter().all(|p| scan::expand_home(p).exists()) {
				return Ok(Burst::Paste(text));
			}
		}
		Ok(Burst::Keys(keys))
	}

	pub fn say(&mut self, level: Level, text: impl Into<String>) {
		self.message = Some(Message { text: text.into(), level, at: Instant::now() });
	}

	pub fn running(&self) -> bool {
		self.runner.is_some()
	}

	/// Queue indices shown with the current filter.
	pub fn visible(&self) -> Vec<usize> {
		(0..self.entries.len())
			.filter(|&i| self.filter.is_none_or(|f| self.entries[i].status == f))
			.collect()
	}

	pub fn selected_index(&self) -> Option<usize> {
		let visible = self.visible();
		self.table.selected().and_then(|row| visible.get(row).copied())
	}

	fn fix_selection(&mut self) {
		let count = self.visible().len();
		match self.table.selected() {
			_ if count == 0 => self.table.select(None),
			None => self.table.select(Some(0)),
			Some(row) if row >= count => self.table.select(Some(count - 1)),
			_ => {},
		}
	}

	pub fn counts(&self, indices: impl IntoIterator<Item = usize>) -> BTreeMap<Status, usize> {
		let mut counts = BTreeMap::new();
		for i in indices {
			*counts.entry(self.entries[i].status).or_insert(0) += 1;
		}
		counts
	}

	fn tick(&mut self) {
		if self.message.as_ref().is_some_and(|m| m.at.elapsed() > Duration::from_secs(6)) {
			self.message = None;
		}
		self.drain_runner();
		self.drain_search();
	}

	fn drain_runner(&mut self) {
		let mut finished = None;
		if let Some(runner) = self.runner.as_mut() {
			while let Some(event) = runner.try_next() {
				match event {
					RunEvent::Started(i) => {
						runner.current = Some(i);
						self.entries[i].status = Status::Processing;
					},
					RunEvent::Finished(i, outcome) => {
						runner.completed += 1;
						self.entries[i].apply(outcome);
					},
					RunEvent::Done { cancelled } => {
						finished = Some(cancelled);
						break;
					},
				}
			}
		}
		let Some(cancelled) = finished else {
			return;
		};
		let runner = self.runner.take().expect("runner exists while draining");
		let counts = self.counts(runner.jobs.iter().copied());
		let summary = Summary {
			mode: runner.mode,
			text: summary_text(runner.mode, &counts),
			cancelled,
			elapsed: runner.started.elapsed(),
		};
		runner.join();
		self.last_run = Some(summary);
		// ring the bell: terminals flash the taskbar or show a notification
		print!("\x07");
		let _ = stdout().flush();
		if matches!(self.popup, Popup::None | Popup::Confirm(Confirm::StopRun, _)) {
			self.popup = Popup::Summary;
		}
	}

	fn drain_search(&mut self) {
		let Some(rx) = &self.search_rx else {
			return;
		};
		let (index, query, result) = match rx.try_recv() {
			Ok(found) => found,
			Err(TryRecvError::Empty) => return,
			Err(TryRecvError::Disconnected) => {
				self.search_rx = None;
				return;
			},
		};
		self.search_rx = None;
		if !matches!(self.popup, Popup::Searching(_)) {
			return;
		}
		match result {
			Ok(candidates) if candidates.is_empty() => {
				self.popup = Popup::None;
				self.say(Level::Warn, format!("Nothing found for {query}"));
			},
			Ok(candidates) => {
				let mut state = ListState::default();
				state.select(Some(0));
				let synced = self.cfg.prefer_synced;
				self.popup = Popup::Search(Search { index, query, candidates, state, synced });
			},
			Err(e) => {
				self.popup = Popup::None;
				self.say(Level::Error, format!("Search failed: {e}"));
			},
		}
	}

	// ---- actions ------------------------------------------------------------------------

	pub fn add_paths(&mut self, paths: &[PathBuf]) -> usize {
		let scan = scan::collect(paths);
		let known: HashSet<PathBuf> = self.entries.iter().map(|e| e.path.clone()).collect();
		let mut added = 0;
		for file in scan.files {
			if !known.contains(&file) {
				self.entries.push(Entry::new(file));
				added += 1;
			}
		}
		self.fix_selection();
		match (added, scan.warnings.first()) {
			(0, Some(warning)) => self.say(Level::Warn, warning.clone()),
			(0, None) => self.say(Level::Info, "Nothing new to add"),
			(n, None) => self.say(Level::Ok, format!("Added {n} file{}", if n == 1 { "" } else { "s" })),
			(n, Some(_)) => self.say(
				Level::Ok,
				format!("Added {n} file{}, {} skipped", if n == 1 { "" } else { "s" }, scan.warnings.len()),
			),
		}
		added
	}

	fn start(&mut self, mode: Mode) {
		if self.running() {
			self.say(Level::Warn, "A run is already in progress");
			return;
		}
		let jobs = self.visible();
		if jobs.is_empty() {
			self.say(Level::Warn, "The queue is empty: press a to browse or p to type a path");
			return;
		}
		let filtered = self.filter.is_some();
		for &i in &jobs {
			self.entries[i].status = Status::Pending;
			self.entries[i].detail = None;
		}
		self.filter = None;
		self.fix_selection();
		let work = jobs.iter().map(|&i| (i, self.entries[i].path.clone())).collect();
		self.runner = Some(Runner::start(work, mode, self.cfg.run_options()));
		let verb = if mode == Mode::Fetch { "Fetching lyrics for" } else { "Removing lyrics from" };
		let scope = if filtered { " (the filtered ones)" } else { "" };
		self.say(Level::Info, format!("{verb} {} files{scope}", jobs.len()));
	}

	fn cycle_filter(&mut self, forward: bool) {
		let present: Vec<Status> = Status::ALL
			.into_iter()
			.filter(|s| self.entries.iter().any(|e| e.status == *s))
			.collect();
		if present.is_empty() {
			return;
		}
		let position = self.filter.and_then(|f| present.iter().position(|s| *s == f));
		self.filter = match (position, forward) {
			(None, true) => Some(present[0]),
			(None, false) => present.last().copied(),
			(Some(i), true) => present.get(i + 1).copied(),
			(Some(0), false) => None,
			(Some(i), false) => Some(present[i - 1]),
		};
		self.table.select(Some(0));
		self.fix_selection();
	}

	fn read_info(&self, index: usize) -> Result<tags::Info, String> {
		let path = &self.entries[index].path;
		let mut file = std::fs::File::open(path).map_err(|e| format!("cannot open: {e}"))?;
		let kind = tags::probe(&mut file)?.ok_or("not an MP3 or FLAC file")?;
		tags::read(&mut file, kind)
	}

	fn write(&mut self, index: usize, lyrics: Option<&str>) {
		match batch::write_lyrics(&self.entries[index].path, lyrics, self.cfg.fix_encoding) {
			Ok(()) => {
				let entry = &mut self.entries[index];
				entry.status = if lyrics.is_some() { Status::Done } else { Status::Removed };
				entry.detail = Some("edited manually".into());
				let name = entry.name();
				let text = match lyrics {
					Some(l) => format!("Saved {} lines into {name}", l.lines().count()),
					None => format!("Removed the lyrics from {name}"),
				};
				self.say(Level::Ok, text);
			},
			Err(e) => self.say(Level::Error, format!("{e:#}")),
		}
	}

	fn search(&mut self, index: usize) {
		let info = match self.read_info(index) {
			Ok(info) => info,
			Err(e) => return self.say(Level::Error, e),
		};
		let (Some(artist), Some(title)) = (info.artist.clone(), info.title.clone()) else {
			return self.say(Level::Warn, "This file has no artist and title tags to search with");
		};
		{
			let entry = &mut self.entries[index];
			entry.artist = info.artist.clone();
			entry.title = info.title.clone();
		}
		let query = format!("{artist} — {title}");
		let (tx, rx) = mpsc::channel();
		let label = query.clone();
		std::thread::spawn(move || {
			let result =
				Client::new().search(&artist, &title, info.album.as_deref(), info.duration_secs);
			let _ = tx.send((index, label, result));
		});
		self.search_rx = Some(rx);
		self.popup = Popup::Searching(query);
	}

	fn view(&mut self, index: usize) {
		match self.read_info(index) {
			Ok(info) => {
				let entry = &mut self.entries[index];
				entry.artist = info.artist.clone().or(entry.artist.take());
				entry.title = info.title.clone().or(entry.title.take());
				let title = match entry.track() {
					t if t.is_empty() => entry.name(),
					t => t,
				};
				self.popup = Popup::View(View { index, title, text: info.lyrics, scroll: 0 });
			},
			Err(e) => self.say(Level::Error, e),
		}
	}

	/// Suspends the interface, opens the lyrics (or `text`) in the external editor and
	/// writes the result back.
	fn edit_entry(
		&mut self,
		terminal: &mut DefaultTerminal,
		index: usize,
		text: Option<String>,
	) -> Result<()> {
		let info = match self.read_info(index) {
			Ok(info) => info,
			Err(e) => {
				self.say(Level::Error, e);
				return Ok(());
			},
		};
		let current = info.lyrics.clone().unwrap_or_default();
		let initial = text.clone().unwrap_or_else(|| current.clone());
		let entry = &mut self.entries[index];
		entry.artist = info.artist.clone().or(entry.artist.take());
		entry.title = info.title.clone().or(entry.title.take());
		let header = editor::header_for(&entry.name(), &entry.track());
		let (command, _) = editor::resolve(self.cfg.editor.as_deref());

		let _ = execute!(stdout(), DisableBracketedPaste, LeaveAlternateScreen, cursor::Show);
		let _ = terminal::disable_raw_mode();
		println!("Waiting for {command} to close...");
		let edited = editor::edit(&command, &header, &initial);
		terminal::enable_raw_mode()?;
		execute!(stdout(), EnterAlternateScreen, EnableBracketedPaste)?;
		terminal.clear()?;
		// drop keys pressed while the editor had the terminal
		while event::poll(Duration::ZERO)? {
			let _ = event::read()?;
		}

		match edited {
			Err(e) => self.say(Level::Error, format!("{e:#}")),
			Ok(edited) if edited.text == current.trim() => {
				if edited.quick && text.is_none() {
					self.say(
						Level::Warn,
						"No changes. If your editor returns immediately, add its wait flag (code --wait)",
					);
				} else {
					self.say(Level::Info, "No changes");
				}
			},
			Ok(edited) if edited.text.is_empty() => {
				self.popup = Popup::Confirm(
					Confirm::RemoveLyrics(index),
					"The text is empty. Remove the lyrics from the file?".into(),
				);
			},
			Ok(edited) => self.write(index, Some(&edited.text)),
		}
		Ok(())
	}

	fn confirm(&mut self, what: Confirm) {
		match what {
			Confirm::RemoveAll => self.start(Mode::Remove),
			Confirm::ClearQueue => {
				self.entries.clear();
				self.filter = None;
				self.fix_selection();
				self.say(Level::Info, "Queue cleared");
			},
			Confirm::StopRun => {
				if let Some(runner) = &self.runner {
					runner.cancel();
					self.say(Level::Info, "Stopping after the current file...");
				}
			},
			Confirm::Quit => {
				if let Some(runner) = &self.runner {
					runner.cancel();
				}
				self.quit = true;
			},
			Confirm::RemoveLyrics(index) => self.write(index, None),
		}
	}

	// ---- input --------------------------------------------------------------------------

	fn on_paste(&mut self, text: &str) {
		match &mut self.popup {
			Popup::Input(_, input) => input.insert_str(&text.replace(['\r', '\n'], " ")),
			Popup::Browser(browser) => {
				if let Some(goto) = &mut browser.goto {
					goto.insert_str(text.trim());
				} else {
					let paths = scan::parse_path_list(text);
					self.add_paths(&paths);
				}
			},
			Popup::None => {
				let paths = scan::parse_path_list(text);
				if !paths.is_empty() {
					self.add_paths(&paths);
				}
			},
			_ => {},
		}
	}

	fn on_key(&mut self, key: KeyEvent) {
		if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
			if let Some(runner) = &self.runner {
				runner.cancel();
			}
			self.quit = true;
			return;
		}
		match std::mem::replace(&mut self.popup, Popup::None) {
			Popup::None => self.on_main_key(key),
			Popup::Help | Popup::Summary => {},
			Popup::Searching(query) => {
				if key.code == KeyCode::Esc {
					self.search_rx = None;
				} else {
					self.popup = Popup::Searching(query);
				}
			},
			Popup::Confirm(what, text) => match key.code {
				KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => self.confirm(what),
				KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {},
				_ => self.popup = Popup::Confirm(what, text),
			},
			Popup::Input(purpose, mut input) => match input.handle(key) {
				InputResult::Continue => self.popup = Popup::Input(purpose, input),
				InputResult::Cancel => {
					if purpose == InputPurpose::Editor {
						self.open_config(4);
					}
				},
				InputResult::Submit(text) => self.submit_input(purpose, &text),
			},
			Popup::Browser(browser) => self.on_browser_key(browser, key),
			Popup::Search(search) => self.on_search_key(search, key),
			Popup::View(view) => self.on_view_key(view, key),
			Popup::Config(state) => self.on_config_key(state, key),
		}
	}

	fn on_main_key(&mut self, key: KeyEvent) {
		let selected = self.selected_index();
		let rows = self.visible().len();
		let row = self.table.selected().unwrap_or(0);
		match key.code {
			KeyCode::Char('q') | KeyCode::Esc => {
				if self.running() {
					let (what, question) = if key.code == KeyCode::Esc {
						(Confirm::StopRun, "Stop the run after the current file?")
					} else {
						(Confirm::Quit, "A run is in progress. Stop after the current file and quit?")
					};
					self.popup = Popup::Confirm(what, question.into());
				} else if key.code == KeyCode::Esc && self.filter.is_some() {
					self.filter = None;
					self.fix_selection();
				} else if key.code == KeyCode::Char('q') {
					self.quit = true;
				}
			},
			KeyCode::Up | KeyCode::Char('k') => self.table.select(Some(row.saturating_sub(1))),
			KeyCode::Down | KeyCode::Char('j') if rows > 0 => {
				self.table.select(Some((row + 1).min(rows - 1)));
			},
			KeyCode::PageUp => self.table.select(Some(row.saturating_sub(10))),
			KeyCode::PageDown if rows > 0 => self.table.select(Some((row + 10).min(rows - 1))),
			KeyCode::Home | KeyCode::Char('g') => self.table.select(Some(0)),
			KeyCode::End | KeyCode::Char('G') if rows > 0 => self.table.select(Some(rows - 1)),
			KeyCode::Tab => self.cycle_filter(true),
			KeyCode::BackTab => self.cycle_filter(false),
			KeyCode::Char('a') => {
				self.popup = Popup::Browser(Browser::open(self.cfg.last_dir.as_deref()));
			},
			KeyCode::Char('p') | KeyCode::Char('i') => {
				self.popup = Popup::Input(InputPurpose::AddPaths, Input::new("", true));
			},
			KeyCode::Char('f') => self.start(Mode::Fetch),
			KeyCode::Char('r') if !self.running() => {
				let count = rows;
				if count > 0 {
					let scope = if self.filter.is_some() { " (filtered)" } else { "" };
					self.popup = Popup::Confirm(
						Confirm::RemoveAll,
						format!("Remove embedded lyrics from {count} file{}{scope}?", if count == 1 { "" } else { "s" }),
					);
				}
			},
			KeyCode::Char('X') if !self.running() && !self.entries.is_empty() => {
				self.popup = Popup::Confirm(
					Confirm::ClearQueue,
					format!("Remove all {} files from the queue? The files stay untouched.", self.entries.len()),
				);
			},
			KeyCode::Char('x') | KeyCode::Delete => match selected {
				_ if self.running() => self.say(Level::Warn, "Wait for the run to finish"),
				Some(i) => {
					self.entries.remove(i);
					self.fix_selection();
				},
				None => {},
			},
			KeyCode::Char('e') | KeyCode::Enter => match selected {
				Some(i) if self.runner.as_ref().is_some_and(|r| r.jobs.contains(&i) && self.entries[i].status == Status::Processing) => {
					self.say(Level::Warn, "This file is being processed right now");
				},
				Some(i) => self.pending_edit = Some((i, None)),
				None => {},
			},
			KeyCode::Char('s') => {
				if let Some(i) = selected {
					self.search(i);
				}
			},
			KeyCode::Char('v') => {
				if let Some(i) = selected {
					self.view(i);
				}
			},
			KeyCode::Char('c') => self.open_config(0),
			KeyCode::Char('?') | KeyCode::F(1) => self.popup = Popup::Help,
			KeyCode::Char('r') => self.say(Level::Warn, "Wait for the run to finish"),
			_ => {},
		}
	}

	fn submit_input(&mut self, purpose: InputPurpose, text: &str) {
		match purpose {
			InputPurpose::AddPaths => {
				let paths = scan::parse_path_list(text);
				if !paths.is_empty() {
					self.add_paths(&paths);
				}
			},
			InputPurpose::Editor => {
				let _ = self.cfg.set("editor", text);
				self.save_config();
				self.open_config(4);
			},
		}
	}

	fn on_browser_key(&mut self, mut browser: Browser, key: KeyEvent) {
		if let Some(mut goto) = browser.goto.take() {
			match goto.handle(key) {
				InputResult::Continue => browser.goto = Some(goto),
				InputResult::Cancel => {},
				InputResult::Submit(text) => {
					if let Err(e) = browser.go(std::path::Path::new(text.trim().trim_matches('"'))) {
						browser.error = Some(e);
					}
				},
			}
			self.popup = Popup::Browser(browser);
			return;
		}
		match key.code {
			KeyCode::Esc | KeyCode::Char('q') => {
				self.cfg.last_dir = browser.dir.clone();
				self.save_config();
				return;
			},
			KeyCode::Up | KeyCode::Char('k') => browser.move_by(-1),
			KeyCode::Down | KeyCode::Char('j') => browser.move_by(1),
			KeyCode::PageUp => browser.move_by(-10),
			KeyCode::PageDown => browser.move_by(10),
			KeyCode::Home => browser.select_edge(false),
			KeyCode::End => browser.select_edge(true),
			KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
				if let Some(file) = browser.enter() {
					self.add_paths(&[file]);
					browser.move_by(1);
				}
			},
			KeyCode::Backspace | KeyCode::Left | KeyCode::Char('h') => browser.up(),
			KeyCode::Char(' ') | KeyCode::Insert => {
				if let Some(entry) = browser.selected() {
					if matches!(entry.kind, BrowserKind::Dir | BrowserKind::File | BrowserKind::Drive) {
						let path = entry.path.clone();
						self.add_paths(&[path]);
					}
				}
				browser.move_by(1);
			},
			KeyCode::Char('a') => {
				if let Some(dir) = browser.dir.clone() {
					self.add_paths(&[dir]);
				}
			},
			KeyCode::Char('g') | KeyCode::Char('/') => {
				let start = browser.dir.as_ref().map(|d| d.display().to_string()).unwrap_or_default();
				let mut goto = Input::new(&start, true);
				if !start.is_empty() && !start.ends_with(['/', '\\']) {
					goto.insert_str(std::path::MAIN_SEPARATOR_STR);
				}
				browser.goto = Some(goto);
			},
			KeyCode::Char('~') => {
				if let Some(home) = dirs::home_dir() {
					let _ = browser.go(&home);
				}
			},
			KeyCode::Char('.') => {
				browser.show_hidden = !browser.show_hidden;
				let keep = browser.selected().map(|e| e.path.clone());
				browser.load(keep.as_deref());
			},
			_ => {},
		}
		self.popup = Popup::Browser(browser);
	}

	fn candidate_text(search: &Search) -> Option<String> {
		let c = search.candidates.get(search.state.selected()?)?;
		if search.synced {
			c.synced.clone().or_else(|| c.plain.clone())
		} else {
			c.plain.clone().or_else(|| c.synced.clone())
		}
	}

	fn on_search_key(&mut self, mut search: Search, key: KeyEvent) {
		let count = search.candidates.len();
		let row = search.state.selected().unwrap_or(0);
		match key.code {
			KeyCode::Esc | KeyCode::Char('q') => return,
			KeyCode::Up | KeyCode::Char('k') => search.state.select(Some(row.saturating_sub(1))),
			KeyCode::Down | KeyCode::Char('j') => search.state.select(Some((row + 1).min(count - 1))),
			KeyCode::Tab => search.synced = !search.synced,
			KeyCode::Enter | KeyCode::Char('w') => match Self::candidate_text(&search) {
				Some(text) => return self.write(search.index, Some(&text)),
				None => self.say(Level::Warn, "This match is instrumental, there is nothing to write"),
			},
			KeyCode::Char('e') => match Self::candidate_text(&search) {
				Some(text) => {
					self.pending_edit = Some((search.index, Some(text)));
					return;
				},
				None => self.say(Level::Warn, "This match is instrumental, there is nothing to edit"),
			},
			_ => {},
		}
		self.popup = Popup::Search(search);
	}

	fn on_view_key(&mut self, mut view: View, key: KeyEvent) {
		match key.code {
			KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('v') => return,
			KeyCode::Up | KeyCode::Char('k') => view.scroll = view.scroll.saturating_sub(1),
			KeyCode::Down | KeyCode::Char('j') => view.scroll = view.scroll.saturating_add(1),
			KeyCode::PageUp => view.scroll = view.scroll.saturating_sub(10),
			KeyCode::PageDown | KeyCode::Char(' ') => view.scroll = view.scroll.saturating_add(10),
			KeyCode::Home => view.scroll = 0,
			KeyCode::Char('e') => {
				self.pending_edit = Some((view.index, None));
				return;
			},
			KeyCode::Char('s') => return self.search(view.index),
			_ => {},
		}
		self.popup = Popup::View(view);
	}

	fn open_config(&mut self, row: usize) {
		let mut state = ListState::default();
		state.select(Some(row));
		self.popup = Popup::Config(state);
	}

	fn save_config(&mut self) {
		if let Err(e) = self.cfg.save() {
			self.say(Level::Error, format!("{e:#}"));
		}
	}

	fn on_config_key(&mut self, mut state: ListState, key: KeyEvent) {
		let row = state.selected().unwrap_or(0);
		match key.code {
			KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('c') => return,
			KeyCode::Up | KeyCode::Char('k') => state.select(Some(row.saturating_sub(1))),
			KeyCode::Down | KeyCode::Char('j') => state.select(Some((row + 1).min(4))),
			KeyCode::Enter | KeyCode::Char(' ') => {
				match row {
					0 => self.cfg.overwrite = !self.cfg.overwrite,
					1 => self.cfg.prefer_synced = !self.cfg.prefer_synced,
					2 => self.cfg.fallback = !self.cfg.fallback,
					3 => self.cfg.fix_encoding = !self.cfg.fix_encoding,
					_ => {
						let current = self.cfg.editor.clone().unwrap_or_default();
						self.popup = Popup::Input(InputPurpose::Editor, Input::new(&current, false));
						return;
					},
				}
				self.save_config();
			},
			_ => {},
		}
		self.popup = Popup::Config(state);
	}
}

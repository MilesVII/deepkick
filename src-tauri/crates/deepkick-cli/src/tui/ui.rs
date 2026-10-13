//! Drawing the interface.

use deepkick_core::Status;
use ratatui::layout::{Alignment, Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
	Block, BorderType, Cell, Clear, LineGauge, List, ListItem, Paragraph, Row, Table, Wrap,
};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use super::browser::Kind as BrowserKind;
use super::input::Input;
use super::{App, InputPurpose, Level, Popup};
use crate::batch::format_duration;
use crate::config::Config;
use crate::editor;

pub const ACCENT: Color = Color::Rgb(0xDA, 0x40, 0x00);
pub const BLUE: Color = Color::Rgb(0x25, 0xBF, 0xFF);
pub const OK: Color = Color::Rgb(0x29, 0xDF, 0x47);
pub const WARN: Color = Color::Rgb(0xE8, 0xB3, 0x3B);
pub const BAD: Color = Color::Rgb(0xFF, 0x4D, 0x6A);
pub const DIM: Color = Color::Rgb(0x86, 0x86, 0x86);
pub const SELECTED: Color = Color::Rgb(0x30, 0x30, 0x30);

pub fn status_color(status: Status) -> Color {
	match status {
		Status::Done | Status::Removed => OK,
		Status::NotFound | Status::NoTag => WARN,
		Status::Error | Status::Unsupported => BAD,
		Status::Processing => ACCENT,
		Status::Existing | Status::Instrumental | Status::Empty => Color::Gray,
		Status::Pending => DIM,
	}
}

fn key_hints(pairs: &[(&str, &str)]) -> Line<'static> {
	let mut spans = vec![];
	for (i, (key, label)) in pairs.iter().enumerate() {
		if i > 0 {
			spans.push(Span::raw("  "));
		}
		spans.push(Span::styled(key.to_string(), Style::new().fg(ACCENT).bold()));
		spans.push(Span::styled(format!(" {label}"), Style::new().fg(DIM)));
	}
	Line::from(spans)
}

/// A centered rectangle of at most `width`x`height`, leaving a margin around it.
fn centered(area: Rect, width: u16, height: u16) -> Rect {
	let width = width.min(area.width.saturating_sub(4)).max(10.min(area.width));
	let height = height.min(area.height.saturating_sub(2)).max(3.min(area.height));
	Rect {
		x: area.x + (area.width - width) / 2,
		y: area.y + (area.height - height) / 2,
		width,
		height,
	}
}

fn popup_block(title: impl Into<String>) -> Block<'static> {
	Block::bordered()
		.border_type(BorderType::Rounded)
		.border_style(Style::new().fg(ACCENT))
		.title(Span::styled(format!(" {} ", title.into()), Style::new().bold()))
}

pub fn draw(frame: &mut Frame, app: &mut App) {
	let area = frame.area();
	let [header, body, progress, detail, footer] = Layout::vertical([
		Constraint::Length(1),
		Constraint::Min(3),
		Constraint::Length(1),
		Constraint::Length(1),
		Constraint::Length(1),
	])
	.areas(area);

	draw_header(frame, header, app);
	draw_queue(frame, body, app);
	draw_progress(frame, progress, app);
	draw_detail(frame, detail, app);
	draw_footer(frame, footer, app);
	draw_popup(frame, area, app);
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
	let left = Line::from(vec![
		Span::styled(" ▌", Style::new().fg(BLUE)),
		Span::styled("DeepKick", Style::new().bold()),
		Span::styled(" Lyrics", Style::new().fg(ACCENT).bold()),
		Span::styled(concat!("  v", env!("CARGO_PKG_VERSION")), Style::new().fg(DIM)),
	]);
	frame.render_widget(Paragraph::new(left), area);

	let total = app.entries.len();
	let mut right: Vec<Span> = vec![];
	if total > 0 {
		right.push(Span::styled(
			format!("{total} file{}", if total == 1 { "" } else { "s" }),
			Style::new().fg(DIM),
		));
		for (status, count) in app.counts(0..total) {
			if status == Status::Pending {
				continue;
			}
			right.push(Span::styled(" · ", Style::new().fg(DIM)));
			right.push(Span::styled(
				format!("{count} {}", status.label()),
				Style::new().fg(status_color(status)),
			));
		}
		right.push(Span::raw(" "));
	}
	frame.render_widget(Paragraph::new(Line::from(right)).alignment(Alignment::Right), area);
}

fn draw_queue(frame: &mut Frame, area: Rect, app: &mut App) {
	let title = match app.filter {
		Some(status) => Line::from(vec![
			Span::raw(" Queue · "),
			Span::styled(status.label(), Style::new().fg(status_color(status)).bold()),
			Span::styled(" (Tab next, Esc all) ", Style::new().fg(DIM)),
		]),
		None => Line::from(" Queue "),
	};
	let block = Block::bordered()
		.border_type(BorderType::Rounded)
		.border_style(Style::new().fg(Color::DarkGray))
		.title(title);

	if app.entries.is_empty() {
		let text = Text::from(vec![
			Line::default(),
			Line::from("The queue is empty.").bold(),
			Line::default(),
			key_hints(&[("a", "browse for files and folders")]),
			key_hints(&[("p", "type or paste a path, or drag a folder onto this window")]),
			Line::from(Span::styled(
				"You can also start with paths:  deepkick D:\\Music \"E:\\More music\"",
				Style::new().fg(DIM),
			)),
		])
		.alignment(Alignment::Center);
		frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: true }).block(block), area);
		return;
	}

	let wide = area.width >= 100;
	let visible = app.visible();
	let rows: Vec<Row> = visible
		.iter()
		.map(|&i| {
			let entry = &app.entries[i];
			let mut cells = vec![
				Cell::from(Span::styled(
					entry.status.label(),
					Style::new().fg(status_color(entry.status)),
				)),
				Cell::from(entry.name()),
			];
			if wide {
				cells.push(Cell::from(Span::styled(entry.folder(), Style::new().fg(DIM))));
			}
			cells.push(Cell::from(entry.track()));
			Row::new(cells)
		})
		.collect();
	let mut widths = vec![Constraint::Length(12), Constraint::Fill(4)];
	let mut header = vec!["STATUS", "FILE"];
	if wide {
		widths.push(Constraint::Fill(2));
		header.push("FOLDER");
	}
	widths.push(Constraint::Fill(3));
	header.push("TRACK");

	let table = Table::new(rows, widths)
		.header(Row::new(header).style(Style::new().fg(DIM).add_modifier(Modifier::BOLD)))
		.row_highlight_style(Style::new().bg(SELECTED).add_modifier(Modifier::BOLD))
		.highlight_symbol(Span::styled("▌", Style::new().fg(ACCENT)))
		.column_spacing(2)
		.block(block);
	frame.render_stateful_widget(table, area, &mut app.table);
}

fn draw_progress(frame: &mut Frame, area: Rect, app: &App) {
	if let Some(runner) = &app.runner {
		let total = runner.total().max(1);
		let verb = match (runner.mode, runner.is_cancelling()) {
			(_, true) => "Stopping",
			(deepkick_core::Mode::Fetch, _) => "Fetching",
			(deepkick_core::Mode::Remove, _) => "Removing",
		};
		let current = runner.current.map(|i| app.entries[i].name()).unwrap_or_default();
		let gauge = LineGauge::default()
			.ratio(runner.completed as f64 / total as f64)
			.label(format!(" {verb} {}/{} {current} ", runner.completed, runner.total()))
			.filled_style(Style::new().fg(ACCENT))
			.unfilled_style(Style::new().fg(Color::DarkGray));
		frame.render_widget(gauge, area);
		return;
	}
	let line = match &app.last_run {
		Some(summary) => Line::from(vec![
			Span::styled(format!(" {}: ", summary.title()), Style::new().bold()),
			Span::raw(summary.text.clone()),
			Span::styled(
				format!(" in {:.0} s · Tab filters by status", summary.elapsed.as_secs_f64()),
				Style::new().fg(DIM),
			),
		]),
		None if app.entries.is_empty() => Line::default(),
		None => key_hints(&[(" f", "fetch lyrics for the queue"), ("r", "remove them")]),
	};
	frame.render_widget(Paragraph::new(line), area);
}

fn draw_detail(frame: &mut Frame, area: Rect, app: &App) {
	let Some(index) = app.selected_index() else {
		return;
	};
	let entry = &app.entries[index];
	let mut spans = vec![Span::styled(format!(" {}", entry.path.display()), Style::new().fg(DIM))];
	if let Some(detail) = &entry.detail {
		spans.push(Span::raw("  "));
		spans.push(Span::styled(detail.clone(), Style::new().fg(status_color(entry.status))));
	}
	frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
	if let Some(message) = &app.message {
		let color = match message.level {
			Level::Info => Color::White,
			Level::Ok => OK,
			Level::Warn => WARN,
			Level::Error => BAD,
		};
		let line = Line::from(Span::styled(format!(" {}", message.text), Style::new().fg(color)));
		frame.render_widget(Paragraph::new(line), area);
		return;
	}
	let hints: &[(&str, &str)] = if app.running() {
		&[(" Esc", "stop"), ("e", "edit"), ("v", "view"), ("Tab", "filter"), ("q", "quit")]
	} else {
		&[
			(" a", "add"),
			("p", "path"),
			("f", "fetch"),
			("r", "remove"),
			("e", "edit"),
			("s", "search"),
			("v", "view"),
			("x", "drop"),
			("Tab", "filter"),
			("c", "settings"),
			("?", "help"),
			("q", "quit"),
		]
	};
	frame.render_widget(Paragraph::new(key_hints(hints)), area);
}

fn draw_popup(frame: &mut Frame, area: Rect, app: &mut App) {
	match &mut app.popup {
		Popup::None => {},
		Popup::Help => draw_help(frame, area),
		Popup::Summary => {
			if let Some(summary) = &app.last_run {
				let text = Text::from(vec![
					Line::from(summary.text.clone()),
					Line::default(),
					Line::from(Span::styled(
						format!(
							"Took {:.0} s. Tab filters the queue by status. Any key closes this.",
							summary.elapsed.as_secs_f64()
						),
						Style::new().fg(DIM),
					)),
				]);
				let rect = centered(area, 64, 7);
				frame.render_widget(Clear, rect);
				frame.render_widget(
					Paragraph::new(text).wrap(Wrap { trim: true }).block(popup_block(summary.title())),
					rect,
				);
			}
		},
		Popup::Confirm(_, question) => {
			let rect = centered(area, 60, 6);
			frame.render_widget(Clear, rect);
			let text = Text::from(vec![
				Line::from(question.clone()),
				Line::default(),
				key_hints(&[("y", "yes"), ("n", "no")]),
			]);
			frame.render_widget(
				Paragraph::new(text).wrap(Wrap { trim: true }).block(popup_block("Confirm")),
				rect,
			);
		},
		Popup::Searching(query) => {
			let rect = centered(area, 60, 3);
			frame.render_widget(Clear, rect);
			frame.render_widget(
				Paragraph::new(format!("Searching LRCLIB for {query}...")).block(popup_block("Search")),
				rect,
			);
		},
		Popup::Input(purpose, input) => {
			let (title, hint) = match purpose {
				InputPurpose::AddPaths => (
					"Add files or folders",
					"Type, paste or drag paths here · Tab completes · Enter adds · Esc cancels",
				),
				InputPurpose::Editor => (
					"Editor command",
					"e.g. code --wait, notepad, vim · the file path is appended · empty = automatic",
				),
			};
			draw_input(frame, area, title, hint, input);
		},
		Popup::Browser(browser) => {
			let rect = centered(area, 90, area.height.saturating_sub(4).max(10));
			frame.render_widget(Clear, rect);
			let block = popup_block(format!("Add · {}", browser.title()));
			let inner = block.inner(rect);
			frame.render_widget(block, rect);
			let [list_area, info_area, hint_area] =
				Layout::vertical([Constraint::Min(1), Constraint::Length(1), Constraint::Length(1)])
					.areas(inner);
			let items: Vec<ListItem> = browser
				.entries
				.iter()
				.map(|e| match e.kind {
					BrowserKind::Parent => ListItem::new(Span::styled("..", Style::new().fg(DIM))),
					BrowserKind::Drive => ListItem::new(Span::styled(e.name.clone(), Style::new().fg(BLUE))),
					BrowserKind::Dir => ListItem::new(Span::styled(format!("{}/", e.name), Style::new().fg(BLUE))),
					BrowserKind::File => ListItem::new(Span::raw(e.name.clone())),
				})
				.collect();
			let list = List::new(items)
				.highlight_style(Style::new().bg(SELECTED).add_modifier(Modifier::BOLD))
				.highlight_symbol("▌");
			frame.render_stateful_widget(list, list_area, &mut browser.state);
			let info = match &browser.error {
				Some(e) => Span::styled(e.clone(), Style::new().fg(BAD)),
				None => Span::styled(
					format!(
						"{} audio file{} in this folder",
						browser.audio_count,
						if browser.audio_count == 1 { "" } else { "s" }
					),
					Style::new().fg(DIM),
				),
			};
			frame.render_widget(Paragraph::new(info), info_area);
			frame.render_widget(
				Paragraph::new(key_hints(&[
					("Enter", "open"),
					("Space", "add"),
					("a", "add current folder"),
					("Bksp", "up"),
					("g", "go to"),
					(".", "hidden"),
					("Esc", "close"),
				])),
				hint_area,
			);
			if let Some(goto) = &browser.goto {
				draw_input(frame, area, "Go to", "Tab completes · Enter goes · Esc cancels", goto);
			}
		},
		Popup::Search(search) => {
			let rect = centered(area, 90, area.height.saturating_sub(4).max(10));
			frame.render_widget(Clear, rect);
			let kind = if search.synced { "synced (LRC)" } else { "plain" };
			let block = popup_block(format!("Search · {} · {kind}", search.query));
			let inner = block.inner(rect);
			frame.render_widget(block, rect);
			let [list_area, hint_area] =
				Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
			let file_duration = None::<f64>;
			let items: Vec<ListItem> = search
				.candidates
				.iter()
				.map(|c| {
					let mut kinds = vec![];
					if c.plain.is_some() {
						kinds.push("plain");
					}
					if c.synced.is_some() {
						kinds.push("synced");
					}
					if c.instrumental {
						kinds.push("instrumental");
					}
					let meta = [
						c.album.clone(),
						format_duration(Some(c.duration).filter(|d| *d > 0.0).or(file_duration)),
						kinds.join("+"),
						c.source.clone(),
					]
					.into_iter()
					.filter(|s| !s.is_empty())
					.collect::<Vec<_>>()
					.join(" · ");
					ListItem::new(Text::from(vec![
						Line::from(format!("{} — {}", c.artist, c.title)).bold(),
						Line::from(Span::styled(meta, Style::new().fg(DIM))),
					]))
				})
				.collect();
			let list = List::new(items)
				.highlight_style(Style::new().bg(SELECTED))
				.highlight_symbol("▌");
			frame.render_stateful_widget(list, list_area, &mut search.state);
			frame.render_widget(
				Paragraph::new(key_hints(&[
					("Enter", "write"),
					("e", "edit first"),
					("Tab", "plain/synced"),
					("Esc", "close"),
				])),
				hint_area,
			);
		},
		Popup::View(view) => {
			let rect = centered(area, 90, area.height.saturating_sub(4).max(10));
			frame.render_widget(Clear, rect);
			let block = popup_block(format!("Lyrics · {}", view.title));
			let inner = block.inner(rect);
			frame.render_widget(block, rect);
			let [text_area, hint_area] =
				Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
			let body = match &view.text {
				Some(text) => Paragraph::new(text.clone()).scroll((view.scroll, 0)),
				None => Paragraph::new(Span::styled("No lyrics embedded.", Style::new().fg(DIM))),
			};
			frame.render_widget(body, text_area);
			frame.render_widget(
				Paragraph::new(key_hints(&[("↑↓", "scroll"), ("e", "edit"), ("s", "search"), ("Esc", "close")])),
				hint_area,
			);
		},
		Popup::Config(state) => draw_config(frame, area, &app.cfg, state),
	}
}

fn draw_input(frame: &mut Frame, area: Rect, title: &str, hint: &str, input: &Input) {
	let rect = centered(area, 90, 5);
	frame.render_widget(Clear, rect);
	let block = popup_block(title);
	let inner = block.inner(rect);
	frame.render_widget(block, rect);
	let [field, info] = Layout::vertical([Constraint::Length(1), Constraint::Length(2)]).areas(inner);

	// scroll horizontally so the cursor stays visible
	let text = input.text();
	let before: String = input.chars[..input.cursor].iter().collect();
	let width = field.width.saturating_sub(1) as usize;
	let mut skip = 0usize;
	while before.chars().skip(skip).collect::<String>().width() > width {
		skip += 1;
	}
	let shown: String = text.chars().skip(skip).collect();
	frame.render_widget(Paragraph::new(shown), field);
	let cursor_x = before.chars().skip(skip).collect::<String>().width() as u16;
	frame.set_cursor_position(Position::new(field.x + cursor_x, field.y));

	let info_line = match &input.hint {
		Some(h) => Line::from(Span::styled(h.clone(), Style::new().fg(WARN))),
		None => Line::from(Span::styled(hint.to_string(), Style::new().fg(DIM))),
	};
	frame.render_widget(Paragraph::new(info_line).wrap(Wrap { trim: true }), info);
}

fn draw_config(frame: &mut Frame, area: Rect, cfg: &Config, state: &mut ratatui::widgets::ListState) {
	let rect = centered(area, 76, 12);
	frame.render_widget(Clear, rect);
	let block = popup_block("Settings");
	let inner = block.inner(rect);
	frame.render_widget(block, rect);
	let [list_area, path_area, hint_area] =
		Layout::vertical([Constraint::Min(1), Constraint::Length(2), Constraint::Length(1)]).areas(inner);
	let check = |on: bool| if on { "[x]" } else { "[ ]" };
	let (command, source) = editor::resolve(cfg.editor.as_deref());
	let editor_line = match source {
		"settings" => format!("    Editor: {command}"),
		other => format!("    Editor: {command}  (automatic, from {other})"),
	};
	let items = vec![
		ListItem::new(format!("{} Overwrite lyrics that are already embedded", check(cfg.overwrite))),
		ListItem::new(format!("{} Prefer synchronised (LRC) lyrics", check(cfg.prefer_synced))),
		ListItem::new(format!("{} Try lyrics.ovh when LRCLIB has nothing", check(cfg.fallback))),
		ListItem::new(format!("{} Repair tags stored as CP1251/UTF-8 but labelled Latin-1", check(cfg.fix_encoding))),
		ListItem::new(editor_line),
	];
	let list = List::new(items)
		.highlight_style(Style::new().bg(SELECTED).add_modifier(Modifier::BOLD))
		.highlight_symbol("▌");
	frame.render_stateful_widget(list, list_area, state);
	let path = Config::path().map(|p| p.display().to_string()).unwrap_or_default();
	frame.render_widget(
		Paragraph::new(Span::styled(format!("Saved to {path}"), Style::new().fg(DIM)))
			.wrap(Wrap { trim: true }),
		path_area,
	);
	frame.render_widget(
		Paragraph::new(key_hints(&[("Enter", "toggle / edit"), ("Esc", "close")])),
		hint_area,
	);
}

fn draw_help(frame: &mut Frame, area: Rect) {
	let rows: &[(&str, &str)] = &[
		("a", "browse for files and folders to add"),
		("p", "type, paste or drag paths (Tab completes)"),
		("f", "fetch lyrics for the queue (or the filtered part)"),
		("r", "remove lyrics from the queue (or the filtered part)"),
		("e / Enter", "edit the selected file's lyrics in your editor"),
		("s", "search LRCLIB for the selected file"),
		("v", "view the selected file's lyrics"),
		("x / Del", "drop the selected file from the queue"),
		("X", "clear the queue"),
		("Tab / S-Tab", "filter the queue by status (the report)"),
		("c", "settings: overwrite, synced lyrics, fallback, editor"),
		("Esc", "stop a run / clear the filter"),
		("q", "quit"),
	];
	let lines: Vec<Line> = rows
		.iter()
		.map(|(k, v)| {
			Line::from(vec![
				Span::styled(format!("{k:>12}  "), Style::new().fg(ACCENT).bold()),
				Span::raw(v.to_string()),
			])
		})
		.collect();
	let rect = centered(area, 72, rows.len() as u16 + 2);
	frame.render_widget(Clear, rect);
	frame.render_widget(Paragraph::new(lines).block(popup_block("Keys")), rect);
}

#[cfg(test)]
mod tests {
	use std::path::PathBuf;

	use deepkick_core::lyrics::Candidate;
	use ratatui::backend::TestBackend;
	use ratatui::widgets::ListState;
	use ratatui::Terminal;

	use super::super::{App, Popup, Search, View};
	use super::*;
	use crate::queue::Entry;

	fn render(app: &mut App, width: u16, height: u16) -> String {
		let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
		terminal.draw(|f| draw(f, app)).unwrap();
		let buffer = terminal.backend().buffer().clone();
		let mut out = String::new();
		for y in 0..height {
			for x in 0..width {
				out.push_str(buffer[(x, y)].symbol());
			}
			out.push('\n');
		}
		out
	}

	fn sample_app() -> App {
		let mut app = App::new(Config::default());
		for (name, status, track, detail) in [
			("01. First Song.mp3", Status::Done, ("Some Band", "First Song"), Some("lrclib: Some Band — First Song")),
			("02. Second Song.flac", Status::NotFound, ("Some Band", "Second Song"), None),
			("03. untitled.mp3", Status::NoTag, ("", ""), None),
			("04. Broken.mp3", Status::Error, ("Other", "Broken"), Some("cannot open: access denied")),
		] {
			let mut entry = Entry::new(PathBuf::from(format!("/music/Some Band/Album/{name}")));
			entry.status = status;
			entry.artist = (!track.0.is_empty()).then(|| track.0.to_string());
			entry.title = (!track.1.is_empty()).then(|| track.1.to_string());
			entry.detail = detail.map(String::from);
			app.entries.push(entry);
		}
		app.table.select(Some(0));
		app
	}

	#[test]
	fn renders_main_screen() {
		let mut app = sample_app();
		let screen = render(&mut app, 110, 14);
		println!("{screen}");
		assert!(screen.contains("DeepKick Lyrics"));
		assert!(screen.contains("01. First Song.mp3"));
		assert!(screen.contains("not found"));
		assert!(screen.contains("FOLDER"));
	}

	#[test]
	fn renders_empty_queue_narrow() {
		let mut app = App::new(Config::default());
		let screen = render(&mut app, 70, 14);
		println!("{screen}");
		assert!(screen.contains("The queue is empty."));
	}

	#[test]
	fn renders_popups() {
		let mut app = sample_app();
		app.filter = Some(Status::NotFound);
		app.table.select(Some(0));
		let screen = render(&mut app, 100, 14);
		println!("{screen}");
		assert!(screen.contains("02. Second Song.flac"));
		assert!(!screen.contains("01. First Song.mp3"));

		app.filter = None;
		let mut state = ListState::default();
		state.select(Some(0));
		app.popup = Popup::Search(Search {
			index: 0,
			query: "Some Band — First Song".into(),
			candidates: vec![Candidate {
				id: 1,
				artist: "Some Band".into(),
				title: "First Song".into(),
				album: "Album".into(),
				duration: 201.0,
				instrumental: false,
				plain: Some("placeholder line".into()),
				synced: None,
				source: "lrclib".into(),
			}],
			state,
			synced: false,
		});
		let screen = render(&mut app, 100, 20);
		println!("{screen}");
		assert!(screen.contains("Search · Some Band — First Song · plain"));
		assert!(screen.contains("Album · 3:21 · plain · lrclib"));

		app.popup = Popup::View(View { index: 0, title: "Some Band — First Song".into(), text: Some("placeholder one\nplaceholder two".into()), scroll: 0 });
		let screen = render(&mut app, 100, 16);
		assert!(screen.contains("placeholder two"));

		let mut state = ListState::default();
		state.select(Some(4));
		app.popup = Popup::Config(state);
		let screen = render(&mut app, 100, 16);
		println!("{screen}");
		assert!(screen.contains("[ ] Overwrite"));
		assert!(screen.contains("Editor:"));

		app.popup = Popup::Help;
		let screen = render(&mut app, 100, 20);
		assert!(screen.contains("filter the queue by status"));
	}
}

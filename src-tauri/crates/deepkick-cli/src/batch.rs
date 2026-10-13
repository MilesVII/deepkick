//! Non-interactive commands: `fetch`, `remove`, `show`, `edit`, `config`.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use deepkick_core::{summary_text, tags, Mode, RunOptions, Status};
use ratatui::crossterm::style::{Color, Stylize};

use crate::config::{self, Config};
use crate::editor;
use crate::queue::{Event, Runner};
use crate::scan;

fn colored(status: Status, text: String) -> String {
	if !std::io::stdout().is_terminal() {
		return text;
	}
	let color = match status {
		Status::Done | Status::Removed => Color::Green,
		Status::NotFound | Status::NoTag => Color::Yellow,
		Status::Error | Status::Unsupported => Color::Red,
		_ => Color::DarkGrey,
	};
	text.with(color).to_string()
}

/// A path relative to the working folder when it is inside it.
pub fn display_path(path: &Path) -> String {
	std::env::current_dir()
		.ok()
		.and_then(|cwd| path.strip_prefix(cwd).ok().map(|p| p.display().to_string()))
		.unwrap_or_else(|| path.display().to_string())
}

fn confirm(question: &str) -> Result<bool> {
	if !std::io::stdin().is_terminal() {
		bail!("{question} Pass --yes to confirm when not running in a terminal.");
	}
	print!("{question} [y/N] ");
	std::io::stdout().flush()?;
	let mut answer = String::new();
	std::io::stdin().read_line(&mut answer)?;
	Ok(matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes"))
}

pub fn run(paths: &[PathBuf], mode: Mode, options: RunOptions, yes: bool) -> Result<ExitCode> {
	let scan = scan::collect(paths);
	for warning in &scan.warnings {
		eprintln!("warning: {warning}");
	}
	if scan.files.is_empty() {
		bail!("no MP3 or FLAC files to process");
	}
	let total = scan.files.len();
	if mode == Mode::Remove
		&& !yes
		&& !confirm(&format!("Remove embedded lyrics from {total} file(s)?"))?
	{
		println!("Nothing changed.");
		return Ok(ExitCode::SUCCESS);
	}

	let runner = Runner::start(scan.files.iter().cloned().enumerate().collect(), mode, options);

	// First Ctrl+C finishes the current file and stops, a second one exits right away.
	let presses = Arc::new(AtomicUsize::new(0));
	let flag = runner.cancel_flag();
	let counter = presses.clone();
	let _ = ctrlc::set_handler(move || {
		if counter.fetch_add(1, Ordering::SeqCst) == 0 {
			eprintln!("\nStopping after the current file (Ctrl+C again to quit now)...");
			flag.store(true, Ordering::SeqCst);
		} else {
			std::process::exit(130);
		}
	});

	let width = total.to_string().len();
	let mut done = 0usize;
	let mut counts: BTreeMap<Status, usize> = BTreeMap::new();
	let cancelled = loop {
		match runner.next_blocking() {
			Event::Started(_) => {},
			Event::Finished(index, outcome) => {
				done += 1;
				*counts.entry(outcome.status).or_insert(0) += 1;
				let label = colored(outcome.status, format!("{:<12}", outcome.status.label()));
				let mut line = format!(
					"[{done:>width$}/{total}] {label} {}",
					display_path(&scan.files[index])
				);
				if let Some(detail) = &outcome.detail {
					line.push_str(&format!("  ({detail})"));
				}
				println!("{line}");
			},
			Event::Done { cancelled } => break cancelled,
		}
	};
	runner.join();
	if cancelled && done < total {
		*counts.entry(Status::Pending).or_insert(0) += total - done;
	}
	let title = match (mode, cancelled) {
		(Mode::Fetch, false) => "Lyrics fetched",
		(Mode::Fetch, true) => "Fetching stopped",
		(Mode::Remove, false) => "Lyrics removed",
		(Mode::Remove, true) => "Removal stopped",
	};
	println!("{title}: {}", summary_text(mode, &counts));
	Ok(if counts.contains_key(&Status::Error) {
		ExitCode::FAILURE
	} else {
		ExitCode::SUCCESS
	})
}

fn read_info(path: &Path) -> Result<tags::Info> {
	let mut file = File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
	let kind = tags::probe(&mut file)
		.map_err(anyhow::Error::msg)?
		.context("not an MP3 or FLAC file")?;
	tags::read(&mut file, kind).map_err(anyhow::Error::msg)
}

pub fn write_lyrics(path: &Path, lyrics: Option<&str>, fix_encoding: bool) -> Result<()> {
	let mut file = File::options()
		.read(true)
		.write(true)
		.open(path)
		.with_context(|| format!("cannot open {} for writing", path.display()))?;
	let kind = tags::probe(&mut file)
		.map_err(anyhow::Error::msg)?
		.context("not an MP3 or FLAC file")?;
	tags::write_lyrics(&mut file, kind, lyrics, fix_encoding)
		.map(|_| ())
		.map_err(anyhow::Error::msg)
}

pub fn format_duration(seconds: Option<f64>) -> String {
	match seconds {
		Some(s) if s > 0.0 => {
			let total = s.round() as u64;
			format!("{}:{:02}", total / 60, total % 60)
		},
		_ => String::new(),
	}
}

pub fn show(path: &Path) -> Result<()> {
	let info = read_info(path)?;
	let field = |v: &Option<String>| v.clone().unwrap_or_else(|| "-".into());
	println!("File:     {}", path.display());
	println!("Format:   {}", if info.kind == tags::Kind::Mp3 { "MP3" } else { "FLAC" });
	println!("Artist:   {}", field(&info.artist));
	println!("Title:    {}", field(&info.title));
	println!("Album:    {}", field(&info.album));
	println!("Duration: {}", format_duration(info.duration_secs));
	if let Some(how) = info.repaired {
		println!("Note:     the tags are stored as {} bytes labelled Latin-1 (shown repaired)", how.label());
	}
	match &info.lyrics {
		Some(lyrics) => {
			println!("Lyrics:   {} lines", lyrics.lines().count());
			println!();
			println!("{lyrics}");
		},
		None => println!("Lyrics:   none"),
	}
	Ok(())
}

pub fn edit(cfg: &Config, path: &Path) -> Result<()> {
	let info = read_info(path)?;
	let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
	let track = match (&info.artist, &info.title) {
		(Some(a), Some(t)) => format!("{a} — {t}"),
		_ => String::new(),
	};
	let (command, _) = editor::resolve(cfg.editor.as_deref());
	let current = info.lyrics.clone().unwrap_or_default();
	let edited = editor::edit(&command, &editor::header_for(&name, &track), &current)?;
	if edited.text == current.trim() {
		println!("No changes.");
		if edited.quick {
			println!("If your editor returns immediately, add its wait flag, e.g. `code --wait`.");
		}
		return Ok(());
	}
	if edited.text.is_empty() {
		// emptying the text is an explicit choice; only double-check when someone can answer
		if std::io::stdin().is_terminal()
			&& !confirm("The text is empty. Remove the lyrics from the file?")?
		{
			println!("Nothing changed.");
			return Ok(());
		}
		write_lyrics(path, None, cfg.fix_encoding)?;
		println!("Lyrics removed.");
	} else {
		write_lyrics(path, Some(&edited.text), cfg.fix_encoding)?;
		println!("Lyrics saved ({} lines).", edited.text.lines().count());
	}
	Ok(())
}

pub fn config_command(key: Option<&str>, value: Option<&str>) -> Result<()> {
	let mut cfg = Config::load()?;
	match (key, value) {
		(None, _) => {
			if let Some(path) = Config::path() {
				println!("# {}", path.display());
			}
			for key in config::KEYS {
				println!("{key} = {}", cfg.get(key)?);
			}
			let (command, source) = editor::resolve(cfg.editor.as_deref());
			println!("# editor in use: {command} (from {source})");
		},
		(Some(key), None) => println!("{}", cfg.get(key)?),
		(Some(key), Some(value)) => {
			cfg.set(key, value)?;
			cfg.save()?;
			println!("{key} = {}", cfg.get(key)?);
		},
	}
	Ok(())
}

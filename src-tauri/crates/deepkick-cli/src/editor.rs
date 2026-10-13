//! Editing lyrics in an external editor, the way git edits commit messages.

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};

use crate::scan::split_words;

/// Everything above this line in the temporary file is ignored.
pub const SCISSORS: &str = "# ------------------------ >8 ------------------------";

/// The editor command line and where it was taken from: `DEEPKICK_EDITOR`, the settings,
/// `VISUAL`, `EDITOR`, then a platform default.
pub fn resolve(configured: Option<&str>) -> (String, &'static str) {
	if let Ok(value) = std::env::var("DEEPKICK_EDITOR") {
		if !value.trim().is_empty() {
			return (value, "DEEPKICK_EDITOR");
		}
	}
	if let Some(value) = configured {
		if !value.trim().is_empty() {
			return (value.to_string(), "settings");
		}
	}
	for var in ["VISUAL", "EDITOR"] {
		if let Ok(value) = std::env::var(var) {
			if !value.trim().is_empty() {
				return (value, if var == "VISUAL" { "VISUAL" } else { "EDITOR" });
			}
		}
	}
	(default_editor(), "default")
}

fn default_editor() -> String {
	if cfg!(windows) {
		return "notepad".into();
	}
	for candidate in ["nano", "vim", "vi"] {
		if which::which(candidate).is_ok() {
			return candidate.into();
		}
	}
	"vi".into()
}

pub struct Edited {
	/// The text below the scissors line, trimmed; empty means "no lyrics"
	pub text: String,
	/// The editor returned almost immediately (a GUI editor without its wait flag?)
	pub quick: bool,
}

/// Writes `initial` below a commented header into a temporary file, opens it in the
/// editor, waits for it to close and returns what is left below the scissors line.
pub fn edit(command_line: &str, header: &[String], initial: &str) -> Result<Edited> {
	let path = std::env::temp_dir().join(format!("deepkick-lyrics-{}.txt", std::process::id()));
	let mut content = String::new();
	for line in header {
		content.push_str("# ");
		content.push_str(line);
		content.push('\n');
	}
	content.push_str(SCISSORS);
	content.push('\n');
	if !initial.trim().is_empty() {
		content.push_str(initial.trim_end());
		content.push('\n');
	}
	std::fs::write(&path, &content)
		.with_context(|| format!("cannot write the temporary file {}", path.display()))?;

	let started = Instant::now();
	let launched = launch(command_line, &path);
	let edited = std::fs::read_to_string(&path);
	let _ = std::fs::remove_file(&path);
	launched?;
	let edited = edited.context("cannot read the edited file back")?;
	Ok(Edited { text: parse(&edited), quick: started.elapsed() < Duration::from_millis(1500) })
}

/// Default header shown above the scissors line.
pub fn header_for(name: &str, track: &str) -> Vec<String> {
	let subject = if track.is_empty() { name.to_string() } else { format!("{track} ({name})") };
	vec![
		format!("DeepKick Lyrics: editing the lyrics of {subject}"),
		"Everything above the scissors line is ignored. Save and close the editor to apply,".into(),
		"leave the text below empty to remove the lyrics.".into(),
	]
}

fn launch(command_line: &str, file: &Path) -> Result<()> {
	let words = split_words(command_line);
	let (program, args) = words
		.split_first()
		.ok_or_else(|| anyhow!("the editor command is empty"))?;
	let resolved = which::which(program).map_err(|_| {
		anyhow!(
			"editor `{program}` not found; set one with `deepkick config editor \"<command>\"` \
			 or in the settings (c)"
		)
	})?;
	let status = Command::new(&resolved)
		.args(args)
		.arg(file)
		.status()
		.with_context(|| format!("cannot start {}", resolved.display()))?;
	if !status.success() {
		bail!("the editor exited with {status}");
	}
	Ok(())
}

/// Extracts the lyrics from the edited file: everything after the scissors line, or, if the
/// line was deleted, everything after the leading `#` comment lines.
pub fn parse(raw: &str) -> String {
	let raw = raw.trim_start_matches('\u{feff}').replace("\r\n", "\n");
	let lines: Vec<&str> = raw.split('\n').collect();
	let body: Vec<&str> = match lines.iter().position(|l| l.trim_end() == SCISSORS) {
		Some(i) => lines[i + 1..].to_vec(),
		None => lines.iter().copied().skip_while(|l| l.starts_with('#')).collect(),
	};
	let text = body.join("\n");
	let text = text.trim_end();
	// drop leading blank lines but keep the indentation of the first real line
	let first = text.find(|c: char| c != '\n' && c != ' ' && c != '\t').unwrap_or(text.len());
	let start = text[..first].rfind('\n').map(|i| i + 1).unwrap_or(0);
	text[start..].to_string()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parses_below_scissors() {
		let raw = format!("# header\n# more\n{SCISSORS}\n\nline one\n  line two\n\n");
		assert_eq!(parse(&raw), "line one\n  line two");
	}

	#[test]
	fn handles_crlf_bom_and_missing_scissors() {
		let raw = "\u{feff}# header\r\n# more\r\nfirst\r\nsecond\r\n";
		assert_eq!(parse(raw), "first\nsecond");
	}

	#[test]
	fn empty_means_no_lyrics() {
		assert_eq!(parse(&format!("# h\n{SCISSORS}\n\n  \n")), "");
	}
}

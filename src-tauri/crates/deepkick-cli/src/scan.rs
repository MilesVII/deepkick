//! Turning user input (arguments, typed or pasted paths) into a list of audio files.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use deepkick_core::is_supported_name;

pub struct Scan {
	pub files: Vec<PathBuf>,
	pub warnings: Vec<String>,
}

/// Expands `~` to the home folder.
pub fn expand_home(path: &Path) -> PathBuf {
	let text = path.to_string_lossy();
	if text == "~" {
		return dirs::home_dir().unwrap_or_else(|| path.to_path_buf());
	}
	if let Some(rest) = text.strip_prefix("~/").or_else(|| text.strip_prefix("~\\")) {
		if let Some(home) = dirs::home_dir() {
			return home.join(rest);
		}
	}
	path.to_path_buf()
}

fn absolute(path: &Path) -> PathBuf {
	let expanded = expand_home(path);
	std::path::absolute(&expanded).unwrap_or(expanded)
}

/// Collects every MP3 and FLAC file from the given files and folders (recursively),
/// without duplicates, keeping the order they were given in.
pub fn collect(paths: &[PathBuf]) -> Scan {
	let mut files = vec![];
	let mut warnings = vec![];
	for path in paths {
		let path = absolute(path);
		match std::fs::metadata(&path) {
			Ok(meta) if meta.is_dir() => {
				let mut found: Vec<PathBuf> = walkdir::WalkDir::new(&path)
					.into_iter()
					.filter_map(|e| e.ok())
					.filter(|e| {
						e.file_type().is_file() && is_supported_name(&e.file_name().to_string_lossy())
					})
					.map(|e| e.into_path())
					.collect();
				if found.is_empty() {
					warnings.push(format!("no MP3 or FLAC files in {}", path.display()));
				}
				found.sort();
				files.extend(found);
			},
			Ok(_) => {
				let name = path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
				if is_supported_name(&name) {
					files.push(path);
				} else {
					warnings.push(format!("skipped {} (not an MP3 or FLAC file)", path.display()));
				}
			},
			Err(e) => warnings.push(format!("{}: {e}", path.display())),
		}
	}
	let mut seen = HashSet::new();
	files.retain(|f| seen.insert(f.clone()));
	Scan { files, warnings }
}

/// Splits a command line or a list of paths into words. Quotes group words, backslashes
/// are kept as they are (they are path separators on Windows, not escapes). A quote only
/// opens a group at the start of a word, so `Don't Stop.mp3` stays intact.
pub fn split_words(input: &str) -> Vec<String> {
	let mut words = vec![];
	let mut current = String::new();
	let mut quote: Option<char> = None;
	let mut quoted_word = false;
	for c in input.chars() {
		match quote {
			Some(q) if c == q => quote = None,
			Some(_) => current.push(c),
			None if (c == '"' || c == '\'') && current.is_empty() => {
				quote = Some(c);
				quoted_word = true;
			},
			None if c.is_whitespace() => {
				if !current.is_empty() || quoted_word {
					words.push(std::mem::take(&mut current));
				}
				quoted_word = false;
			},
			None => current.push(c),
		}
	}
	if !current.is_empty() || quoted_word {
		words.push(current);
	}
	words
}

fn percent_decode(input: &str) -> String {
	let bytes = input.as_bytes();
	let mut out = Vec::with_capacity(bytes.len());
	let mut i = 0;
	while i < bytes.len() {
		if bytes[i] == b'%' && i + 2 < bytes.len() {
			if let Ok(byte) = u8::from_str_radix(&input[i + 1..i + 3], 16) {
				out.push(byte);
				i += 3;
				continue;
			}
		}
		out.push(bytes[i]);
		i += 1;
	}
	String::from_utf8_lossy(&out).into_owned()
}

fn from_uri(word: &str) -> PathBuf {
	match word.strip_prefix("file://") {
		Some(rest) => {
			let decoded = percent_decode(rest);
			// file:///C:/Music -> C:/Music on Windows
			if cfg!(windows) && decoded.len() > 3 && decoded.as_bytes()[2] == b':' {
				PathBuf::from(&decoded[1..])
			} else {
				PathBuf::from(decoded)
			}
		},
		None => PathBuf::from(word),
	}
}

/// Interprets typed or pasted text (possibly several dragged files) as a list of paths.
pub fn parse_path_list(input: &str) -> Vec<PathBuf> {
	let trimmed = input.trim();
	if trimmed.is_empty() {
		return vec![];
	}
	// One path with spaces typed or pasted without quotes
	let whole = trimmed.trim_matches(|c| c == '"' || c == '\'');
	if !whole.contains(['"', '\n']) && expand_home(Path::new(whole)).exists() {
		return vec![PathBuf::from(whole)];
	}
	trimmed
		.lines()
		.flat_map(|line| {
			let line = line.trim();
			if expand_home(Path::new(line)).exists() {
				vec![line.to_string()]
			} else {
				split_words(line)
			}
		})
		.filter(|w| !w.is_empty())
		.map(|w| from_uri(&w))
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn splits_quoted_words_without_escapes() {
		assert_eq!(
			split_words(r#""C:\Program Files\Notepad++\notepad++.exe" -multiInst  -nosession"#),
			vec![r"C:\Program Files\Notepad++\notepad++.exe", "-multiInst", "-nosession"]
		);
		assert_eq!(split_words("code --wait"), vec!["code", "--wait"]);
		assert_eq!(split_words("Don't Stop.mp3"), vec!["Don't", "Stop.mp3"]);
		assert_eq!(split_words(r#"'a b' "c d" e"#), vec!["a b", "c d", "e"]);
	}

	#[test]
	fn parses_dropped_paths() {
		let list = parse_path_list(r#""C:\My Music\a.mp3" C:\b.flac"#);
		assert_eq!(list, vec![PathBuf::from(r"C:\My Music\a.mp3"), PathBuf::from(r"C:\b.flac")]);
		let uris = parse_path_list("file:///home/me/My%20Music\n");
		assert!(uris[0].to_string_lossy().ends_with("me/My Music"));
		assert!(parse_path_list("   ").is_empty());
	}
}

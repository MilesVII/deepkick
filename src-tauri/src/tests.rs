//! Core logic tests. Set `DEEPKICK_TEST_DIR` to a directory containing the generated test
//! files (`creep.mp3`, `karma.flac`, `notag.mp3`); network tests run against LRCLIB.

use std::fs::File;
use std::path::PathBuf;

use crate::lyrics::{self, Found};
use crate::tags::{self, Kind};

fn fixture(name: &str) -> Option<PathBuf> {
	let dir = std::env::var_os("DEEPKICK_TEST_DIR")?;
	let src = PathBuf::from(dir).join(name);
	if !src.exists() {
		return None;
	}
	let out = std::env::temp_dir().join(format!("deepkick-test-{}-{name}", std::process::id()));
	std::fs::copy(&src, &out).ok()?;
	Some(out)
}

fn open(path: &PathBuf) -> File {
	File::options().read(true).write(true).open(path).unwrap()
}

const LYRICS: &str = "When you were here before\nCouldn't look you in the eye\n\nYou're just like an angel";

#[test]
fn mp3_roundtrip() {
	let Some(path) = fixture("creep.mp3") else { return };
	let mut file = open(&path);
	assert_eq!(tags::probe(&mut file).unwrap(), Some(Kind::Mp3));

	let info = tags::read(&mut file, Kind::Mp3).unwrap();
	assert_eq!(info.artist.as_deref(), Some("Radiohead"));
	assert_eq!(info.title.as_deref(), Some("Creep"));
	assert_eq!(info.album.as_deref(), Some("Pablo Honey"));
	assert!(info.duration_secs.unwrap() > 3.0);
	assert!(info.lyrics.is_none());

	tags::write_lyrics(&mut file, Kind::Mp3, Some(LYRICS)).unwrap();
	drop(file);

	let mut file = open(&path);
	let info = tags::read(&mut file, Kind::Mp3).unwrap();
	assert_eq!(info.lyrics.as_deref(), Some(LYRICS));
	assert_eq!(info.artist.as_deref(), Some("Radiohead"));
	assert_eq!(info.title.as_deref(), Some("Creep"));

	// overwrite with a longer text, then remove
	let longer = LYRICS.repeat(40);
	tags::write_lyrics(&mut file, Kind::Mp3, Some(&longer)).unwrap();
	drop(file);
	let mut file = open(&path);
	assert_eq!(tags::read(&mut file, Kind::Mp3).unwrap().lyrics.as_deref(), Some(longer.trim()));
	tags::write_lyrics(&mut file, Kind::Mp3, None).unwrap();
	drop(file);
	let mut file = open(&path);
	let info = tags::read(&mut file, Kind::Mp3).unwrap();
	assert!(info.lyrics.is_none());
	assert_eq!(info.title.as_deref(), Some("Creep"));
	let _ = std::fs::remove_file(path);
}

#[test]
fn flac_roundtrip() {
	let Some(path) = fixture("karma.flac") else { return };
	let mut file = open(&path);
	assert_eq!(tags::probe(&mut file).unwrap(), Some(Kind::Flac));

	let info = tags::read(&mut file, Kind::Flac).unwrap();
	assert_eq!(info.artist.as_deref(), Some("Radiohead"));
	assert_eq!(info.title.as_deref(), Some("Karma Police"));
	assert!(info.lyrics.is_none());

	tags::write_lyrics(&mut file, Kind::Flac, Some(LYRICS)).unwrap();
	drop(file);
	let mut file = open(&path);
	let info = tags::read(&mut file, Kind::Flac).unwrap();
	assert_eq!(info.lyrics.as_deref(), Some(LYRICS));
	assert_eq!(info.album.as_deref(), Some("OK Computer"));

	tags::write_lyrics(&mut file, Kind::Flac, None).unwrap();
	drop(file);
	let mut file = open(&path);
	let info = tags::read(&mut file, Kind::Flac).unwrap();
	assert!(info.lyrics.is_none());
	assert_eq!(info.title.as_deref(), Some("Karma Police"));
	let _ = std::fs::remove_file(path);
}

#[test]
fn mp3_without_tags() {
	let Some(path) = fixture("notag.mp3") else { return };
	let mut file = open(&path);
	assert_eq!(tags::probe(&mut file).unwrap(), Some(Kind::Mp3));
	let info = tags::read(&mut file, Kind::Mp3).unwrap();
	assert!(info.artist.is_none() && info.title.is_none());
	// writing creates a fresh ID3v2 tag
	tags::write_lyrics(&mut file, Kind::Mp3, Some("la la la")).unwrap();
	drop(file);
	let mut file = open(&path);
	assert_eq!(tags::read(&mut file, Kind::Mp3).unwrap().lyrics.as_deref(), Some("la la la"));
	let _ = std::fs::remove_file(path);
}

#[test]
fn simplify_title_strips_decorations() {
	assert_eq!(lyrics::simplify_title("Creep (Remastered)").as_deref(), Some("Creep"));
	assert_eq!(lyrics::simplify_title("Creep [Live]").as_deref(), Some("Creep"));
	assert_eq!(lyrics::simplify_title("Creep"), None);
	assert_eq!(lyrics::simplify_title("(Intro)"), None);
}

#[test]
#[ignore = "needs network"]
fn lrclib_find_known_song() {
	let client = lyrics::Client::new();
	match client.find("Radiohead", "Creep", Some("Pablo Honey"), Some(239.0), false, false).unwrap() {
		Some(Found::Lyrics { text, source }) => {
			assert!(text.contains("creep"), "unexpected lyrics: {text}");
			assert!(source.starts_with("lrclib"));
		},
		other => panic!("expected lyrics, got {}", match other {
			Some(Found::Instrumental) => "instrumental",
			None => "nothing",
			_ => unreachable!(),
		}),
	}
	// decorated title falls back to the simplified one
	let found = client.find("Radiohead", "Creep (Live at Glastonbury)", None, None, false, false).unwrap();
	assert!(matches!(found, Some(Found::Lyrics { .. })));
	// search list for the editor
	let candidates = client.search("Radiohead", "Creep", None, None).unwrap();
	assert!(!candidates.is_empty());
}

#[test]
#[ignore = "needs network"]
fn lrclib_unknown_song() {
	let client = lyrics::Client::new();
	let found = client.find("Nobody Anywhere", "Unfindable Song 12345", None, None, false, true).unwrap();
	assert!(found.is_none());
}

#[test]
fn report_json_roundtrip() {
	use crate::state::*;
	let mut queue = QueueState::default();
	queue.items.push(Item { id: 1, uri: "x".into(), name: "x.mp3".into(), folder: "".into(), size: 0, status: Status::Done, artist: None, title: None, detail: None });
	queue.run = Some(Run { mode: Mode::Fetch, active: false, cancelled: false, started_at: 1, finished_at: Some(2), current: 1, total: 1 });
	let report = Report::from_queue(&queue).unwrap();
	let json = serde_json::to_string(&report).unwrap();
	println!("{json}");
	let back: Report = serde_json::from_str(&json).unwrap();
	assert_eq!(back.summary.get(&Status::Done), Some(&1));
	let value = serde_json::to_value(&report).unwrap();
	assert!(value["summary"]["done"].is_number());
}

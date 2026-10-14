//! Real-world tag defects: padding junk before the audio, unparseable date frames and
//! Cyrillic text stored as CP1251 but labelled Latin-1. Fixtures (`junk.mp3`, `year.mp3`,
//! `cp1251.mp3`) live in `DEEPKICK_TEST_DIR`; the tests are skipped without it.

use std::fs::File;
use std::path::PathBuf;

use deepkick_core::encoding::Repair;
use deepkick_core::tags::{self, Kind, LyricsChange};

fn fixture(name: &str) -> Option<PathBuf> {
	let dir = std::env::var_os("DEEPKICK_TEST_DIR")?;
	let src = PathBuf::from(dir).join(name);
	if !src.exists() {
		return None;
	}
	let out = std::env::temp_dir().join(format!("deepkick-robust-{}-{name}", std::process::id()));
	std::fs::copy(&src, &out).ok()?;
	Some(out)
}

fn open(path: &PathBuf) -> File {
	File::options().read(true).write(true).open(path).unwrap()
}

#[test]
fn tolerates_padding_junk_before_the_audio() {
	let Some(path) = fixture("junk.mp3") else { return };
	let mut file = open(&path);
	assert_eq!(tags::probe(&mut file).unwrap(), Some(Kind::Mp3));
	let info = tags::read(&mut file, Kind::Mp3).unwrap();
	assert_eq!(info.artist.as_deref(), Some("Radiohead"));
	tags::write_lyrics(&mut file, Kind::Mp3, Some("la"), true).unwrap();
	drop(file);
	let mut file = open(&path);
	assert_eq!(tags::read(&mut file, Kind::Mp3).unwrap().lyrics.as_deref(), Some("la"));
	let _ = std::fs::remove_file(path);
}

#[test]
fn keeps_the_year_of_an_unparseable_date_frame() {
	let Some(path) = fixture("year.mp3") else { return };
	let mut file = open(&path);
	let info = tags::read(&mut file, Kind::Mp3).unwrap();
	assert_eq!(info.artist.as_deref(), Some("Rammstein"));
	assert_eq!(info.album.as_deref(), Some("Sehnsucht"));
	let report = tags::write_lyrics(&mut file, Kind::Mp3, Some("la"), true).unwrap();
	assert!(report.year_rescued);
	drop(file);

	// the rewritten tag carries 1997 in a frame lofty can read
	let mut file = open(&path);
	let mpeg = lofty::mpeg::MpegFile::read_from(&mut file, lofty::config::ParseOptions::new()).unwrap();
	use lofty::prelude::*;
	let tag = mpeg.id3v2().unwrap();
	let year = tag.iter().find_map(|f| match f {
		lofty::id3::v2::Frame::Timestamp(t) => Some(t.timestamp.year),
		_ => None,
	});
	assert_eq!(year, Some(1997));
	assert_eq!(tag.artist().as_deref(), Some("Rammstein"));
	let _ = std::fs::remove_file(path);
}

#[test]
fn repairs_cp1251_tags_when_writing() {
	let Some(path) = fixture("cp1251.mp3") else { return };
	let mut file = open(&path);
	let info = tags::read(&mut file, Kind::Mp3).unwrap();
	assert_eq!(info.artist.as_deref(), Some("Дельфин"));
	assert_eq!(info.title.as_deref(), Some("Сны"));
	assert_eq!(info.repaired, Some(Repair::Cp1251));

	// repair only, no lyrics change
	let report = tags::write(&mut file, Kind::Mp3, LyricsChange::Keep, true).unwrap();
	assert_eq!(report.repaired_fields, 2);
	drop(file);

	let mut file = open(&path);
	let info = tags::read(&mut file, Kind::Mp3).unwrap();
	assert_eq!(info.artist.as_deref(), Some("Дельфин"));
	assert_eq!(info.title.as_deref(), Some("Сны"));
	assert_eq!(info.repaired, None, "the tag should now be stored properly");
	let _ = std::fs::remove_file(path);
}

#[test]
fn leaves_encoding_alone_when_asked() {
	let Some(path) = fixture("cp1251.mp3") else { return };
	let mut file = open(&path);
	let report = tags::write_lyrics(&mut file, Kind::Mp3, Some("la"), false).unwrap();
	assert_eq!(report.repaired_fields, 0);
	drop(file);
	let mut file = open(&path);
	let info = tags::read(&mut file, Kind::Mp3).unwrap();
	assert_eq!(info.lyrics.as_deref(), Some("la"));
	assert_eq!(info.repaired, Some(Repair::Cp1251), "still mis-encoded on disk, repaired on read");
	let _ = std::fs::remove_file(path);
}

#[test]
fn survives_a_bogus_extended_header_flag() {
	let Some(path) = fixture("exthdr.mp3") else { return };
	let mut file = open(&path);
	let info = tags::read(&mut file, Kind::Mp3).unwrap();
	assert_eq!(info.artist.as_deref(), Some("Radiohead"));
	assert!(info.structure_fixed);
	let report = tags::write_lyrics(&mut file, Kind::Mp3, Some("la"), true).unwrap();
	assert!(report.structure_fixed);
	drop(file);
	let mut file = open(&path);
	let info = tags::read(&mut file, Kind::Mp3).unwrap();
	assert_eq!(info.lyrics.as_deref(), Some("la"));
	assert!(!info.structure_fixed, "the rewritten tag should be clean");
	let _ = std::fs::remove_file(path);
}

#[test]
fn survives_bogus_length_indicators_and_null_languages() {
	let Some(path) = fixture("dli.mp3") else { return };
	let mut file = open(&path);
	let info = tags::read(&mut file, Kind::Mp3).unwrap();
	assert_eq!(info.artist.as_deref(), Some("Radiohead"));
	assert_eq!(info.title.as_deref(), Some("Creep"));
	assert!(info.structure_fixed);
	// the null-language comments would make lofty refuse the write without the fix
	let report = tags::write_lyrics(&mut file, Kind::Mp3, Some("la"), true).unwrap();
	assert!(report.structure_fixed);
	drop(file);
	let mut file = open(&path);
	let info = tags::read(&mut file, Kind::Mp3).unwrap();
	assert_eq!(info.lyrics.as_deref(), Some("la"));
	assert_eq!(info.album.as_deref(), Some("Pablo Honey"));
	assert!(!info.structure_fixed);
	let _ = std::fs::remove_file(path);
}

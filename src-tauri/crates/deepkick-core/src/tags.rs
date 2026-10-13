//! Reading and writing lyrics in MP3 (ID3v2 `USLT`) and FLAC (Vorbis comment `LYRICS`) files.
//!
//! Real-world files are messy, so reading is tolerant: up to a few megabytes of junk between
//! the tag and the first audio frame are skipped, malformed frames are dropped instead of
//! failing the whole tag, and Cyrillic text stored as CP1251 but labelled Latin-1 is repaired
//! (see [`crate::encoding`]).

use std::borrow::Cow;
use std::fs::File;
use std::io::{Read, Seek};

use lofty::config::{ParseOptions, ParsingMode, WriteOptions};
use lofty::file::FileType;
use lofty::flac::FlacFile;
use lofty::id3::v2::{
	Frame, FrameId, Id3v2Tag, Id3v2Version, TimestampFrame, UnsynchronizedTextFrame,
};
use lofty::mpeg::MpegFile;
use lofty::ogg::tag::VorbisComments;
use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::items::Timestamp;
use lofty::TextEncoding;
use serde::Serialize;

use crate::encoding::{self, Repair};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
	Mp3,
	Flac,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
	pub kind: Kind,
	pub artist: Option<String>,
	pub title: Option<String>,
	pub album: Option<String>,
	pub duration_secs: Option<f64>,
	pub lyrics: Option<String>,
	/// Set when the artist, title, album or lyrics had to be repaired (the fields above hold
	/// the repaired text); says what the bytes really were.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub repaired: Option<Repair>,
}

/// What to do with the lyrics when writing a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LyricsChange {
	Keep,
	Set(String),
	Remove,
}

/// What a write did besides the lyrics change.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WriteReport {
	/// Number of tag fields whose encoding was repaired
	pub repaired_fields: usize,
	/// The year was carried over from a malformed date frame lofty would have dropped
	pub year_rescued: bool,
}

const USLT: FrameId<'static> = FrameId::Valid(Cow::Borrowed("USLT"));
const TDRC: FrameId<'static> = FrameId::Valid(Cow::Borrowed("TDRC"));
const VORBIS_LYRICS_KEYS: [&str; 2] = ["LYRICS", "UNSYNCEDLYRICS"];
/// Some rippers leave far more than lofty's default kilobyte of padding before the audio.
const MAX_JUNK_BYTES: usize = 4 << 20;

fn parse_options(read_properties: bool) -> ParseOptions {
	ParseOptions::new()
		.read_properties(read_properties)
		.parsing_mode(ParsingMode::Relaxed)
		.max_junk_bytes(MAX_JUNK_BYTES)
}

/// The writer re-probes the file before rewriting it, so it needs the same tolerance.
fn write_options(use_id3v23: bool) -> WriteOptions {
	let mut options = WriteOptions::default();
	let mut options = options.parse_options(parse_options(false));
	if use_id3v23 {
		options = options.use_id3v23(true);
	}
	options
}

fn clean(value: Option<Cow<'_, str>>) -> Option<String> {
	value
		.map(|v| v.trim().to_string())
		.filter(|v| !v.is_empty())
}

/// Repairs a field in place; returns what was wrong with it.
fn repair_field(value: &mut Option<String>) -> Option<Repair> {
	let (fixed, how) = encoding::repair(value.as_deref()?)?;
	*value = Some(fixed);
	Some(how)
}

/// Detects whether the file is an MP3 or a FLAC by its contents.
pub fn probe(file: &mut File) -> Result<Option<Kind>, String> {
	file.rewind().map_err(|e| e.to_string())?;
	let probe = Probe::new(&mut *file)
		.options(parse_options(false))
		.guess_file_type()
		.map_err(|e| e.to_string())?;
	let kind = match probe.file_type() {
		Some(FileType::Mpeg) => Some(Kind::Mp3),
		Some(FileType::Flac) => Some(Kind::Flac),
		_ => None,
	};
	Ok(kind)
}

/// Lofty's error messages hide the interesting part (e.g. which frame was bad) in the
/// source chain; this joins the chain into one line.
fn describe(error: impl std::error::Error) -> String {
	let mut text = error.to_string();
	let mut source = error.source();
	while let Some(inner) = source {
		text.push_str(": ");
		text.push_str(&inner.to_string());
		source = inner.source();
	}
	text
}

pub fn read(file: &mut File, kind: Kind) -> Result<Info, String> {
	file.rewind().map_err(|e| e.to_string())?;
	let options = parse_options(true);
	let mut info = Info {
		kind,
		artist: None,
		title: None,
		album: None,
		duration_secs: None,
		lyrics: None,
		repaired: None,
	};
	match kind {
		Kind::Mp3 => {
			let mpeg = MpegFile::read_from(file, options).map_err(describe)?;
			let duration = mpeg.properties().duration().as_secs_f64();
			info.duration_secs = (duration > 0.0).then_some(duration);
			if let Some(tag) = mpeg.id3v2() {
				info.artist = clean(tag.artist());
				info.title = clean(tag.title());
				info.album = clean(tag.album());
				info.lyrics = tag
					.unsync_text()
					.map(|f| f.content.trim().to_string())
					.find(|l| !l.is_empty());
			}
			if let Some(tag) = mpeg.id3v1() {
				info.artist = info.artist.or_else(|| clean(tag.artist()));
				info.title = info.title.or_else(|| clean(tag.title()));
				info.album = info.album.or_else(|| clean(tag.album()));
			}
		},
		Kind::Flac => {
			let flac = FlacFile::read_from(file, options).map_err(describe)?;
			let duration = flac.properties().duration().as_secs_f64();
			info.duration_secs = (duration > 0.0).then_some(duration);
			if let Some(vc) = flac.vorbis_comments() {
				info.artist = clean(vc.artist());
				info.title = clean(vc.title());
				info.album = clean(vc.album());
				info.lyrics = VORBIS_LYRICS_KEYS
					.iter()
					.filter_map(|k| vc.get(k))
					.map(|l| l.trim().to_string())
					.find(|l| !l.is_empty());
			}
		},
	}
	for field in [&mut info.artist, &mut info.title, &mut info.album, &mut info.lyrics] {
		if let Some(how) = repair_field(field) {
			info.repaired.get_or_insert(how);
		}
	}
	Ok(info)
}

/// Writes (`Some`) or removes (`None`) the lyrics, leaving every other tag intact
/// (apart from encoding repairs when `fix_encoding` is set).
pub fn write_lyrics(
	file: &mut File,
	kind: Kind,
	lyrics: Option<&str>,
	fix_encoding: bool,
) -> Result<WriteReport, String> {
	let change = match lyrics {
		Some(text) => LyricsChange::Set(text.to_string()),
		None => LyricsChange::Remove,
	};
	write(file, kind, change, fix_encoding)
}

/// Rewrites the file's tags with the given lyrics change and, when `fix_encoding` is set,
/// with every mis-encoded text field repaired.
pub fn write(
	file: &mut File,
	kind: Kind,
	change: LyricsChange,
	fix_encoding: bool,
) -> Result<WriteReport, String> {
	let mut report = WriteReport::default();
	let options = parse_options(false);
	file.rewind().map_err(|e| e.to_string())?;
	match kind {
		Kind::Mp3 => {
			// Rescue the year before lofty's relaxed parser drops a malformed date frame
			let raw_year = raw_year(file)?;
			file.rewind().map_err(|e| e.to_string())?;
			let mut mpeg = MpegFile::read_from(&mut *file, options).map_err(describe)?;
			if mpeg.id3v2().is_none() {
				if change == LyricsChange::Keep && !fix_encoding {
					return Ok(report);
				}
				if change != LyricsChange::Keep {
					mpeg.set_id3v2(id3v2_from_v1(mpeg.id3v1(), fix_encoding));
				} else {
					return Ok(report);
				}
			}
			let use_v23 = mpeg
				.id3v2()
				.map(|t| t.original_version() == Id3v2Version::V3)
				.unwrap_or(false);
			let tag = mpeg.id3v2_mut().expect("tag was just ensured");
			match &change {
				LyricsChange::Keep => {},
				LyricsChange::Remove => tag.remove(&USLT).for_each(drop),
				LyricsChange::Set(text) => {
					tag.remove(&USLT).for_each(drop);
					tag.insert(Frame::UnsynchronizedText(UnsynchronizedTextFrame::new(
						TextEncoding::UTF8,
						*b"eng",
						"",
						text.clone(),
					)));
				},
			}
			if fix_encoding {
				report.repaired_fields = repair_id3v2(tag);
			}
			if let Some(year) = raw_year {
				if tag.get(&TDRC).is_none() {
					tag.insert(Frame::Timestamp(TimestampFrame::new(
						TDRC,
						TextEncoding::UTF8,
						Timestamp { year, ..Timestamp::default() },
					)));
					report.year_rescued = true;
				}
			}
			file.rewind().map_err(|e| e.to_string())?;
			mpeg.save_to(file, write_options(use_v23)).map_err(describe)?;
		},
		Kind::Flac => {
			let mut flac = FlacFile::read_from(&mut *file, options).map_err(describe)?;
			if flac.vorbis_comments().is_none() {
				if change == LyricsChange::Keep || change == LyricsChange::Remove {
					return Ok(report);
				}
				flac.set_vorbis_comments(VorbisComments::default());
			}
			let vc = flac.vorbis_comments_mut().expect("tag was just ensured");
			match &change {
				LyricsChange::Keep => {},
				LyricsChange::Remove => {
					for key in VORBIS_LYRICS_KEYS {
						vc.remove(key).for_each(drop);
					}
				},
				LyricsChange::Set(text) => {
					for key in VORBIS_LYRICS_KEYS {
						vc.remove(key).for_each(drop);
					}
					vc.insert("LYRICS".to_string(), text.clone());
				},
			}
			if fix_encoding {
				report.repaired_fields = repair_vorbis(vc);
			}
			file.rewind().map_err(|e| e.to_string())?;
			flac.save_to(file, write_options(false)).map_err(describe)?;
		},
	}
	Ok(report)
}

/// A fresh ID3v2 tag carrying the basic ID3v1 fields, so players that prefer v2 keep
/// seeing them.
fn id3v2_from_v1(v1: Option<&lofty::id3::v1::Id3v1Tag>, fix_encoding: bool) -> Id3v2Tag {
	let mut tag = Id3v2Tag::new();
	let Some(v1) = v1 else {
		return tag;
	};
	let text = |value: Option<Cow<'_, str>>| {
		clean(value).map(|v| if fix_encoding { encoding::fixed(&v) } else { v })
	};
	if let Some(v) = text(v1.title()) {
		tag.set_title(v);
	}
	if let Some(v) = text(v1.artist()) {
		tag.set_artist(v);
	}
	if let Some(v) = text(v1.album()) {
		tag.set_album(v);
	}
	if let Some(year) = v1.year {
		tag.insert(Frame::Timestamp(TimestampFrame::new(
			TDRC,
			TextEncoding::UTF8,
			Timestamp { year, ..Timestamp::default() },
		)));
	}
	if let Some(track) = v1.track_number {
		tag.set_track(u32::from(track));
	}
	tag
}

fn repair_cow(value: &mut Cow<'_, str>) -> bool {
	match encoding::repair(value) {
		Some((fixed, _)) => {
			*value = Cow::Owned(fixed);
			true
		},
		None => false,
	}
}

/// Repairs the text of every frame that can carry mis-encoded text; returns how many did.
/// A repaired frame is switched to Unicode, otherwise lofty would squeeze the Cyrillic text
/// back into the frame's Latin-1 encoding as question marks.
fn repair_id3v2(tag: &mut Id3v2Tag) -> usize {
	let mut count = 0;
	tag.retain_mut(|frame| {
		let (changed, encoding) = match frame {
			Frame::Text(f) => (repair_cow(&mut f.value), &mut f.encoding),
			Frame::Comment(f) => {
				let changed = repair_cow(&mut f.content) | repair_cow(&mut f.description);
				(changed, &mut f.encoding)
			},
			Frame::UnsynchronizedText(f) => {
				let changed = repair_cow(&mut f.content) | repair_cow(&mut f.description);
				(changed, &mut f.encoding)
			},
			Frame::UserText(f) => {
				let changed = repair_cow(&mut f.content) | repair_cow(&mut f.description);
				(changed, &mut f.encoding)
			},
			_ => return true,
		};
		if changed {
			*encoding = TextEncoding::UTF8;
			count += 1;
		}
		true
	});
	count
}

fn repair_vorbis(vc: &mut VorbisComments) -> usize {
	let items: Vec<(String, String)> = vc.items().map(|(k, v)| (k.to_string(), v.to_string())).collect();
	let fixed: Vec<(String, String, bool)> = items
		.into_iter()
		.map(|(k, v)| match encoding::repair(&v) {
			Some((f, _)) => (k, f, true),
			None => (k, v, false),
		})
		.collect();
	let count = fixed.iter().filter(|(_, _, changed)| *changed).count();
	if count > 0 {
		let keys: Vec<String> = fixed.iter().map(|(k, _, _)| k.clone()).collect();
		for key in &keys {
			vc.remove(key).for_each(drop);
		}
		for (k, v, _) in fixed {
			vc.push(k, v);
		}
	}
	count
}

/// Reads the year straight out of the raw ID3v2 header, for frames like `TYER "1997 / 1998"`
/// that lofty cannot parse as a timestamp.
fn raw_year(file: &mut File) -> Result<Option<u16>, String> {
	file.rewind().map_err(|e| e.to_string())?;
	let mut header = [0u8; 10];
	if file.read_exact(&mut header).is_err() || &header[..3] != b"ID3" {
		return Ok(None);
	}
	let version = header[3];
	let flags = header[5];
	let size = synchsafe(&header[6..10]) as usize;
	if !(2..=4).contains(&version) || flags & 0x80 != 0 || size > MAX_JUNK_BYTES {
		// v2.2 and unsynchronised tags are rare enough to leave to lofty
		return Ok(None);
	}
	let mut body = vec![0u8; size];
	if file.read_exact(&mut body).is_err() {
		return Ok(None);
	}
	let mut pos = 0usize;
	if flags & 0x40 != 0 && body.len() >= 4 {
		// skip the extended header
		pos = if version == 4 { synchsafe(&body[..4]) as usize } else { 4 + u32::from_be_bytes([body[0], body[1], body[2], body[3]]) as usize };
	}
	while pos + 10 <= body.len() {
		let id = &body[pos..pos + 4];
		if id == b"\0\0\0\0" {
			break;
		}
		let len = if version == 4 {
			synchsafe(&body[pos + 4..pos + 8]) as usize
		} else {
			u32::from_be_bytes([body[pos + 4], body[pos + 5], body[pos + 6], body[pos + 7]]) as usize
		};
		let start = pos + 10;
		let end = start.saturating_add(len).min(body.len());
		if matches!(id, b"TYER" | b"TDRC" | b"TDRL" | b"TORY" | b"TDOR") && end > start + 1 {
			let text = decode_text_frame(&body[start..end]);
			if let Some(year) = first_year(&text) {
				return Ok(Some(year));
			}
		}
		pos = end;
	}
	Ok(None)
}

fn synchsafe(bytes: &[u8]) -> u32 {
	bytes.iter().fold(0u32, |acc, b| (acc << 7) | u32::from(b & 0x7f))
}

fn decode_text_frame(body: &[u8]) -> String {
	match body[0] {
		0 | 3 => String::from_utf8_lossy(&body[1..]).into_owned(),
		1 | 2 => {
			let bytes = &body[1..];
			let (big_endian, data) = match bytes {
				[0xFF, 0xFE, rest @ ..] => (false, rest),
				[0xFE, 0xFF, rest @ ..] => (true, rest),
				_ => (body[0] == 2, bytes),
			};
			let units: Vec<u16> = data
				.chunks_exact(2)
				.map(|c| if big_endian { u16::from_be_bytes([c[0], c[1]]) } else { u16::from_le_bytes([c[0], c[1]]) })
				.collect();
			String::from_utf16_lossy(&units)
		},
		_ => String::new(),
	}
}

fn first_year(text: &str) -> Option<u16> {
	let digits: Vec<char> = text.chars().collect();
	digits
		.windows(4)
		.filter(|w| w.iter().all(|c| c.is_ascii_digit()))
		.map(|w| w.iter().collect::<String>().parse::<u16>().unwrap_or(0))
		.find(|y| (1000..=2999).contains(y))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn finds_the_first_year() {
		assert_eq!(first_year("1997 / 1998"), Some(1997));
		assert_eq!(first_year("12.04.2005"), Some(2005));
		assert_eq!(first_year("n/a"), None);
	}

	#[test]
	fn decodes_utf16_text_frames() {
		assert_eq!(decode_text_frame(b"\x01\xff\xfe1\x009\x00"), "19");
		assert_eq!(decode_text_frame(b"\x00abc"), "abc");
	}
}

//! Reading and writing lyrics in MP3 (ID3v2 `USLT`) and FLAC (Vorbis comment `LYRICS`) files.
//!
//! Real-world files are messy, so reading is tolerant: up to a few megabytes of junk between
//! the tag and the first audio frame are skipped, malformed frames are dropped instead of
//! failing the whole tag, a few header and frame flag bugs of old taggers are patched before
//! parsing (see [`raw`]), and Cyrillic text stored as CP1251 but labelled Latin-1 is repaired
//! (see [`crate::encoding`]).

use std::borrow::Cow;
use std::fs::File;
use std::io::{Cursor, Read, Seek, Write};

use lofty::config::{ParseOptions, ParsingMode, WriteOptions};
use lofty::file::FileType;
use lofty::flac::FlacFile;
use lofty::id3::v2::{
	Frame, FrameId, Id3v2Tag, Id3v2Version, TimestampFrame, UnsynchronizedTextFrame,
};
use lofty::io::FileLike;
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
	/// The tag header or frame flags had to be patched to parse the file at all; writing
	/// the file stores it properly.
	#[serde(default)]
	pub structure_fixed: bool,
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
	/// Header or frame bugs were patched (bogus flags, invalid comment languages)
	pub structure_fixed: bool,
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

/// The raw ID3v2 tag of an MP3 (header included), patched where needed, plus what was
/// found in it. `None` for files without an ID3v2 tag at the start.
struct RawTag {
	bytes: Vec<u8>,
	fixes: raw::Fixes,
	year: Option<u16>,
}

fn load_raw_tag(file: &mut File) -> Result<Option<RawTag>, String> {
	file.rewind().map_err(|e| e.to_string())?;
	let mut header = [0u8; 10];
	if file.read_exact(&mut header).is_err() || &header[..3] != b"ID3" {
		return Ok(None);
	}
	let size = raw::synchsafe(&header[6..10]) as usize;
	if size > MAX_JUNK_BYTES {
		return Ok(None);
	}
	let mut bytes = header.to_vec();
	bytes.resize(10 + size, 0);
	// a truncated tag is left to lofty
	let mut filled = 0;
	while filled < size {
		match file.read(&mut bytes[10 + filled..]) {
			Ok(0) => break,
			Ok(n) => filled += n,
			Err(e) => return Err(e.to_string()),
		}
	}
	bytes.truncate(10 + filled);
	let fixes = raw::sanitize(&mut bytes);
	let year = raw::year(&bytes);
	Ok(Some(RawTag { bytes, fixes, year }))
}

/// Loads the whole file into memory with the patched tag in front, so lofty can parse it.
fn patched_copy(file: &mut File, tag: &RawTag) -> Result<Cursor<Vec<u8>>, String> {
	file.rewind().map_err(|e| e.to_string())?;
	let mut data = Vec::new();
	file.read_to_end(&mut data).map_err(|e| e.to_string())?;
	let n = tag.bytes.len().min(data.len());
	data[..n].copy_from_slice(&tag.bytes[..n]);
	Ok(Cursor::new(data))
}

pub fn read(file: &mut File, kind: Kind) -> Result<Info, String> {
	let raw = match kind {
		Kind::Mp3 => load_raw_tag(file)?,
		Kind::Flac => None,
	};
	let mut info = match &raw {
		Some(tag) if tag.fixes.any() => {
			let mut copy = patched_copy(file, tag)?;
			read_from(&mut copy, kind)?
		},
		_ => read_from(file, kind)?,
	};
	info.structure_fixed = raw.as_ref().map(|t| t.fixes.any()).unwrap_or(false);
	for field in [&mut info.artist, &mut info.title, &mut info.album, &mut info.lyrics] {
		if let Some(how) = repair_field(field) {
			info.repaired.get_or_insert(how);
		}
	}
	Ok(info)
}

fn read_from<R: Read + Seek>(reader: &mut R, kind: Kind) -> Result<Info, String> {
	reader.rewind().map_err(|e| e.to_string())?;
	let options = parse_options(true);
	let mut info = Info {
		kind,
		artist: None,
		title: None,
		album: None,
		duration_secs: None,
		lyrics: None,
		repaired: None,
		structure_fixed: false,
	};
	match kind {
		Kind::Mp3 => {
			let mpeg = MpegFile::read_from(reader, options).map_err(describe)?;
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
			let flac = FlacFile::read_from(reader, options).map_err(describe)?;
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
	Ok(info)
}

/// Writes (`Some`) or removes (`None`) the lyrics, leaving every other tag intact
/// (apart from repairs when `fix_encoding` is set).
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
/// with every mis-encoded text field repaired. Structural defects are always patched,
/// since the file could not be written back otherwise.
pub fn write(
	file: &mut File,
	kind: Kind,
	change: LyricsChange,
	fix_encoding: bool,
) -> Result<WriteReport, String> {
	let raw = match kind {
		Kind::Mp3 => load_raw_tag(file)?,
		Kind::Flac => None,
	};
	let year = raw.as_ref().and_then(|t| t.year);
	match &raw {
		Some(tag) if tag.fixes.any() => {
			let mut copy = patched_copy(file, tag)?;
			let mut report = write_to(&mut copy, kind, change, fix_encoding, year)?;
			let data = copy.into_inner();
			file.rewind().map_err(|e| e.to_string())?;
			file.set_len(data.len() as u64).map_err(|e| e.to_string())?;
			file.write_all(&data).map_err(|e| e.to_string())?;
			report.structure_fixed = true;
			Ok(report)
		},
		_ => write_to(file, kind, change, fix_encoding, year),
	}
}

fn write_to<F: FileLike>(
	file: &mut F,
	kind: Kind,
	change: LyricsChange,
	fix_encoding: bool,
	raw_year: Option<u16>,
) -> Result<WriteReport, String> {
	let mut report = WriteReport::default();
	let options = parse_options(false);
	file.rewind().map_err(|e| e.to_string())?;
	match kind {
		Kind::Mp3 => {
			let mut mpeg = MpegFile::read_from(&mut *file, options).map_err(describe)?;
			if mpeg.id3v2().is_none() {
				if change == LyricsChange::Keep || change == LyricsChange::Remove {
					return Ok(report);
				}
				mpeg.set_id3v2(id3v2_from_v1(mpeg.id3v1(), fix_encoding));
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
			report.structure_fixed |= fix_languages(tag);
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

/// Comment and lyrics frames must carry a three-letter language, or lofty refuses to write
/// them; some taggers leave zeros there. Empty comments with a bad language are dropped.
fn fix_languages(tag: &mut Id3v2Tag) -> bool {
	let mut fixed = false;
	let valid = |lang: &[u8; 3]| lang.iter().all(|b| b.is_ascii_alphabetic());
	tag.retain_mut(|frame| match frame {
		Frame::Comment(f) if !valid(&f.language) => {
			fixed = true;
			if f.content.trim().is_empty() && f.description.trim().is_empty() {
				return false;
			}
			f.language = *b"und";
			true
		},
		Frame::UnsynchronizedText(f) if !valid(&f.language) => {
			fixed = true;
			f.language = *b"und";
			true
		},
		_ => true,
	});
	fixed
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

/// Byte-level inspection and patching of an ID3v2 tag, for the bugs lofty cannot get past.
pub mod raw {
	/// Which patches [`sanitize`] applied.
	#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
	pub struct Fixes {
		/// The header claimed an extended header that is not there
		pub bogus_extended_header: bool,
		/// Frames claimed a data length indicator (v2.4) that is not there
		pub bogus_length_indicators: usize,
	}

	impl Fixes {
		pub fn any(&self) -> bool {
			self.bogus_extended_header || self.bogus_length_indicators > 0
		}
	}

	pub fn synchsafe(bytes: &[u8]) -> u32 {
		bytes.iter().fold(0u32, |acc, b| (acc << 7) | u32::from(b & 0x7f))
	}

	fn is_frame_id(id: &[u8]) -> bool {
		id.len() == 4 && id.iter().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
	}

	struct Header {
		version: u8,
		frames_start: usize,
	}

	/// Parses the tag header; `None` for v2.2, unsynchronised v2.3 and anything odd.
	fn header(tag: &[u8]) -> Option<Header> {
		if tag.len() < 10 || &tag[..3] != b"ID3" {
			return None;
		}
		let version = tag[3];
		let flags = tag[5];
		if !(3..=4).contains(&version) || (version == 3 && flags & 0x80 != 0) {
			return None;
		}
		let mut frames_start = 10;
		if flags & 0x40 != 0 {
			let ext = if version == 4 {
				synchsafe(tag.get(10..14)?) as usize
			} else {
				4 + u32::from_be_bytes(tag.get(10..14)?.try_into().ok()?) as usize
			};
			frames_start += ext;
		}
		Some(Header { version, frames_start })
	}

	fn frame_size(tag: &[u8], pos: usize, version: u8) -> usize {
		if version == 4 {
			synchsafe(&tag[pos + 4..pos + 8]) as usize
		} else {
			u32::from_be_bytes([tag[pos + 4], tag[pos + 5], tag[pos + 6], tag[pos + 7]]) as usize
		}
	}

	/// Patches the known header and frame flag bugs in place and reports what it did.
	pub fn sanitize(tag: &mut [u8]) -> Fixes {
		let mut fixes = Fixes::default();
		if tag.len() < 14 || &tag[..3] != b"ID3" {
			return fixes;
		}
		// An extended-header flag directly followed by a frame id: the header is not there.
		if tag[5] & 0x40 != 0 && is_frame_id(&tag[10..14]) {
			tag[5] &= !0x40;
			fixes.bogus_extended_header = true;
		}
		let Some(header) = header(tag) else {
			return fixes;
		};
		let mut pos = header.frames_start;
		while pos + 10 <= tag.len() {
			if !is_frame_id(&tag[pos..pos + 4]) {
				break;
			}
			let size = frame_size(tag, pos, header.version);
			let end = (pos + 10).saturating_add(size).min(tag.len());
			let format_flags = tag[pos + 9];
			// v2.4: a data length indicator must be a synchsafe number of plausible size
			if header.version == 4 && format_flags & 0x01 != 0 && format_flags & 0x08 == 0 {
				let body = &tag[pos + 10..end];
				let plausible = body.len() >= 4
					&& body[..4].iter().all(|b| *b < 0x80)
					&& (synchsafe(&body[..4]) as usize) <= size.saturating_mul(2) + 16;
				if !plausible {
					tag[pos + 9] &= !0x01;
					fixes.bogus_length_indicators += 1;
				}
			}
			pos = end;
		}
		fixes
	}

	/// The year straight out of the raw tag, for frames like `TYER "1997 / 1998"` that lofty
	/// cannot parse as a timestamp.
	pub fn year(tag: &[u8]) -> Option<u16> {
		let header = header(tag)?;
		let mut pos = header.frames_start;
		while pos + 10 <= tag.len() {
			let id = &tag[pos..pos + 4];
			if !is_frame_id(id) {
				break;
			}
			let size = frame_size(tag, pos, header.version);
			let start = pos + 10;
			let end = start.saturating_add(size).min(tag.len());
			if matches!(id, b"TYER" | b"TDRC" | b"TDRL" | b"TORY" | b"TDOR") && end > start + 1 {
				if let Some(year) = first_year(&decode_text_frame(&tag[start..end])) {
					return Some(year);
				}
			}
			pos = end;
		}
		None
	}

	pub(super) fn decode_text_frame(body: &[u8]) -> String {
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

	pub(super) fn first_year(text: &str) -> Option<u16> {
		let chars: Vec<char> = text.chars().collect();
		chars
			.windows(4)
			.filter(|w| w.iter().all(|c| c.is_ascii_digit()))
			.map(|w| w.iter().collect::<String>().parse::<u16>().unwrap_or(0))
			.find(|y| (1000..=2999).contains(y))
	}
}

#[cfg(test)]
mod tests {
	use super::raw::*;

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

	#[test]
	fn patches_bogus_extended_header_flag() {
		let mut tag = b"ID3\x03\x00\x40\x00\x00\x00\x10TIT2\x00\x00\x00\x06\x00\x00\x00hello".to_vec();
		let fixes = sanitize(&mut tag);
		assert!(fixes.bogus_extended_header);
		assert_eq!(tag[5], 0);
		// a genuine extended header (size bytes, not a frame id) is left alone
		let mut tag = b"ID3\x03\x00\x40\x00\x00\x00\x10\x00\x00\x00\x06\x00\x00\x00\x00\x00\x00".to_vec();
		assert_eq!(sanitize(&mut tag), Fixes::default());
	}

	#[test]
	fn patches_bogus_data_length_indicators() {
		let mut tag = b"ID3\x04\x00\x00\x00\x00\x00\x12TPE1\x00\x00\x00\x08\x00\x03\x01\xff\xfeA\x00b\x00".to_vec();
		let fixes = sanitize(&mut tag);
		assert_eq!(fixes.bogus_length_indicators, 1);
		assert_eq!(tag[19], 0x02);
		// a real indicator (synchsafe, sensible) survives
		let mut tag = b"ID3\x04\x00\x00\x00\x00\x00\x12TPE1\x00\x00\x00\x08\x00\x03\x00\x00\x00\x04\x00abc".to_vec();
		assert_eq!(sanitize(&mut tag).bogus_length_indicators, 0);
	}
}

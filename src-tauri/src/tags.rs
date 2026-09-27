//! Reading and writing lyrics in MP3 (ID3v2 `USLT`) and FLAC (Vorbis comment `LYRICS`) files.

use std::borrow::Cow;
use std::fs::File;
use std::io::Seek;

use lofty::config::{ParseOptions, WriteOptions};
use lofty::file::FileType;
use lofty::flac::FlacFile;
use lofty::id3::v2::{Frame, FrameId, Id3v2Tag, Id3v2Version, TextInformationFrame, UnsynchronizedTextFrame};
use lofty::mpeg::MpegFile;
use lofty::ogg::tag::VorbisComments;
use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::TextEncoding;
use serde::Serialize;

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
}

const USLT: FrameId<'static> = FrameId::Valid(Cow::Borrowed("USLT"));
const VORBIS_LYRICS_KEYS: [&str; 2] = ["LYRICS", "UNSYNCEDLYRICS"];

fn clean(value: Option<Cow<'_, str>>) -> Option<String> {
	value
		.map(|v| v.trim().to_string())
		.filter(|v| !v.is_empty())
}

/// Detects whether the file is an MP3 or a FLAC by its contents.
pub fn probe(file: &mut File) -> Result<Option<Kind>, String> {
	file.rewind().map_err(|e| e.to_string())?;
	let probe = Probe::new(&mut *file)
		.guess_file_type()
		.map_err(|e| e.to_string())?;
	let kind = match probe.file_type() {
		Some(FileType::Mpeg) => Some(Kind::Mp3),
		Some(FileType::Flac) => Some(Kind::Flac),
		_ => None,
	};
	Ok(kind)
}

pub fn read(file: &mut File, kind: Kind) -> Result<Info, String> {
	file.rewind().map_err(|e| e.to_string())?;
	let options = ParseOptions::new().read_properties(true);
	match kind {
		Kind::Mp3 => {
			let mpeg = MpegFile::read_from(file, options).map_err(|e| e.to_string())?;
			let duration = mpeg.properties().duration().as_secs_f64();
			let mut info = Info {
				kind,
				artist: None,
				title: None,
				album: None,
				duration_secs: (duration > 0.0).then_some(duration),
				lyrics: None,
			};
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
			Ok(info)
		},
		Kind::Flac => {
			let flac = FlacFile::read_from(file, options).map_err(|e| e.to_string())?;
			let duration = flac.properties().duration().as_secs_f64();
			let mut info = Info {
				kind,
				artist: None,
				title: None,
				album: None,
				duration_secs: (duration > 0.0).then_some(duration),
				lyrics: None,
			};
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
			Ok(info)
		},
	}
}

/// Writes (`Some`) or removes (`None`) the lyrics of the file, leaving every other tag intact.
pub fn write_lyrics(file: &mut File, kind: Kind, lyrics: Option<&str>) -> Result<(), String> {
	file.rewind().map_err(|e| e.to_string())?;
	let options = ParseOptions::new().read_properties(false);
	match kind {
		Kind::Mp3 => {
			let mut mpeg = MpegFile::read_from(&mut *file, options).map_err(|e| e.to_string())?;
			if mpeg.id3v2().is_none() {
				if lyrics.is_none() {
					return Ok(());
				}
				// Promote ID3v1 fields so players that prefer v2 keep seeing them
				let mut tag = Id3v2Tag::new();
				if let Some(v1) = mpeg.id3v1() {
					if let Some(v) = clean(v1.title()) {
						tag.set_title(v);
					}
					if let Some(v) = clean(v1.artist()) {
						tag.set_artist(v);
					}
					if let Some(v) = clean(v1.album()) {
						tag.set_album(v);
					}
					if let Some(v) = v1.year {
						tag.insert(Frame::Text(TextInformationFrame::new(
							FrameId::Valid(Cow::Borrowed("TDRC")),
							TextEncoding::UTF8,
							v.to_string(),
						)));
					}
					if let Some(v) = v1.track_number {
						tag.set_track(v as u32);
					}
				}
				mpeg.set_id3v2(tag);
			}
			let use_v23 = mpeg
				.id3v2()
				.map(|t| t.original_version() == Id3v2Version::V3)
				.unwrap_or(false);
			let tag = mpeg.id3v2_mut().expect("tag was just ensured");
			tag.remove(&USLT).for_each(drop);
			if let Some(text) = lyrics {
				tag.insert(Frame::UnsynchronizedText(UnsynchronizedTextFrame::new(
					TextEncoding::UTF8,
					*b"eng",
					"",
					text.to_string(),
				)));
			}
			file.rewind().map_err(|e| e.to_string())?;
			let mut write_options = WriteOptions::default();
			if use_v23 {
				write_options = write_options.use_id3v23(true);
			}
			mpeg.save_to(file, write_options).map_err(|e| e.to_string())
		},
		Kind::Flac => {
			let mut flac = FlacFile::read_from(&mut *file, options).map_err(|e| e.to_string())?;
			if flac.vorbis_comments().is_none() {
				if lyrics.is_none() {
					return Ok(());
				}
				flac.set_vorbis_comments(VorbisComments::default());
			}
			let vc = flac.vorbis_comments_mut().expect("tag was just ensured");
			for key in VORBIS_LYRICS_KEYS {
				vc.remove(key).for_each(drop);
			}
			if let Some(text) = lyrics {
				vc.insert("LYRICS".to_string(), text.to_string());
			}
			file.rewind().map_err(|e| e.to_string())?;
			flac.save_to(file, WriteOptions::default()).map_err(|e| e.to_string())
		},
	}
}

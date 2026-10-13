//! Debugging aid: prints what lofty makes of each file given on the command line.
//!
//! `inspect [--keep|--set|--remove] FILE...` rewrites each file first (encoding repair on)
//! and then dumps the ID3v2 frames with their encodings.
use std::fs::File;

use deepkick_core::tags::{self, Kind, LyricsChange};
use lofty::config::ParseOptions;
use lofty::file::AudioFile;
use lofty::id3::v2::Frame;
use lofty::mpeg::MpegFile;
use lofty::probe::Probe;

fn main() {
	let mut change: Option<LyricsChange> = None;
	for arg in std::env::args().skip(1) {
		match arg.as_str() {
			"--keep" => change = Some(LyricsChange::Keep),
			"--set" => change = Some(LyricsChange::Set("placeholder line".into())),
			"--remove" => change = Some(LyricsChange::Remove),
			path => inspect(path, change.clone()),
		}
	}
}

fn inspect(path: &str, change: Option<LyricsChange>) {
	println!("== {path}");
	let mut file = File::options().read(true).write(true).open(path).expect("open");
	if let Some(change) = change {
		match tags::probe(&mut file).expect("probe") {
			Some(kind) => match tags::write(&mut file, kind, change, true) {
				Ok(report) => println!("write: {report:?}"),
				Err(e) => println!("write error: {e}"),
			},
			None => println!("unsupported"),
		}
	}
	use std::io::Seek;
	file.rewind().unwrap();
	let mut options = ParseOptions::new().max_junk_bytes(1 << 20);
	match Probe::new(&mut file).options(options).guess_file_type() {
		Ok(p) => println!("probe: {:?}", p.file_type()),
		Err(e) => println!("probe error: {e:?}"),
	}
	file.rewind().unwrap();
	match MpegFile::read_from(&mut file, options.read_properties(false)) {
		Ok(f) => {
			println!("ok: id3v2={} id3v1={}", f.id3v2().is_some(), f.id3v1().is_some());
			if let Some(tag) = f.id3v2() {
				for frame in tag.iter() {
					match frame {
						Frame::Text(t) => println!("  {} {:?} {:?}", t.id(), t.encoding, t.value),
						Frame::UnsynchronizedText(t) => {
							println!("  USLT {:?} {} chars", t.encoding, t.content.chars().count())
						},
						Frame::Comment(c) => println!("  COMM {:?} {:?}", c.encoding, c.content),
						other => println!("  {}", other.id()),
					}
				}
			}
		},
		Err(e) => {
			println!("read error: {e}");
			let mut source = std::error::Error::source(&e);
			while let Some(inner) = source {
				println!("  caused by: {inner}");
				source = inner.source();
			}
		},
	}
	let _ = Kind::Mp3;
}

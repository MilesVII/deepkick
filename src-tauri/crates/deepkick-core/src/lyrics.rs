//! Lyrics providers: LRCLIB (primary) and lyrics.ovh (optional fallback).

use std::time::Duration;

use serde::{Deserialize, Serialize};

const LRCLIB: &str = "https://lrclib.net/api";
const LYRICS_OVH: &str = "https://api.lyrics.ovh/v1";
const USER_AGENT: &str = concat!(
	"DeepKick Lyrics/",
	env!("CARGO_PKG_VERSION"),
	" (https://github.com/milesvii/slm-dkl)"
);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
	pub id: i64,
	pub artist: String,
	pub title: String,
	pub album: String,
	pub duration: f64,
	pub instrumental: bool,
	pub plain: Option<String>,
	pub synced: Option<String>,
	pub source: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrclibTrack {
	id: i64,
	#[serde(default)]
	track_name: Option<String>,
	#[serde(default)]
	artist_name: Option<String>,
	#[serde(default)]
	album_name: Option<String>,
	#[serde(default)]
	duration: Option<f64>,
	#[serde(default)]
	instrumental: bool,
	#[serde(default)]
	plain_lyrics: Option<String>,
	#[serde(default)]
	synced_lyrics: Option<String>,
}

impl From<LrclibTrack> for Candidate {
	fn from(t: LrclibTrack) -> Self {
		let nonempty = |v: Option<String>| v.filter(|s| !s.trim().is_empty());
		Candidate {
			id: t.id,
			artist: t.artist_name.unwrap_or_default(),
			title: t.track_name.unwrap_or_default(),
			album: t.album_name.unwrap_or_default(),
			duration: t.duration.unwrap_or(0.0),
			instrumental: t.instrumental,
			plain: nonempty(t.plain_lyrics),
			synced: nonempty(t.synced_lyrics),
			source: "lrclib".into(),
		}
	}
}

#[derive(Debug, Clone, Deserialize)]
struct OvhResponse {
	#[serde(default)]
	lyrics: Option<String>,
}

pub enum Found {
	Instrumental,
	Lyrics { text: String, source: String },
}

pub struct Client {
	agent: ureq::Agent,
}

impl Default for Client {
	fn default() -> Self {
		Self::new()
	}
}

/// Strips trailing "(feat. ...)", "[Remastered]" and similar decorations.
pub fn simplify_title(title: &str) -> Option<String> {
	let cut = title
		.find(['(', '['])
		.map(|i| title[..i].trim())
		.filter(|s| !s.is_empty())?;
	(cut != title.trim()).then(|| cut.to_string())
}

impl Client {
	pub fn new() -> Self {
		let config = ureq::Agent::config_builder()
			.timeout_global(Some(Duration::from_secs(30)))
			.user_agent(USER_AGENT)
			.http_status_as_error(false)
			.build();
		Client { agent: config.new_agent() }
	}

	/// GET with a few retries on transport errors and transient HTTP statuses (429, 5xx).
	fn get(
		&self,
		provider: &str,
		url: &str,
		query: &[(&str, String)],
	) -> Result<http::Response<ureq::Body>, String> {
		const ATTEMPTS: u32 = 3;
		let mut last_error = String::new();
		for attempt in 1..=ATTEMPTS {
			let mut req = self.agent.get(url).header("Lrclib-Client", USER_AGENT);
			for (key, value) in query {
				req = req.query(*key, value);
			}
			match req.call() {
				Ok(response) => {
					let status = response.status().as_u16();
					if status != 429 && status < 500 {
						return Ok(response);
					}
					last_error = format!("{provider}: HTTP {status}");
				},
				Err(e) => last_error = format!("{provider}: {e}"),
			}
			if attempt < ATTEMPTS {
				log::warn!("{last_error}, retrying ({attempt}/{ATTEMPTS})");
				std::thread::sleep(Duration::from_millis(700 * 2u64.pow(attempt - 1)));
			}
		}
		Err(last_error)
	}

	fn lrclib_get(
		&self,
		artist: &str,
		title: &str,
		album: Option<&str>,
		duration: Option<f64>,
	) -> Result<Option<Candidate>, String> {
		let mut query: Vec<(&str, String)> = vec![
			("artist_name", artist.to_string()),
			("track_name", title.to_string()),
		];
		if let Some(album) = album {
			query.push(("album_name", album.to_string()));
		}
		if let Some(duration) = duration {
			query.push(("duration", format!("{}", duration.round() as i64)));
		}
		let mut response = self.get("lrclib", &format!("{LRCLIB}/get"), &query)?;
		match response.status().as_u16() {
			200 => {
				let track: LrclibTrack = response
					.body_mut()
					.read_json()
					.map_err(|e| format!("lrclib: bad response: {e}"))?;
				Ok(Some(track.into()))
			},
			404 => Ok(None),
			code => Err(format!("lrclib: HTTP {code}")),
		}
	}

	pub fn lrclib_search(&self, artist: &str, title: &str) -> Result<Vec<Candidate>, String> {
		let query = [
			("artist_name", artist.to_string()),
			("track_name", title.to_string()),
		];
		let mut response = self.get("lrclib", &format!("{LRCLIB}/search"), &query)?;
		if response.status().as_u16() != 200 {
			return Err(format!("lrclib: HTTP {}", response.status().as_u16()));
		}
		let tracks: Vec<LrclibTrack> = response
			.body_mut()
			.read_json()
			.map_err(|e| format!("lrclib: bad response: {e}"))?;
		Ok(tracks.into_iter().map(Candidate::from).collect())
	}

	pub fn ovh(&self, artist: &str, title: &str) -> Result<Option<String>, String> {
		let url = format!(
			"{LYRICS_OVH}/{}/{}",
			urlencode(artist),
			urlencode(title)
		);
		let mut response = self.get("lyrics.ovh", &url, &[])?;
		match response.status().as_u16() {
			200 => {
				let body: OvhResponse = response
					.body_mut()
					.read_json()
					.map_err(|e| format!("lyrics.ovh: bad response: {e}"))?;
				Ok(body
					.lyrics
					.map(|l| l.replace("\r\n", "\n").trim().to_string())
					.filter(|l| !l.is_empty()))
			},
			404 => Ok(None),
			code => Err(format!("lyrics.ovh: HTTP {code}")),
		}
	}

	/// Full search used by the manual editor: exact lookup first, then the fuzzy search,
	/// with the simplified title as a last attempt.
	pub fn search(
		&self,
		artist: &str,
		title: &str,
		album: Option<&str>,
		duration: Option<f64>,
	) -> Result<Vec<Candidate>, String> {
		let mut out: Vec<Candidate> = vec![];
		if let Some(exact) = self.lrclib_get(artist, title, album, duration)? {
			out.push(exact);
		}
		for c in self.lrclib_search(artist, title)? {
			if !out.iter().any(|o| o.id == c.id) {
				out.push(c);
			}
		}
		if out.is_empty() {
			if let Some(simple) = simplify_title(title) {
				out.extend(self.lrclib_search(artist, &simple)?);
			}
		}
		Ok(out)
	}

	fn pick_text(candidate: &Candidate, prefer_synced: bool) -> Option<String> {
		if prefer_synced {
			candidate.synced.clone().or_else(|| candidate.plain.clone())
		} else {
			candidate.plain.clone().or_else(|| candidate.synced.clone())
		}
	}

	/// Chooses the best match from a search result list: the candidate with lyrics whose
	/// duration is closest to the file's (within 10 s when the duration is known).
	fn best<'a>(candidates: &'a [Candidate], duration: Option<f64>) -> Option<&'a Candidate> {
		let usable = candidates
			.iter()
			.filter(|c| c.instrumental || c.plain.is_some() || c.synced.is_some());
		match duration {
			Some(d) if d > 0.0 => usable
				.filter(|c| c.duration <= 0.0 || (c.duration - d).abs() <= 10.0)
				.min_by(|a, b| {
					let da = if a.duration > 0.0 { (a.duration - d).abs() } else { 10.0 };
					let db = if b.duration > 0.0 { (b.duration - d).abs() } else { 10.0 };
					da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
				}),
			_ => usable.min_by_key(|c| c.instrumental as u8),
		}
	}

	/// Automatic lookup used by the queue worker.
	pub fn find(
		&self,
		artist: &str,
		title: &str,
		album: Option<&str>,
		duration: Option<f64>,
		prefer_synced: bool,
		use_fallback: bool,
	) -> Result<Option<Found>, String> {
		let to_found = |c: &Candidate| -> Option<Found> {
			if c.instrumental {
				return Some(Found::Instrumental);
			}
			Self::pick_text(c, prefer_synced).map(|text| Found::Lyrics {
				text,
				source: format!("{}: {} — {}", c.source, c.artist, c.title),
			})
		};

		// 1. exact lookup (with duration for disambiguation, then without)
		if let Some(c) = self.lrclib_get(artist, title, album, duration)? {
			if let Some(found) = to_found(&c) {
				return Ok(Some(found));
			}
		}
		if duration.is_some() {
			if let Some(c) = self.lrclib_get(artist, title, None, None)? {
				if let Some(found) = to_found(&c) {
					return Ok(Some(found));
				}
			}
		}

		// 2. fuzzy search, then with the simplified title
		let mut titles = vec![title.to_string()];
		if let Some(simple) = simplify_title(title) {
			titles.push(simple);
		}
		for t in &titles {
			let candidates = self.lrclib_search(artist, t)?;
			if let Some(best) = Self::best(&candidates, duration) {
				if let Some(found) = to_found(best) {
					return Ok(Some(found));
				}
			}
		}

		// 3. secondary provider
		if use_fallback {
			for t in &titles {
				if let Some(text) = self.ovh(artist, t)? {
					return Ok(Some(Found::Lyrics { text, source: "lyrics.ovh".into() }));
				}
			}
		}

		Ok(None)
	}
}

fn urlencode(input: &str) -> String {
	let mut out = String::with_capacity(input.len() * 3);
	for byte in input.bytes() {
		match byte {
			b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
			_ => out.push_str(&format!("%{byte:02X}")),
		}
	}
	out
}

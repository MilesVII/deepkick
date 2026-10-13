//! Settings stored in `<config dir>/deepkick-lyrics/config.toml`
//! (`DEEPKICK_CONFIG` overrides the location).

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use deepkick_core::RunOptions;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Config {
	/// Replace lyrics that are already embedded
	pub overwrite: bool,
	/// Prefer synchronised (LRC) lyrics when LRCLIB has them
	pub prefer_synced: bool,
	/// Try lyrics.ovh when LRCLIB has nothing
	pub fallback: bool,
	/// Repair tags stored in the wrong character set (CP1251 / UTF-8 read as Latin-1)
	pub fix_encoding: bool,
	/// Command used to edit lyrics; the file path is appended (like git's core.editor)
	pub editor: Option<String>,
	/// Folder the file browser opened last
	pub last_dir: Option<PathBuf>,
}

impl Default for Config {
	fn default() -> Self {
		Config {
			overwrite: false,
			prefer_synced: false,
			fallback: true,
			fix_encoding: true,
			editor: None,
			last_dir: None,
		}
	}
}

pub const KEYS: [&str; 5] = ["overwrite", "prefer-synced", "fallback", "fix-encoding", "editor"];

fn parse_bool(value: &str) -> Result<bool> {
	match value.trim().to_ascii_lowercase().as_str() {
		"true" | "yes" | "on" | "1" => Ok(true),
		"false" | "no" | "off" | "0" => Ok(false),
		other => bail!("expected true or false, got `{other}`"),
	}
}

impl Config {
	pub fn path() -> Option<PathBuf> {
		if let Some(path) = std::env::var_os("DEEPKICK_CONFIG") {
			return Some(PathBuf::from(path));
		}
		Some(dirs::config_dir()?.join("deepkick-lyrics").join("config.toml"))
	}

	pub fn load() -> Result<Config> {
		let Some(path) = Self::path() else {
			return Ok(Config::default());
		};
		match std::fs::read_to_string(&path) {
			Ok(text) => toml::from_str(&text)
				.with_context(|| format!("cannot parse the settings file {}", path.display())),
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
			Err(e) => Err(e).with_context(|| format!("cannot read {}", path.display())),
		}
	}

	pub fn save(&self) -> Result<()> {
		let path = Self::path().context("cannot find a settings folder")?;
		if let Some(dir) = path.parent() {
			std::fs::create_dir_all(dir)
				.with_context(|| format!("cannot create {}", dir.display()))?;
		}
		let text = toml::to_string_pretty(self).context("cannot serialize the settings")?;
		std::fs::write(&path, text).with_context(|| format!("cannot write {}", path.display()))
	}

	pub fn run_options(&self) -> RunOptions {
		RunOptions {
			overwrite: self.overwrite,
			prefer_synced: self.prefer_synced,
			use_fallback: self.fallback,
			fix_encoding: self.fix_encoding,
		}
	}

	pub fn get(&self, key: &str) -> Result<String> {
		Ok(match key {
			"overwrite" => self.overwrite.to_string(),
			"prefer-synced" => self.prefer_synced.to_string(),
			"fallback" => self.fallback.to_string(),
			"fix-encoding" => self.fix_encoding.to_string(),
			"editor" => self.editor.clone().unwrap_or_default(),
			_ => bail!("unknown setting `{key}`; known settings: {}", KEYS.join(", ")),
		})
	}

	pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
		match key {
			"overwrite" => self.overwrite = parse_bool(value)?,
			"prefer-synced" => self.prefer_synced = parse_bool(value)?,
			"fallback" => self.fallback = parse_bool(value)?,
			"fix-encoding" => self.fix_encoding = parse_bool(value)?,
			"editor" => {
				let value = value.trim();
				self.editor = (!value.is_empty()).then(|| value.to_string());
			},
			_ => bail!("unknown setting `{key}`; known settings: {}", KEYS.join(", ")),
		}
		Ok(())
	}
}

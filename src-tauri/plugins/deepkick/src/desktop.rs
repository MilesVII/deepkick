//! Desktop stand-in used for development: native dialogs, plain paths, log-only notifications.

use std::fs::File;

use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime};

use crate::models::*;

pub fn init<R: Runtime, C: DeserializeOwned>(
	app: &AppHandle<R>,
	_api: PluginApi<R, C>,
) -> crate::Result<Deepkick<R>> {
	Ok(Deepkick(app.clone()))
}

pub struct Deepkick<R: Runtime>(#[allow(dead_code)] AppHandle<R>);

fn is_supported(name: &str) -> bool {
	let lower = name.to_ascii_lowercase();
	lower.ends_with(".mp3") || lower.ends_with(".flac")
}

fn describe(path: &std::path::Path, folder: String) -> PickedFile {
	PickedFile {
		uri: path.to_string_lossy().into_owned(),
		name: path
			.file_name()
			.map(|n| n.to_string_lossy().into_owned())
			.unwrap_or_default(),
		folder,
		size: std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
		mime: None,
	}
}

impl<R: Runtime> Deepkick<R> {
	pub fn pick_files(&self) -> crate::Result<Vec<PickedFile>> {
		let picked = rfd::FileDialog::new()
			.add_filter("Audio", &["mp3", "flac"])
			.pick_files()
			.unwrap_or_default();
		Ok(picked
			.iter()
			.map(|p| {
				let folder = p
					.parent()
					.map(|d| d.to_string_lossy().into_owned())
					.unwrap_or_default();
				describe(p, folder)
			})
			.collect())
	}

	pub fn pick_directory(&self) -> crate::Result<Vec<PickedFile>> {
		let Some(root) = rfd::FileDialog::new().pick_folder() else {
			return Ok(vec![]);
		};
		let root_name = root
			.file_name()
			.map(|n| n.to_string_lossy().into_owned())
			.unwrap_or_default();
		let mut out = vec![];
		for entry in walkdir::WalkDir::new(&root).into_iter().flatten() {
			if !entry.file_type().is_file() {
				continue;
			}
			let name = entry.file_name().to_string_lossy();
			if !is_supported(&name) {
				continue;
			}
			let rel = entry
				.path()
				.parent()
				.and_then(|p| p.strip_prefix(&root).ok())
				.map(|p| p.to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/"))
				.unwrap_or_default();
			let folder = if rel.is_empty() {
				root_name.clone()
			} else {
				format!("{root_name}/{rel}")
			};
			out.push(describe(entry.path(), folder));
		}
		out.sort_by(|a, b| a.folder.cmp(&b.folder).then(a.name.cmp(&b.name)));
		Ok(out)
	}

	pub fn open_file(&self, uri: &str, write: bool) -> crate::Result<File> {
		Ok(File::options().read(true).write(write).open(uri)?)
	}

	pub fn service_start(&self, n: &ProgressNotification) -> crate::Result<()> {
		log::info!("[service start] {} / {} ({}/{})", n.title, n.text, n.progress, n.max);
		Ok(())
	}

	pub fn service_update(&self, n: &ProgressNotification) -> crate::Result<()> {
		log::info!("[service update] {} / {} ({}/{})", n.title, n.text, n.progress, n.max);
		Ok(())
	}

	pub fn service_stop(&self, done: Option<DoneNotification>) -> crate::Result<()> {
		match done {
			Some(d) => log::info!("[service stop] {} / {}", d.title, d.text),
			None => log::info!("[service stop]"),
		}
		Ok(())
	}

	pub fn request_notifications(&self) -> crate::Result<bool> {
		Ok(true)
	}

	pub fn take_launch_action(&self) -> crate::Result<Option<String>> {
		Ok(None)
	}

	pub fn insets(&self) -> crate::Result<Insets> {
		Ok(Insets::default())
	}
}

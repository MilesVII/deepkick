use std::fs::File;
use std::os::fd::FromRawFd;

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tauri::{
	plugin::{PluginApi, PluginHandle},
	AppHandle, Runtime,
};

use crate::models::*;

const PLUGIN_IDENTIFIER: &str = "com.milesseventh.deepkick.bridge";

pub fn init<R: Runtime, C: DeserializeOwned>(
	_app: &AppHandle<R>,
	api: PluginApi<R, C>,
) -> crate::Result<Deepkick<R>> {
	let handle = api.register_android_plugin(PLUGIN_IDENTIFIER, "DeepkickPlugin")?;
	Ok(Deepkick(handle))
}

/// Access to the Kotlin side. Every call is blocking (it round-trips through the Android
/// main thread), so never call these from the Tauri event loop thread; use worker threads
/// or `spawn_blocking` inside async commands.
pub struct Deepkick<R: Runtime>(PluginHandle<R>);

#[derive(Deserialize)]
struct FilesResponse {
	files: Vec<PickedFile>,
}

#[derive(Serialize)]
struct OpenFdArgs<'a> {
	uri: &'a str,
	mode: &'a str,
}

#[derive(Deserialize)]
struct FdResponse {
	fd: i32,
}

#[derive(Serialize)]
struct StopArgs {
	done: Option<DoneNotification>,
}

#[derive(Deserialize)]
struct GrantedResponse {
	granted: bool,
}

#[derive(Deserialize)]
struct ActionResponse {
	#[serde(default)]
	action: Option<String>,
}

impl<R: Runtime> Deepkick<R> {
	/// Opens the system document picker (multiple selection). Cancelling yields an empty list.
	pub fn pick_files(&self) -> crate::Result<Vec<PickedFile>> {
		let r: FilesResponse = self.0.run_mobile_plugin("pickFiles", ())?;
		Ok(r.files)
	}

	/// Opens the system tree picker and returns every supported audio file found inside it.
	pub fn pick_directory(&self) -> crate::Result<Vec<PickedFile>> {
		let r: FilesResponse = self.0.run_mobile_plugin("pickDirectory", ())?;
		Ok(r.files)
	}

	/// Opens the document behind `uri` as a regular `File` (ownership of the descriptor is
	/// transferred to Rust, so dropping the `File` closes it).
	pub fn open_file(&self, uri: &str, write: bool) -> crate::Result<File> {
		let r: FdResponse = self.0.run_mobile_plugin(
			"openFd",
			OpenFdArgs { uri, mode: if write { "rw" } else { "r" } },
		)?;
		if r.fd < 0 {
			return Err(crate::Error::Other("invalid file descriptor".into()));
		}
		// SAFETY: the descriptor was detached from its ParcelFileDescriptor on the Kotlin side,
		// nobody else owns it.
		Ok(unsafe { File::from_raw_fd(r.fd) })
	}

	pub fn service_start(&self, n: &ProgressNotification) -> crate::Result<()> {
		self.0.run_mobile_plugin::<()>("serviceStart", n)?;
		Ok(())
	}

	pub fn service_update(&self, n: &ProgressNotification) -> crate::Result<()> {
		self.0.run_mobile_plugin::<()>("serviceUpdate", n)?;
		Ok(())
	}

	pub fn service_stop(&self, done: Option<DoneNotification>) -> crate::Result<()> {
		self.0.run_mobile_plugin::<()>("serviceStop", StopArgs { done })?;
		Ok(())
	}

	/// Requests POST_NOTIFICATIONS on Android 13+; resolves once the user answered.
	pub fn request_notifications(&self) -> crate::Result<bool> {
		let r: GrantedResponse = self.0.run_mobile_plugin("requestNotifications", ())?;
		Ok(r.granted)
	}

	/// Returns (and clears) the action carried by the intent that opened the activity,
	/// e.g. `"report"` when the user tapped the completion notification.
	pub fn take_launch_action(&self) -> crate::Result<Option<String>> {
		let r: ActionResponse = self.0.run_mobile_plugin("takeLaunchAction", ())?;
		Ok(r.action)
	}

	pub fn insets(&self) -> crate::Result<Insets> {
		Ok(self.0.run_mobile_plugin("getInsets", ())?)
	}
}

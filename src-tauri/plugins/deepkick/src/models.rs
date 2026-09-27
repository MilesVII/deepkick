use serde::{Deserialize, Serialize};

/// A file the user picked (directly or through a directory), addressed by an opaque URI.
/// On Android it is a `content://` document URI, on desktop it is a plain path.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PickedFile {
	pub uri: String,
	pub name: String,
	/// Human-readable location hint (relative folder inside the picked tree, or the storage path)
	#[serde(default)]
	pub folder: String,
	#[serde(default)]
	pub size: u64,
	#[serde(default)]
	pub mime: Option<String>,
}

/// Contents of the ongoing-progress notification shown by the foreground service.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressNotification {
	pub title: String,
	pub text: String,
	pub max: u32,
	pub progress: u32,
}

/// A one-shot notification posted when a run is over; tapping it opens the report.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoneNotification {
	pub title: String,
	pub text: String,
}

/// System bar / keyboard insets in CSS pixels.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Insets {
	pub top: f64,
	pub bottom: f64,
	pub left: f64,
	pub right: f64,
}

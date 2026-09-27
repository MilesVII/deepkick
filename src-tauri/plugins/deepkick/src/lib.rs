//! DeepKick Lyrics platform bridge.
//!
//! On Android this wraps a Kotlin plugin that handles the Storage Access Framework
//! (picking files and trees, opening file descriptors), the foreground service that keeps
//! the process alive while a queue is processed, and notifications.
//! On desktop a small stand-in with native dialogs and plain paths is used, so the app
//! can be developed and tested without an emulator.

use tauri::{
	plugin::{Builder, TauriPlugin},
	Manager, Runtime,
};

mod error;
mod models;

#[cfg(target_os = "android")]
mod mobile;
#[cfg(not(target_os = "android"))]
mod desktop;

pub use error::{Error, Result};
pub use models::*;

#[cfg(target_os = "android")]
pub use mobile::Deepkick;
#[cfg(not(target_os = "android"))]
pub use desktop::Deepkick;

/// Extension trait to access the bridge from any `Manager` (app handle, window, ...).
pub trait DeepkickExt<R: Runtime> {
	fn deepkick(&self) -> &Deepkick<R>;
}

impl<R: Runtime, T: Manager<R>> DeepkickExt<R> for T {
	fn deepkick(&self) -> &Deepkick<R> {
		self.state::<Deepkick<R>>().inner()
	}
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
	Builder::new("deepkick")
		.setup(|app, api| {
			#[cfg(target_os = "android")]
			let bridge = mobile::init(app, api)?;
			#[cfg(not(target_os = "android"))]
			let bridge = desktop::init(app, api)?;
			app.manage(bridge);
			Ok(())
		})
		.build()
}

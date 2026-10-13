//! DeepKick Lyrics core, shared by the Android app and the `deepkick` console utility:
//! reading and writing lyrics tags, the lyrics providers, and processing a single file.

pub mod encoding;
pub mod lyrics;
pub mod tags;

mod process;

pub use process::*;

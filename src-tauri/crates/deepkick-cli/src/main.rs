//! `deepkick`: the DeepKick Lyrics console utility.
//!
//! Without a subcommand it opens the interactive interface (optionally with files and
//! folders already queued, so dropping a folder onto `deepkick.exe` works too).

mod batch;
mod config;
mod editor;
mod queue;
mod scan;
mod tui;

use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use deepkick_core::Mode;

use crate::config::Config;

#[derive(Parser)]
#[command(
	name = "deepkick",
	version,
	about = "DeepKick Lyrics: fetch, edit and remove the lyrics of MP3 and FLAC files",
	long_about = "DeepKick Lyrics: fetch, edit and remove the lyrics of MP3 and FLAC files.\n\n\
		Run without a command to open the interactive interface. Files and folders given \
		as arguments are put in its queue.",
	args_conflicts_with_subcommands = true
)]
struct Cli {
	#[command(subcommand)]
	command: Option<Command>,

	/// Files or folders to put in the queue of the interactive interface
	paths: Vec<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
	/// Fetch lyrics and embed them, without the interactive interface
	Fetch {
		/// Files or folders (searched recursively)
		#[arg(required = true)]
		paths: Vec<PathBuf>,
		/// Replace lyrics that are already embedded
		#[arg(long)]
		overwrite: bool,
		/// Prefer synchronised (LRC) lyrics
		#[arg(long)]
		synced: bool,
		/// Do not try lyrics.ovh when LRCLIB has nothing
		#[arg(long)]
		no_fallback: bool,
	},
	/// Remove embedded lyrics
	Remove {
		/// Files or folders (searched recursively)
		#[arg(required = true)]
		paths: Vec<PathBuf>,
		/// Do not ask for confirmation
		#[arg(short, long)]
		yes: bool,
	},
	/// Print the tags and the lyrics of a file
	Show { file: PathBuf },
	/// Edit the lyrics of a file in your text editor
	Edit { file: PathBuf },
	/// Show or change settings: `deepkick config editor "code --wait"`
	Config {
		/// overwrite, prefer-synced, fallback or editor
		key: Option<String>,
		value: Option<String>,
	},
}

fn main() -> ExitCode {
	let cli = Cli::parse();
	let result = match cli.command {
		None => interactive(cli.paths),
		Some(command) => run(command),
	};
	match result {
		Ok(code) => code,
		Err(e) => {
			eprintln!("error: {e:#}");
			ExitCode::FAILURE
		},
	}
}

fn interactive(paths: Vec<PathBuf>) -> anyhow::Result<ExitCode> {
	if !std::io::stdout().is_terminal() {
		anyhow::bail!(
			"the interactive interface needs a terminal; use `deepkick fetch`, `remove`, `show` or `edit` instead"
		);
	}
	let cfg = Config::load()?;
	if let Some(summary) = tui::run(cfg, paths)? {
		println!("DeepKick Lyrics: {summary}");
	}
	Ok(ExitCode::SUCCESS)
}

fn run(command: Command) -> anyhow::Result<ExitCode> {
	match command {
		Command::Fetch { paths, overwrite, synced, no_fallback } => {
			let cfg = Config::load()?;
			let mut options = cfg.run_options();
			options.overwrite |= overwrite;
			options.prefer_synced |= synced;
			options.use_fallback &= !no_fallback;
			batch::run(&paths, Mode::Fetch, options, true)
		},
		Command::Remove { paths, yes } => {
			let cfg = Config::load()?;
			batch::run(&paths, Mode::Remove, cfg.run_options(), yes)
		},
		Command::Show { file } => batch::show(&file).map(|_| ExitCode::SUCCESS),
		Command::Edit { file } => {
			let cfg = Config::load()?;
			batch::edit(&cfg, &file).map(|_| ExitCode::SUCCESS)
		},
		Command::Config { key, value } => {
			batch::config_command(key.as_deref(), value.as_deref()).map(|_| ExitCode::SUCCESS)
		},
	}
}

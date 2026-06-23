//! Command-line front-end: parse arguments and dispatch to the app layer.
//!
//! The CLI is its own layer above [`app`](crate::app): it owns argument parsing
//! (`args`) and per-command handlers (`commands`), and its [`CliError`] wraps
//! [`AppError`] plus failures specific to the command line (such as an
//! unreadable input file).

mod args;
mod commands;
mod input;

use std::path::PathBuf;

use clap::Parser;

use crate::app::{App, AppError};
use args::{Cli, Command};

/// Parse arguments, build the app, and execute the requested command.
pub fn run() -> Result<(), CliError> {
    // clap handles --help/--version and reports malformed arguments itself.
    let cli = Cli::parse();
    init_logging(cli.verbose);

    let mut app = App::bootstrap()?;

    match cli.command {
        Command::Add(args) => commands::add::run(&mut app, args),
        Command::Draw(args) => commands::draw::run(&mut app, args),
        Command::List => commands::list::run(&app),
        Command::Show(args) => commands::show::run(&app, args),
        Command::Delete(args) => commands::delete::run(&mut app, args),
    }
}

/// Initialize logging. `RUST_LOG`, if set, wins; otherwise the `-v` count picks
/// the level (none → warnings, `-v` → info, `-vv`+ → debug).
fn init_logging(verbose: u8) {
    let mut builder = env_logger::Builder::new();
    if std::env::var_os("RUST_LOG").is_some() {
        builder.parse_env("RUST_LOG");
    } else {
        let level = match verbose {
            0 => log::LevelFilter::Warn,
            1 => log::LevelFilter::Info,
            _ => log::LevelFilter::Debug,
        };
        builder.filter_level(level);
    }
    builder.init();
}

/// Errors surfaced by the command-line layer.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error(transparent)]
    App(#[from] AppError),

    /// Terminal I/O failure while prompting for interactive input.
    #[error("interactive input failed")]
    Io(#[from] std::io::Error),

    #[error("could not read content file {path}")]
    ContentFileRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("a content source is required: pass --content or --content-file, or use --interactive")]
    MissingContent,

    #[error("could not launch editor '{editor}'")]
    EditorLaunch {
        editor: String,
        #[source]
        source: std::io::Error,
    },

    #[error("editor '{editor}' exited with an error; theorem not added")]
    EditorFailed { editor: String },

    #[error("aborted")]
    Aborted,

    #[error("provide a theorem id to delete, or use --interactive to pick from a list")]
    MissingId,

    #[error("no theorem found matching '{query}'")]
    TheoremNotFound { query: String },

    #[error("'{query}' matches {count} theorems; use a longer id prefix")]
    AmbiguousId { query: String, count: usize },
}

impl CliError {
    /// Process exit code, delegating to [`AppError`] for wrapped app errors and
    /// treating CLI input problems as user errors.
    pub fn exit_code(&self) -> u8 {
        match self {
            CliError::App(err) => err.exit_code(),
            CliError::Io(_)
            | CliError::ContentFileRead { .. }
            | CliError::MissingContent
            | CliError::EditorLaunch { .. }
            | CliError::EditorFailed { .. }
            | CliError::Aborted
            | CliError::MissingId
            | CliError::TheoremNotFound { .. }
            | CliError::AmbiguousId { .. } => 1,
        }
    }
}

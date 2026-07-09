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

use crate::app::{App, AppError, VaultEnv};
use args::{Cli, Command};

/// Env var naming the vault to operate on for this invocation, overridden by
/// the `--vault` flag. Empty is treated as unset.
const VAULT_ENV: &str = "THEOREM_PROVING_PRACTICE_VAULT";

/// Parse arguments, build the app, and execute the requested command.
pub fn run() -> Result<(), CliError> {
    // clap handles --help/--version and reports malformed arguments itself.
    let cli = Cli::parse();
    init_logging(cli.verbose);

    if cli.vault.is_some() && matches!(cli.command, Command::Vault(_)) {
        return Err(CliError::VaultFlagWithVaultCommand);
    }

    // Phase 1: the vault store itself, which must work even if the current
    // vault is broken — `vault add`/`vault switch` are the recovery path.
    let vaults = VaultEnv::bootstrap()?;
    let vault_override = resolve_vault_override(cli.vault);

    match cli.command {
        Command::Vault(args) => commands::vault::run(&vaults, args.command),
        other => {
            // Phase 2: bind the app to the resolved vault for every other
            // command.
            let mut app = App::bootstrap_in(&vaults, vault_override.as_deref())?;
            match other {
                Command::Add(args) => commands::add::run(&mut app, args),
                Command::Draw(args) => commands::draw::run(&mut app, args),
                Command::List => commands::list::run(&app),
                Command::Show(args) => commands::show::run(&app, args),
                Command::Edit(args) => commands::edit::run(&mut app, args),
                Command::Delete(args) => commands::delete::run(&mut app, args),
                Command::Open(args) => commands::open::run(&app, args),
                Command::Vault(_) => unreachable!("handled above"),
            }
        }
    }
}

/// Resolve the `--vault` override: the flag wins; otherwise
/// `THEOREM_PROVING_PRACTICE_VAULT`, if set and non-empty.
fn resolve_vault_override(flag: Option<String>) -> Option<String> {
    flag.or_else(|| std::env::var(VAULT_ENV).ok().filter(|v| !v.trim().is_empty()))
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

    #[error(
        "nothing to edit: pass --subject, --name, --content, or --content-file, or use --interactive"
    )]
    NoEditsRequested,

    #[error("no theorem found matching '{query}'")]
    TheoremNotFound { query: String },

    #[error("'{query}' matches {count} theorems; use a longer id prefix")]
    AmbiguousId { query: String, count: usize },

    #[error("no generated outputs yet; run `draw` first")]
    NoOutputs,

    #[error("no output file matching '{name}'")]
    OutputNotFound { name: String },

    #[error("'{name}' matches {count} output files; be more specific")]
    AmbiguousOutput { name: String, count: usize },

    #[error("could not launch opener '{opener}'")]
    OpenerLaunch {
        opener: String,
        #[source]
        source: std::io::Error,
    },

    #[error("opener '{opener}' exited with an error")]
    OpenerFailed { opener: String },

    #[error("no vault matches '{query}'")]
    VaultNotFound { query: String },

    #[error("'{query}' matches {count} vaults; use a longer prefix")]
    AmbiguousVault { query: String, count: usize },

    #[error("--vault cannot be combined with the 'vault' subcommand")]
    VaultFlagWithVaultCommand,
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
            | CliError::NoEditsRequested
            | CliError::TheoremNotFound { .. }
            | CliError::AmbiguousId { .. }
            | CliError::NoOutputs
            | CliError::OutputNotFound { .. }
            | CliError::AmbiguousOutput { .. }
            | CliError::OpenerLaunch { .. }
            | CliError::OpenerFailed { .. }
            | CliError::VaultNotFound { .. }
            | CliError::AmbiguousVault { .. }
            | CliError::VaultFlagWithVaultCommand => 1,
        }
    }
}

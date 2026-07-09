//! Application layer: orchestration of the storage, selection, and render
//! layers into use-cases (add, draw, list).
//!
//! [`App`] holds the trait objects for those layers and exposes one method per
//! use-case, each implemented in its own submodule. The use-cases know *what*
//! to do; the layers know *how*. [`AppError`] is the single error type they
//! surface, with `From` conversions from every layer error so `?` composes
//! cleanly.

mod add;
mod delete;
mod draw;
mod edit;
mod list;
mod open;
mod vaults;

pub use add::AddRequest;
pub use draw::{DrawOptions, DrawOutcome};
pub use edit::EditRequest;
pub use vaults::VaultEnv;

use std::path::PathBuf;

use crate::config::{Config, ConfigError};
use crate::domain::{DomainError, VaultName};
use crate::render::{HtmlRenderer, LatexRenderer, RenderError, Renderer};
use crate::selection::{ForgettingCurveSelector, SelectionError, Selector};
use crate::storage::{JsonStore, Repository, StorageError};
use crate::vaults::VaultError;

/// The application: configuration plus the storage, selection, and render
/// backends, bound to one vault. Use-case methods live in the submodules.
pub struct App {
    config: Config,
    /// Which vault this instance is bound to — set by [`App::bootstrap_in`]
    /// (or left at [`VaultName::default_vault`] by [`App::new`], which
    /// doesn't itself know about vaults; existing tests construct it that
    /// way and don't care which name is reported).
    vault: VaultName,
    repo: Box<dyn Repository>,
    selector: Box<dyn Selector>,
    /// Renderer for the default PDF format.
    renderer: Box<dyn Renderer>,
    /// Renderer for the HTML format (`draw --format html`).
    html_renderer: Box<dyn Renderer>,
}

impl App {
    /// Construct an app with the default selection and rendering strategies,
    /// backed by the given config and repository. Tests can swap in fakes with
    /// [`App::with_selector`] / [`App::with_renderer`] / [`App::with_html_renderer`].
    pub fn new(config: Config, repo: Box<dyn Repository>) -> Self {
        let renderer = LatexRenderer::new(config.latex_engine.clone());
        Self {
            vault: VaultName::default_vault(),
            repo,
            selector: Box::new(ForgettingCurveSelector::new()),
            renderer: Box::new(renderer),
            html_renderer: Box::new(HtmlRenderer::new()),
            config,
        }
    }

    /// Replace the selection strategy (used to inject fakes in tests).
    pub fn with_selector(mut self, selector: Box<dyn Selector>) -> Self {
        self.selector = selector;
        self
    }

    /// Replace the PDF rendering strategy (used to inject fakes in tests).
    pub fn with_renderer(mut self, renderer: Box<dyn Renderer>) -> Self {
        self.renderer = renderer;
        self
    }

    /// Replace the HTML rendering strategy (used to inject fakes in tests).
    pub fn with_html_renderer(mut self, renderer: Box<dyn Renderer>) -> Self {
        self.html_renderer = renderer;
        self
    }

    /// Build the app for the vault resolved from `vault_override` (the
    /// `--vault` flag or its env var, if given) or, when `None`, `vaults`'
    /// persisted current vault — see [`VaultEnv::resolve_vault`] for exactly
    /// how that resolution and its error cases work.
    ///
    /// Binds this instance's [`Repository`] to the resolved vault's store and
    /// its output directory to that vault's output subdirectory, so every
    /// other use-case (`draw`, `list`, `open`, …) needs no vault-awareness of
    /// its own.
    pub fn bootstrap_in(vaults: &VaultEnv, vault_override: Option<&str>) -> Result<Self, AppError> {
        let name = vaults.resolve_vault(vault_override)?;

        let mut config = vaults.config().clone();
        let store_path = config.vault_store_path(&name);
        config.output_dir = config.vault_output_dir(&name);

        let repo = JsonStore::new(store_path);
        let mut app = Self::new(config, Box::new(repo));
        app.vault = name;
        Ok(app)
    }

    /// The resolved configuration for this run.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// The vault this instance is bound to.
    pub fn vault_name(&self) -> &VaultName {
        &self.vault
    }
}

/// The unified error type returned by application use-cases.
///
/// Most variants wrap a layer error transparently, preserving its `Display` and
/// `source()` chain; [`AppError::OutputExists`] is an app-level use-case
/// condition that belongs to no single layer. The binary maps
/// [`AppError::exit_code`] to a process exit status.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Config(#[from] ConfigError),

    #[error(transparent)]
    Domain(#[from] DomainError),

    #[error(transparent)]
    Storage(#[from] StorageError),

    #[error(transparent)]
    Selection(#[from] SelectionError),

    #[error(transparent)]
    Render(#[from] RenderError),

    #[error(transparent)]
    Vault(#[from] VaultError),

    #[error(
        "current vault '{name}' no longer exists; run 'vault list' and 'vault switch' to \
         pick another, or 'vault add {name}' to recreate it"
    )]
    CurrentVaultMissing { name: VaultName },

    #[error("output file {path} already exists; omit --no-clobber to replace it")]
    OutputExists { path: PathBuf },

    #[error("could not read output directory {path}")]
    OutputListing {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

impl AppError {
    /// Process exit code for this error, so the tool is scriptable.
    ///
    /// `1` user/input problems, `2` data/storage problems, `3` rendering
    /// problems. Kept small and stable on purpose.
    pub fn exit_code(&self) -> u8 {
        match self {
            AppError::Domain(_) | AppError::Selection(_) | AppError::OutputExists { .. } => 1,
            AppError::Config(_)
            | AppError::Storage(_)
            | AppError::OutputListing { .. }
            | AppError::CurrentVaultMissing { .. } => 2,
            AppError::Render(_) => 3,
            AppError::Vault(err) => err.exit_code(),
        }
    }
}

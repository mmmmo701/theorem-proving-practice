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
mod list;

pub use add::AddRequest;
pub use draw::{DrawOptions, DrawOutcome};

use std::path::PathBuf;

use crate::config::{Config, ConfigError};
use crate::domain::DomainError;
use crate::render::{HtmlRenderer, LatexRenderer, RenderError, Renderer};
use crate::selection::{ForgettingCurveSelector, SelectionError, Selector};
use crate::storage::{JsonStore, Repository, StorageError};

/// The application: configuration plus the storage, selection, and render
/// backends. Use-case methods live in the submodules.
pub struct App {
    config: Config,
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

    /// Build the app with the default wiring: load configuration, ensure its
    /// directories exist, and back it with a [`JsonStore`].
    pub fn bootstrap() -> Result<Self, AppError> {
        let config = Config::load()?;
        log::debug!("configuration: {config:?}");
        config.ensure_dirs()?;
        let repo = JsonStore::new(config.store_path());
        Ok(Self::new(config, Box::new(repo)))
    }

    /// The resolved configuration for this run.
    pub fn config(&self) -> &Config {
        &self.config
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

    #[error("output file {path} already exists; omit --no-clobber to replace it")]
    OutputExists { path: PathBuf },
}

impl AppError {
    /// Process exit code for this error, so the tool is scriptable.
    ///
    /// `1` user/input problems, `2` data/storage problems, `3` rendering
    /// problems. Kept small and stable on purpose.
    pub fn exit_code(&self) -> u8 {
        match self {
            AppError::Domain(_) | AppError::Selection(_) | AppError::OutputExists { .. } => 1,
            AppError::Config(_) | AppError::Storage(_) => 2,
            AppError::Render(_) => 3,
        }
    }
}

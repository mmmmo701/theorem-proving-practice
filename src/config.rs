//! Runtime configuration: where data and output live, and rendering defaults.
//!
//! For the prototype, [`Config`] is built from defaults (paths relative to the
//! current working directory). The loader is fallible by design so that reading
//! from a config file or environment can be added later without changing any
//! call sites.

use std::path::{Path, PathBuf};

/// File name of the JSON theorem store, kept inside [`Config::data_dir`].
pub const STORE_FILE_NAME: &str = "theorems.json";

/// Resolved configuration for a single run of the tool.
#[derive(Debug, Clone)]
pub struct Config {
    /// Directory holding the persisted theorem store.
    pub data_dir: PathBuf,
    /// Directory where generated practice PDFs are written.
    pub output_dir: PathBuf,
    /// How many theorems a daily draw selects when not overridden.
    pub default_draw_count: usize,
    /// LaTeX engine command used by the renderer (must be on `PATH`).
    pub latex_engine: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("data"),
            output_dir: PathBuf::from("output"),
            default_draw_count: 3,
            // latexmk drives the underlying engine and reruns as needed; it is
            // installed on the target host. Overridable here for xelatex etc.
            latex_engine: "latexmk".to_string(),
        }
    }
}

impl Config {
    /// Resolve the effective configuration for this run.
    ///
    /// Currently this only materializes defaults, but it returns a `Result` so
    /// a future file/env-backed loader can surface [`ConfigError`] without a
    /// signature change.
    pub fn load() -> Result<Self, ConfigError> {
        Ok(Self::default())
    }

    /// Full path to the JSON theorem store.
    pub fn store_path(&self) -> PathBuf {
        self.data_dir.join(STORE_FILE_NAME)
    }

    /// Ensure the data and output directories exist, creating them if needed.
    ///
    /// Idempotent. Call before any operation that reads or writes those dirs.
    pub fn ensure_dirs(&self) -> Result<(), ConfigError> {
        ensure_dir(&self.data_dir)?;
        ensure_dir(&self.output_dir)?;
        Ok(())
    }
}

fn ensure_dir(path: &Path) -> Result<(), ConfigError> {
    std::fs::create_dir_all(path).map_err(|source| ConfigError::CreateDir {
        path: path.to_path_buf(),
        source,
    })
}

/// Failures while resolving configuration or preparing its directories.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("could not create directory {path}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

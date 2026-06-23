//! Runtime configuration: where data and output live, and rendering defaults.
//!
//! [`Config::default`] uses paths relative to the current working directory and
//! exists mainly for tests. The real entry point is [`Config::load`], which
//! resolves *fixed*, per-user directories (XDG Base Directory layout) so an
//! installed binary always reads and writes the same place regardless of the
//! working directory it is invoked from.

use std::path::{Path, PathBuf};

/// File name of the JSON theorem store, kept inside [`Config::data_dir`].
pub const STORE_FILE_NAME: &str = "theorems.json";

/// Sub-directory name under the per-user data root, also used as the env-var
/// prefix for overrides.
const APP_DIR_NAME: &str = "theorem-proving-practice";

/// Env var that overrides the resolved data directory outright.
const DATA_DIR_ENV: &str = "THEOREM_PROVING_PRACTICE_DATA_DIR";
/// Env var that overrides the resolved output directory outright.
const OUTPUT_DIR_ENV: &str = "THEOREM_PROVING_PRACTICE_OUTPUT_DIR";

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
    /// Unlike [`Config::default`], paths here are absolute and *independent of
    /// the current working directory*, so an installed binary always uses the
    /// same store and output location. Resolution order for each directory:
    ///
    /// 1. an explicit env override (`THEOREM_PROVING_PRACTICE_DATA_DIR` /
    ///    `THEOREM_PROVING_PRACTICE_OUTPUT_DIR`),
    /// 2. the per-user XDG location
    ///    (`$XDG_DATA_HOME/theorem-proving-practice/...`),
    /// 3. `$HOME/.local/share/theorem-proving-practice/...`.
    ///
    /// Only if none of those yield an absolute path do we fall back to the
    /// cwd-relative defaults (e.g. when neither `HOME` nor `XDG_DATA_HOME` is
    /// set), preserving the old behaviour as a last resort.
    pub fn load() -> Result<Self, ConfigError> {
        let defaults = Self::default();
        let data_root = data_root();
        Ok(Self {
            data_dir: dir_from_env_or(DATA_DIR_ENV, data_root.as_deref(), "data", defaults.data_dir),
            output_dir: dir_from_env_or(
                OUTPUT_DIR_ENV,
                data_root.as_deref(),
                "output",
                defaults.output_dir,
            ),
            ..defaults
        })
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

/// The per-user data root (`.../theorem-proving-practice`), or `None` if the
/// environment gives us no absolute base to anchor it to.
fn data_root() -> Option<PathBuf> {
    let base = non_empty_env("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| non_empty_env("HOME").map(|home| PathBuf::from(home).join(".local/share")))?;
    Some(base.join(APP_DIR_NAME))
}

/// Resolve a single directory: an explicit env override wins; otherwise place
/// `leaf` under `data_root`; otherwise fall back to the cwd-relative default.
fn dir_from_env_or(
    env_key: &str,
    data_root: Option<&Path>,
    leaf: &str,
    fallback: PathBuf,
) -> PathBuf {
    if let Some(p) = non_empty_env(env_key) {
        return PathBuf::from(p);
    }
    match data_root {
        Some(root) => root.join(leaf),
        None => fallback,
    }
}

/// Read an env var, treating unset *and* empty as absent.
fn non_empty_env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.is_empty())
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

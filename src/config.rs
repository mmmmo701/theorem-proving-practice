//! Runtime configuration: where data and output live, and rendering defaults.
//!
//! [`Config::default`] uses paths relative to the current working directory and
//! exists mainly for tests. The real entry point is [`Config::load`], which
//! resolves *fixed*, per-user directories (XDG Base Directory layout) so an
//! installed binary always reads and writes the same place regardless of the
//! working directory it is invoked from.
//!
//! Since the vault feature, storage is split into per-vault directories under
//! [`Config::vaults_dir`] (see the `vaults` module); [`Config::data_dir`] now
//! names only the *legacy* flat-store location, kept solely so startup can
//! detect and migrate a pre-vault library into the `default` vault.

use std::path::{Path, PathBuf};

use crate::domain::VaultName;

/// File name of the JSON theorem store, kept inside each vault's directory
/// (and, pre-migration, directly inside [`Config::data_dir`]).
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
    /// Directory holding the pre-vault, flat theorem store (`theorems.json`
    /// directly inside it). Used only to detect and migrate an existing
    /// library into the `default` vault; new data never targets this path.
    pub data_dir: PathBuf,
    /// Root directory under which each vault's output subdirectory
    /// (`<output_dir>/<vault>/practice-*.pdf`) is written.
    pub output_dir: PathBuf,
    /// Root directory under which `vaults/<name>/theorems.json` and
    /// `state.json` (the persisted current vault) live.
    pub vaults_root: PathBuf,
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
            vaults_root: PathBuf::from("."),
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
    ///
    /// `THEOREM_PROVING_PRACTICE_DATA_DIR`, if set, now names [`vaults_root`]
    /// directly (the root under which `vaults/` and `state.json` live) rather
    /// than a flat store directory — but the same value is also kept as
    /// [`data_dir`] so the *legacy* `theorems.json` an override previously
    /// pointed at is still found for migration.
    ///
    /// [`vaults_root`]: Config::vaults_root
    /// [`data_dir`]: Config::data_dir
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
            vaults_root: vaults_root_from_env_or(data_root.as_deref(), defaults.vaults_root),
            ..defaults
        })
    }

    /// Full path to the pre-vault, flat theorem store — the migration source,
    /// never a live vault's store. (Also used directly by tests that build a
    /// bare `App` over a flat `Config::data_dir`, bypassing vaults entirely.)
    pub fn store_path(&self) -> PathBuf {
        self.data_dir.join(STORE_FILE_NAME)
    }

    /// Directory holding one subdirectory per vault.
    pub fn vaults_dir(&self) -> PathBuf {
        self.vaults_root.join("vaults")
    }

    /// Path to the small file recording which vault is current.
    pub fn state_path(&self) -> PathBuf {
        self.vaults_root.join("state.json")
    }

    /// Full path to the given vault's theorem store.
    pub fn vault_store_path(&self, name: &VaultName) -> PathBuf {
        self.vaults_dir().join(name.as_str()).join(STORE_FILE_NAME)
    }

    /// Directory the given vault's practice sheets are written into.
    pub fn vault_output_dir(&self, name: &VaultName) -> PathBuf {
        self.output_dir.join(name.as_str())
    }

    /// Ensure the vaults and output root directories exist, creating them if
    /// needed. A vault's own directory and its output subdirectory are
    /// created lazily (on `vault add` / first `draw`), so this only has to
    /// cover the roots they live under.
    ///
    /// Idempotent. Call before any operation that reads or writes those dirs.
    pub fn ensure_dirs(&self) -> Result<(), ConfigError> {
        ensure_dir(&self.vaults_dir())?;
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

/// Resolve [`Config::vaults_root`]: an explicit `DATA_DIR_ENV` override wins
/// and is used *as-is* (it names the root directly, with no leaf joined,
/// unlike [`dir_from_env_or`]); otherwise the root is `data_root` itself;
/// otherwise the cwd-relative fallback.
fn vaults_root_from_env_or(data_root: Option<&Path>, fallback: PathBuf) -> PathBuf {
    if let Some(p) = non_empty_env(DATA_DIR_ENV) {
        return PathBuf::from(p);
    }
    match data_root {
        Some(root) => root.to_path_buf(),
        None => fallback,
    }
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

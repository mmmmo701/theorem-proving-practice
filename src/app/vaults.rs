//! Vault-level use-cases: list, add, switch, and query the current vault —
//! plus [`VaultEnv::bootstrap`], the first phase of startup
//! ([`super::App::bootstrap_in`] is the second).
//!
//! Split into two phases so vault management keeps working even when the
//! *current* vault is broken or missing: [`VaultEnv::bootstrap`] only needs
//! the vault store itself to be reachable, never any specific vault's data,
//! so `vault add` / `vault switch` remain a working recovery path.

use crate::config::Config;
use crate::domain::VaultName;
use crate::vaults::{FsVaultStore, VaultError, VaultStore, migrate_legacy_layout};

use super::AppError;

/// Configuration plus the vault store: which vaults exist, and which is
/// current. Independent of any single vault's theorem data — that binding
/// happens in [`super::App::bootstrap_in`].
pub struct VaultEnv {
    config: Config,
    store: Box<dyn VaultStore>,
}

impl VaultEnv {
    /// Load configuration, ensure its root directories exist, migrate a
    /// pre-vault library into the `default` vault if one is found, and wire
    /// up the vault store.
    pub fn bootstrap() -> Result<Self, AppError> {
        let config = Config::load()?;
        log::debug!("configuration: {config:?}");
        config.ensure_dirs()?;
        migrate_legacy_layout(&config)?;
        let store = FsVaultStore::new(config.vaults_dir(), config.state_path());
        Ok(Self {
            config,
            store: Box::new(store),
        })
    }

    /// The resolved configuration for this run.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// All vaults, in a stable order.
    pub fn list_vaults(&self) -> Result<Vec<VaultName>, AppError> {
        Ok(self.store.list()?)
    }

    /// The persisted current vault (`default` if none has ever been set).
    pub fn current_vault(&self) -> Result<VaultName, AppError> {
        Ok(self.store.current()?)
    }

    /// Whether a vault with this name exists.
    pub fn vault_exists(&self, name: &VaultName) -> Result<bool, AppError> {
        Ok(self.store.exists(name)?)
    }

    /// Create a new, empty vault. Errors if one with this name already
    /// exists. Does not switch to it — a caller that wants that does so
    /// explicitly via [`VaultEnv::switch_vault`].
    pub fn add_vault(&self, name: &VaultName) -> Result<(), AppError> {
        Ok(self.store.create(name)?)
    }

    /// Persist `name` as the current vault. Errors if no such vault exists;
    /// never auto-creates one, so a typo can't silently switch into an empty
    /// vault.
    pub fn switch_vault(&self, name: &VaultName) -> Result<(), AppError> {
        if !self.store.exists(name)? {
            return Err(self.not_found(name)?.into());
        }
        Ok(self.store.set_current(name)?)
    }

    /// Build a [`VaultError::NotFound`] for `name`, filled in with the
    /// current list of available vaults.
    fn not_found(&self, name: &VaultName) -> Result<VaultError, AppError> {
        let available = self
            .list_vaults()?
            .iter()
            .map(|v| v.as_str().to_string())
            .collect();
        Ok(VaultError::NotFound {
            name: name.as_str().to_string(),
            available,
        })
    }

    /// Resolve which vault a theorem-level command should bind to:
    /// `vault_override` (from `--vault` or its env var) if given, otherwise
    /// the persisted current vault.
    ///
    /// The resolved vault must exist, with one exception: if resolution
    /// (from the persisted state, not an explicit override) yields `default`
    /// and it doesn't exist yet, it is created on the fly — that is the
    /// first-run path. An explicit `--vault` naming a vault that doesn't
    /// exist is always a user error and is never auto-created, even for
    /// `default`, so a typo can't silently start an empty vault. A missing
    /// *persisted* current vault (not from an override) is reported as
    /// [`AppError::CurrentVaultMissing`] rather than [`VaultError::NotFound`]
    /// — a broken environment, not a bad argument to this invocation.
    pub fn resolve_vault(&self, vault_override: Option<&str>) -> Result<VaultName, AppError> {
        let (name, from_override) = match vault_override {
            Some(raw) => (VaultName::new(raw)?, true),
            None => (self.current_vault()?, false),
        };

        if self.vault_exists(&name)? {
            return Ok(name);
        }
        if !from_override && name == VaultName::default_vault() {
            self.add_vault(&name)?;
            return Ok(name);
        }
        if from_override {
            return Err(self.not_found(&name)?.into());
        }
        Err(AppError::CurrentVaultMissing { name })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn env_in(dir: &std::path::Path) -> VaultEnv {
        let config = Config {
            data_dir: dir.join("data"),
            output_dir: dir.join("output"),
            vaults_root: dir.join("root"),
            ..Config::default()
        };
        config.ensure_dirs().unwrap();
        let store = FsVaultStore::new(config.vaults_dir(), config.state_path());
        VaultEnv {
            config,
            store: Box::new(store),
        }
    }

    #[test]
    fn fresh_env_resolves_to_default_and_creates_it() {
        let dir = tempdir().unwrap();
        let env = env_in(dir.path());

        let name = env.resolve_vault(None).unwrap();

        assert_eq!(name, VaultName::default_vault());
        assert!(env.vault_exists(&name).unwrap());
    }

    #[test]
    fn override_selects_an_existing_vault_without_changing_current() {
        let dir = tempdir().unwrap();
        let env = env_in(dir.path());
        let exams = VaultName::new("exams").unwrap();
        env.add_vault(&exams).unwrap();

        let resolved = env.resolve_vault(Some("exams")).unwrap();

        assert_eq!(resolved, exams);
        assert_eq!(env.current_vault().unwrap(), VaultName::default_vault());
    }

    #[test]
    fn override_naming_an_unknown_vault_is_an_error_even_for_default() {
        let dir = tempdir().unwrap();
        let env = env_in(dir.path());

        let err = env.resolve_vault(Some("default")).unwrap_err();
        assert!(matches!(
            err,
            AppError::Vault(VaultError::NotFound { .. })
        ));
    }

    #[test]
    fn missing_non_default_current_vault_is_a_current_vault_missing_error() {
        let dir = tempdir().unwrap();
        let env = env_in(dir.path());
        let exams = VaultName::new("exams").unwrap();
        // Persist "exams" as current without ever creating it.
        env.store.set_current(&exams).unwrap();

        let err = env.resolve_vault(None).unwrap_err();
        assert!(matches!(err, AppError::CurrentVaultMissing { name } if name == exams));
    }

    #[test]
    fn switch_to_unknown_vault_is_not_found() {
        let dir = tempdir().unwrap();
        let env = env_in(dir.path());

        let err = env.switch_vault(&VaultName::new("nope").unwrap()).unwrap_err();
        assert!(matches!(err, AppError::Vault(VaultError::NotFound { .. })));
    }

    #[test]
    fn switch_to_existing_vault_persists_it() {
        let dir = tempdir().unwrap();
        let env = env_in(dir.path());
        let exams = VaultName::new("exams").unwrap();
        env.add_vault(&exams).unwrap();

        env.switch_vault(&exams).unwrap();
        assert_eq!(env.current_vault().unwrap(), exams);
    }

    #[test]
    fn add_duplicate_vault_is_an_error() {
        let dir = tempdir().unwrap();
        let env = env_in(dir.path());
        let exams = VaultName::new("exams").unwrap();
        env.add_vault(&exams).unwrap();

        let err = env.add_vault(&exams).unwrap_err();
        assert!(matches!(err, AppError::Vault(VaultError::AlreadyExists { .. })));
    }
}

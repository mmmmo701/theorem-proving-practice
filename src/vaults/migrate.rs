//! One-time migration of a pre-vault, flat theorem store into the `default`
//! vault.
//!
//! Runs on every startup (see `app::VaultEnv::bootstrap`); cheap and a no-op
//! once migrated. The move is a same-filesystem rename, atomic on virtually
//! all platforms, and refuses to guess if both the old and new locations
//! already hold data — see [`VaultError::MigrationConflict`]. Outputs already
//! in the output directory are not migrated: they are regenerable artifacts,
//! not data, and new draws in the `default` vault simply start writing into
//! its own output subdirectory.

use crate::config::Config;
use crate::domain::VaultName;

use super::VaultError;

/// If a pre-vault flat store exists and the `default` vault does not yet have
/// one, move it into place. A no-op if already migrated, or if there was
/// never a pre-vault store to migrate (a fresh install).
///
/// Errors if *both* locations already hold a store: never silently pick a
/// side, since either could be the one the user actually wants; the error
/// tells them to remove or rename one before continuing.
pub fn migrate_legacy_layout(config: &Config) -> Result<(), VaultError> {
    let legacy = config.store_path();
    let default_store = config.vault_store_path(&VaultName::default_vault());

    if !legacy.exists() {
        return Ok(());
    }
    if default_store.exists() {
        return Err(VaultError::MigrationConflict {
            legacy,
            default_store,
        });
    }

    let dir = default_store
        .parent()
        .expect("a vault store path always has a parent directory");
    std::fs::create_dir_all(dir).map_err(|source| VaultError::MigrationIo {
        from: legacy.clone(),
        to: default_store.clone(),
        source,
    })?;
    std::fs::rename(&legacy, &default_store).map_err(|source| VaultError::MigrationIo {
        from: legacy.clone(),
        to: default_store.clone(),
        source,
    })?;
    log::info!(
        "migrated existing library from {} into vault 'default' at {}",
        legacy.display(),
        default_store.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn config_in(dir: &std::path::Path) -> Config {
        Config {
            data_dir: dir.join("data"),
            output_dir: dir.join("output"),
            vaults_root: dir.join("root"),
            ..Config::default()
        }
    }

    #[test]
    fn no_legacy_store_is_a_no_op() {
        let dir = tempdir().unwrap();
        let config = config_in(dir.path());
        migrate_legacy_layout(&config).unwrap();
        assert!(!config.vault_store_path(&VaultName::default_vault()).exists());
    }

    #[test]
    fn migrates_legacy_store_into_default_vault() {
        let dir = tempdir().unwrap();
        let config = config_in(dir.path());
        std::fs::create_dir_all(&config.data_dir).unwrap();
        std::fs::write(config.store_path(), br#"{"version":1,"theorems":[]}"#).unwrap();

        migrate_legacy_layout(&config).unwrap();

        assert!(!config.store_path().exists());
        assert!(config.vault_store_path(&VaultName::default_vault()).exists());
    }

    #[test]
    fn migration_is_idempotent() {
        let dir = tempdir().unwrap();
        let config = config_in(dir.path());
        std::fs::create_dir_all(&config.data_dir).unwrap();
        std::fs::write(config.store_path(), br#"{"version":1,"theorems":[]}"#).unwrap();

        migrate_legacy_layout(&config).unwrap();
        // Legacy is gone now, so a second run is a no-op, not an error.
        migrate_legacy_layout(&config).unwrap();
    }

    #[test]
    fn both_locations_existing_is_a_conflict_and_touches_neither() {
        let dir = tempdir().unwrap();
        let config = config_in(dir.path());
        std::fs::create_dir_all(&config.data_dir).unwrap();
        std::fs::write(config.store_path(), b"legacy").unwrap();
        let default_store = config.vault_store_path(&VaultName::default_vault());
        std::fs::create_dir_all(default_store.parent().unwrap()).unwrap();
        std::fs::write(&default_store, b"already-vaulted").unwrap();

        let err = migrate_legacy_layout(&config).unwrap_err();
        assert!(matches!(err, VaultError::MigrationConflict { .. }));
        assert_eq!(std::fs::read(config.store_path()).unwrap(), b"legacy");
        assert_eq!(std::fs::read(&default_store).unwrap(), b"already-vaulted");
    }

    #[test]
    fn preserves_theorem_content_across_migration() {
        let dir = tempdir().unwrap();
        let config = config_in(dir.path());
        std::fs::create_dir_all(&config.data_dir).unwrap();
        let content = br#"{"version":1,"theorems":[{"id":"00000000-0000-0000-0000-000000000000","subject":"S","name":"N","content":"C","added_at":"2026-06-18T00:00:00Z"}]}"#;
        std::fs::write(config.store_path(), content).unwrap();

        migrate_legacy_layout(&config).unwrap();

        let migrated =
            std::fs::read(config.vault_store_path(&VaultName::default_vault())).unwrap();
        assert_eq!(migrated, content);
    }
}

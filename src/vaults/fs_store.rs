//! [`FsVaultStore`]: vaults as directories under a root, with the directory
//! listing itself as the source of truth for which vaults exist.
//!
//! No separate registry file lists the vaults — a registry can drift from the
//! directories on disk; a directory scan cannot. Only the *current* vault
//! pointer is persisted, in a small `state.json` next to the `vaults/`
//! directory, written with the same atomic write-fsync-rename recipe as the
//! theorem store (`fs_atomic::write`).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{VaultError, VaultStore};
use crate::config::STORE_FILE_NAME;
use crate::domain::VaultName;
use crate::storage::JsonStore;

/// Schema version this build writes and is the newest it understands for
/// `state.json`. Independent of the theorem store's own schema version.
const STATE_VERSION: u32 = 1;

/// On-disk envelope for `state.json`.
#[derive(Debug, Serialize, Deserialize)]
struct StateFile {
    version: u32,
    current_vault: VaultName,
}

/// Minimal probe that reads only the schema version, so a version mismatch is
/// reported as such even if the rest of the document doesn't parse.
#[derive(Debug, Deserialize)]
struct StateVersionProbe {
    version: u32,
}

/// A [`VaultStore`] backed by one subdirectory per vault plus a `state.json`
/// recording the current vault.
#[derive(Debug, Clone)]
pub struct FsVaultStore {
    vaults_dir: PathBuf,
    state_path: PathBuf,
}

impl FsVaultStore {
    /// Create a store rooted at `vaults_dir` (one subdirectory per vault),
    /// persisting the current-vault pointer at `state_path`. Neither path
    /// need exist yet.
    pub fn new(vaults_dir: impl Into<PathBuf>, state_path: impl Into<PathBuf>) -> Self {
        Self {
            vaults_dir: vaults_dir.into(),
            state_path: state_path.into(),
        }
    }

    /// Directory holding the given vault's files.
    fn vault_dir(&self, name: &VaultName) -> PathBuf {
        self.vaults_dir.join(name.as_str())
    }

    /// Path to the given vault's theorem store.
    pub fn store_path(&self, name: &VaultName) -> PathBuf {
        self.vault_dir(name).join(STORE_FILE_NAME)
    }
}

impl VaultStore for FsVaultStore {
    fn list(&self) -> Result<Vec<VaultName>, VaultError> {
        let read_dir = match std::fs::read_dir(&self.vaults_dir) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => {
                return Err(VaultError::ListDir {
                    path: self.vaults_dir.clone(),
                    source,
                });
            }
        };

        let mut names = Vec::new();
        for entry in read_dir {
            let entry = entry.map_err(|source| VaultError::ListDir {
                path: self.vaults_dir.clone(),
                source,
            })?;
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if !is_dir {
                continue;
            }

            // A stray non-directory or badly-named entry must not brick the
            // tool: skip it with a warning rather than failing the listing.
            let Some(file_name) = entry.file_name().into_string().ok() else {
                log::warn!(
                    "skipping non-UTF-8 entry in vaults directory: {:?}",
                    entry.file_name()
                );
                continue;
            };
            match VaultName::new(&file_name) {
                Ok(name) => names.push(name),
                Err(err) => log::warn!("skipping invalid vault directory '{file_name}': {err}"),
            }
        }
        names.sort();
        Ok(names)
    }

    fn exists(&self, name: &VaultName) -> Result<bool, VaultError> {
        Ok(self.vault_dir(name).is_dir())
    }

    fn create(&self, name: &VaultName) -> Result<(), VaultError> {
        if self.exists(name)? {
            return Err(VaultError::AlreadyExists { name: name.clone() });
        }
        let dir = self.vault_dir(name);
        std::fs::create_dir_all(&dir).map_err(|source| VaultError::CreateDir {
            path: dir,
            source,
        })?;
        JsonStore::new(self.store_path(name))
            .ensure_exists()
            .map_err(|source| VaultError::InitialStore {
                name: name.clone(),
                source,
            })
    }

    fn current(&self) -> Result<VaultName, VaultError> {
        let bytes = match std::fs::read(&self.state_path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(VaultName::default_vault());
            }
            Err(source) => {
                return Err(VaultError::StateRead {
                    path: self.state_path.clone(),
                    source,
                });
            }
        };

        // Check the version before a full parse, so a future format is
        // reported as StateUnsupportedVersion rather than StateCorrupt.
        let probe: StateVersionProbe =
            serde_json::from_slice(&bytes).map_err(|source| VaultError::StateCorrupt {
                path: self.state_path.clone(),
                source,
            })?;
        if probe.version > STATE_VERSION {
            return Err(VaultError::StateUnsupportedVersion {
                found: probe.version,
                supported: STATE_VERSION,
            });
        }

        let state: StateFile =
            serde_json::from_slice(&bytes).map_err(|source| VaultError::StateCorrupt {
                path: self.state_path.clone(),
                source,
            })?;
        Ok(state.current_vault)
    }

    fn set_current(&self, name: &VaultName) -> Result<(), VaultError> {
        let json = serde_json::to_vec_pretty(&StateFile {
            version: STATE_VERSION,
            current_vault: name.clone(),
        })
        .expect("StateFile serialization cannot fail");
        crate::fs_atomic::write(&self.state_path, &json).map_err(|source| {
            VaultError::StateWrite {
                path: self.state_path.clone(),
                source,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn store_in(dir: &std::path::Path) -> FsVaultStore {
        FsVaultStore::new(dir.join("vaults"), dir.join("state.json"))
    }

    #[test]
    fn fresh_store_has_no_vaults_and_defaults_to_default_current() {
        let dir = tempdir().unwrap();
        let store = store_in(dir.path());
        assert!(store.list().unwrap().is_empty());
        assert_eq!(store.current().unwrap(), VaultName::default_vault());
    }

    #[test]
    fn create_makes_the_vault_visible_and_durable() {
        let dir = tempdir().unwrap();
        let store = store_in(dir.path());
        let name = VaultName::new("exams").unwrap();

        store.create(&name).unwrap();

        assert!(store.exists(&name).unwrap());
        assert_eq!(store.list().unwrap(), vec![name.clone()]);
        assert!(store.store_path(&name).exists());
    }

    #[test]
    fn create_duplicate_is_an_error() {
        let dir = tempdir().unwrap();
        let store = store_in(dir.path());
        let name = VaultName::new("exams").unwrap();
        store.create(&name).unwrap();

        let err = store.create(&name).unwrap_err();
        assert!(matches!(err, VaultError::AlreadyExists { .. }));
    }

    #[test]
    fn set_current_then_current_round_trips() {
        let dir = tempdir().unwrap();
        let store = store_in(dir.path());
        let name = VaultName::new("exams").unwrap();

        store.set_current(&name).unwrap();
        assert_eq!(store.current().unwrap(), name);
    }

    #[test]
    fn list_is_sorted_and_stable() {
        let dir = tempdir().unwrap();
        let store = store_in(dir.path());
        for n in ["zeta", "alpha", "mu"] {
            store.create(&VaultName::new(n).unwrap()).unwrap();
        }
        let names: Vec<String> = store.list().unwrap().iter().map(|v| v.as_str().to_string()).collect();
        assert_eq!(names, vec!["alpha", "mu", "zeta"]);
    }

    #[test]
    fn stray_non_vault_entries_are_skipped_not_fatal() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("vaults")).unwrap();
        // A file (not a directory) sitting inside vaults/.
        std::fs::write(dir.path().join("vaults").join("README.txt"), b"hi").unwrap();
        // A directory whose name is not a valid VaultName.
        std::fs::create_dir_all(dir.path().join("vaults").join("..bad..")).unwrap();

        let store = store_in(dir.path());
        let good = VaultName::new("good").unwrap();
        store.create(&good).unwrap();

        assert_eq!(store.list().unwrap(), vec![good]);
    }

    #[test]
    fn corrupt_state_file_reports_corrupt_error() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path()).unwrap();
        std::fs::write(dir.path().join("state.json"), b"{ not json ]").unwrap();
        let store = store_in(dir.path());
        assert!(matches!(store.current(), Err(VaultError::StateCorrupt { .. })));
    }

    #[test]
    fn newer_state_schema_version_is_rejected() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path()).unwrap();
        std::fs::write(
            dir.path().join("state.json"),
            br#"{"version": 999, "current_vault": "default"}"#,
        )
        .unwrap();
        let store = store_in(dir.path());
        assert!(matches!(
            store.current(),
            Err(VaultError::StateUnsupportedVersion {
                found: 999,
                supported: STATE_VERSION
            })
        ));
    }

    #[test]
    fn invalid_vault_name_in_state_file_is_rejected_on_load() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path()).unwrap();
        std::fs::write(
            dir.path().join("state.json"),
            br#"{"version": 1, "current_vault": ".."}"#,
        )
        .unwrap();
        let store = store_in(dir.path());
        assert!(matches!(store.current(), Err(VaultError::StateCorrupt { .. })));
    }

    #[test]
    fn state_write_never_leaves_the_file_truncated() {
        let dir = tempdir().unwrap();
        let store = store_in(dir.path());
        store.set_current(&VaultName::new("a").unwrap()).unwrap();
        store.set_current(&VaultName::new("b").unwrap()).unwrap();
        // The file must always parse as valid, current state — never empty
        // or half-written, since writes go through the atomic helper.
        assert_eq!(store.current().unwrap(), VaultName::new("b").unwrap());
    }
}

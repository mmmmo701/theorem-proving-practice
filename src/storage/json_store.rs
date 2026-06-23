//! File-backed [`Repository`] storing the whole library as one JSON file.
//!
//! Human-readable, diffable, trivially backed up. Each mutating operation does a
//! load-modify-save; writes are atomic (write to a temp file, fsync, rename over
//! the target) so a crash mid-write can never corrupt the library. Good enough
//! for a single-user prototype; the [`Repository`] trait lets a database back
//! end drop in later without touching anything above it.

use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use super::{Repository, StorageError};
use crate::domain::{Theorem, TheoremId};

/// Schema version this build writes and is the newest it understands. Bumped
/// when the on-disk format changes; older files remain readable, newer ones are
/// rejected with [`StorageError::UnsupportedVersion`].
const CURRENT_VERSION: u32 = 1;

/// On-disk envelope wrapping the theorem list with a schema version, so the
/// format can evolve without breaking existing libraries.
#[derive(Debug, Deserialize)]
struct StoreFile {
    #[serde(default)]
    theorems: Vec<Theorem>,
}

/// Borrowing counterpart used only for writing, to avoid cloning the list.
#[derive(Debug, Serialize)]
struct StoreFileRef<'a> {
    version: u32,
    theorems: &'a [Theorem],
}

/// Minimal probe that reads only the schema version, so a version mismatch is
/// reported as such even when the rest of the document would fail to parse
/// against the current `Theorem` shape.
#[derive(Debug, Deserialize)]
struct VersionProbe {
    version: u32,
}

/// A [`Repository`] backed by a single JSON file at a fixed path.
#[derive(Debug, Clone)]
pub struct JsonStore {
    path: PathBuf,
}

impl JsonStore {
    /// Create a store reading from / writing to `path`. The file need not exist
    /// yet — until something is added, the library is simply empty.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Read and parse the store, returning an empty list if the file is absent.
    fn load(&self) -> Result<Vec<Theorem>, StorageError> {
        let bytes = match std::fs::read(&self.path) {
            Ok(bytes) => bytes,
            // A missing store is an empty library, not an error.
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => {
                return Err(StorageError::Read {
                    path: self.path.clone(),
                    source,
                });
            }
        };

        // Check the version before a full parse, so a future format is reported
        // as UnsupportedVersion rather than Corrupt.
        let probe: VersionProbe = serde_json::from_slice(&bytes).map_err(|source| {
            StorageError::Corrupt {
                path: self.path.clone(),
                source,
            }
        })?;
        if probe.version > CURRENT_VERSION {
            return Err(StorageError::UnsupportedVersion {
                found: probe.version,
                supported: CURRENT_VERSION,
            });
        }

        let store: StoreFile =
            serde_json::from_slice(&bytes).map_err(|source| StorageError::Corrupt {
                path: self.path.clone(),
                source,
            })?;
        Ok(store.theorems)
    }

    /// Serialize and atomically replace the store file with `theorems`.
    fn save(&self, theorems: &[Theorem]) -> Result<(), StorageError> {
        let json = serde_json::to_vec_pretty(&StoreFileRef {
            version: CURRENT_VERSION,
            theorems,
        })
        .map_err(|source| StorageError::Serialize { source })?;

        let dir = self.parent_dir();
        std::fs::create_dir_all(dir).map_err(|source| StorageError::Write {
            path: dir.to_path_buf(),
            source,
        })?;

        // Write to a temp file in the *same* directory (so the final rename is a
        // cheap, atomic same-filesystem operation), fsync, then rename over the
        // target. A crash at any point leaves the old file intact.
        let mut tmp = NamedTempFile::new_in(dir).map_err(|source| StorageError::Write {
            path: dir.to_path_buf(),
            source,
        })?;
        tmp.write_all(&json)
            .and_then(|()| tmp.as_file().sync_all())
            .map_err(|source| StorageError::Write {
                path: self.path.clone(),
                source,
            })?;
        tmp.persist(&self.path).map_err(|err| StorageError::Write {
            path: self.path.clone(),
            source: err.error,
        })?;
        Ok(())
    }

    /// Directory containing the store file, defaulting to the current directory
    /// when the path has no parent component.
    fn parent_dir(&self) -> &Path {
        match self.path.parent() {
            Some(p) if !p.as_os_str().is_empty() => p,
            _ => Path::new("."),
        }
    }
}

impl Repository for JsonStore {
    fn add(&mut self, theorem: Theorem) -> Result<(), StorageError> {
        let mut theorems = self.load()?;
        theorems.push(theorem);
        self.save(&theorems)
    }

    fn remove(&mut self, id: &TheoremId) -> Result<bool, StorageError> {
        let mut theorems = self.load()?;
        let before = theorems.len();
        theorems.retain(|t| &t.id != id);
        if theorems.len() == before {
            return Ok(false);
        }
        self.save(&theorems)?;
        Ok(true)
    }

    fn update(&mut self, theorem: Theorem) -> Result<bool, StorageError> {
        let mut theorems = self.load()?;
        let Some(slot) = theorems.iter_mut().find(|t| t.id == theorem.id) else {
            return Ok(false);
        };
        *slot = theorem;
        self.save(&theorems)?;
        Ok(true)
    }

    fn get(&self, id: &TheoremId) -> Result<Option<Theorem>, StorageError> {
        Ok(self.load()?.into_iter().find(|t| &t.id == id))
    }

    fn all(&self) -> Result<Vec<Theorem>, StorageError> {
        self.load()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample(subject: &str, name: &str) -> Theorem {
        Theorem::new(subject, name, "$a^2+b^2=c^2$").unwrap()
    }

    #[test]
    fn missing_file_is_an_empty_library() {
        let dir = tempdir().unwrap();
        let store = JsonStore::new(dir.path().join("theorems.json"));
        assert_eq!(store.all().unwrap(), Vec::new());
        assert_eq!(store.count().unwrap(), 0);
    }

    #[test]
    fn add_persists_across_store_instances() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("theorems.json");

        let t = sample("Analysis", "MCT");
        let id = t.id;
        {
            let mut store = JsonStore::new(&path);
            store.add(t.clone()).unwrap();
        }

        // A fresh instance reads what the previous one wrote.
        let store = JsonStore::new(&path);
        assert_eq!(store.count().unwrap(), 1);
        assert_eq!(store.get(&id).unwrap(), Some(t));
    }

    #[test]
    fn multiple_adds_accumulate_in_order() {
        let dir = tempdir().unwrap();
        let mut store = JsonStore::new(dir.path().join("theorems.json"));
        store.add(sample("A", "one")).unwrap();
        store.add(sample("B", "two")).unwrap();
        let all = store.all().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].name.as_str(), "one");
        assert_eq!(all[1].name.as_str(), "two");
    }

    #[test]
    fn remove_deletes_the_matching_theorem_and_reports_true() {
        let dir = tempdir().unwrap();
        let mut store = JsonStore::new(dir.path().join("theorems.json"));
        let keep = sample("A", "keep");
        let drop = sample("B", "drop");
        store.add(keep.clone()).unwrap();
        store.add(drop.clone()).unwrap();

        assert!(store.remove(&drop.id).unwrap());
        let all = store.all().unwrap();
        assert_eq!(all, vec![keep]);
    }

    #[test]
    fn remove_unknown_id_is_false_and_leaves_the_library_intact() {
        let dir = tempdir().unwrap();
        let mut store = JsonStore::new(dir.path().join("theorems.json"));
        store.add(sample("A", "one")).unwrap();

        assert!(!store.remove(&TheoremId::new()).unwrap());
        assert_eq!(store.count().unwrap(), 1);
    }

    #[test]
    fn update_replaces_the_matching_theorem_and_reports_true() {
        let dir = tempdir().unwrap();
        let mut store = JsonStore::new(dir.path().join("theorems.json"));
        let mut t = sample("A", "one");
        store.add(t.clone()).unwrap();

        t.record_draw(chrono::Utc::now());
        assert!(store.update(t.clone()).unwrap());

        let stored = store.get(&t.id).unwrap().unwrap();
        assert_eq!(stored.draw_count, 1);
        assert_eq!(stored, t);
    }

    #[test]
    fn update_unknown_id_is_false_and_leaves_the_library_intact() {
        let dir = tempdir().unwrap();
        let mut store = JsonStore::new(dir.path().join("theorems.json"));
        let kept = sample("A", "one");
        store.add(kept.clone()).unwrap();

        // A theorem with an id not in the store changes nothing.
        let stranger = sample("B", "two");
        assert!(!store.update(stranger).unwrap());
        assert_eq!(store.all().unwrap(), vec![kept]);
    }

    #[test]
    fn get_unknown_id_returns_none() {
        let dir = tempdir().unwrap();
        let mut store = JsonStore::new(dir.path().join("theorems.json"));
        store.add(sample("A", "one")).unwrap();
        assert_eq!(store.get(&TheoremId::new()).unwrap(), None);
    }

    #[test]
    fn missing_parent_directory_is_created_on_save() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("a").join("b").join("theorems.json");
        let mut store = JsonStore::new(&nested);
        store.add(sample("A", "one")).unwrap();
        assert!(nested.exists());
    }

    #[test]
    fn corrupt_file_reports_corrupt_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("theorems.json");
        std::fs::write(&path, b"{ this is not json ]").unwrap();
        let store = JsonStore::new(&path);
        assert!(matches!(
            store.all(),
            Err(StorageError::Corrupt { .. })
        ));
    }

    #[test]
    fn newer_schema_version_is_rejected() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("theorems.json");
        std::fs::write(&path, br#"{"version": 999, "theorems": []}"#).unwrap();
        let store = JsonStore::new(&path);
        assert!(matches!(
            store.all(),
            Err(StorageError::UnsupportedVersion {
                found: 999,
                supported: CURRENT_VERSION
            })
        ));
    }

    #[test]
    fn written_file_has_current_version_envelope() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("theorems.json");
        let mut store = JsonStore::new(&path);
        store.add(sample("A", "one")).unwrap();

        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(value["version"], CURRENT_VERSION);
        assert!(value["theorems"].is_array());
    }
}

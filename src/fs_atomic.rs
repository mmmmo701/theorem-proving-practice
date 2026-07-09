//! Shared low-level helper: atomic file writes.
//!
//! Write to a temp file in the same directory as the target (so the final
//! rename is a same-filesystem, atomic operation), fsync, then rename over
//! the target. A crash at any point leaves the previous file intact. Used by
//! both `storage::JsonStore` and `vaults::FsVaultStore`, whose on-disk state
//! must never be observable half-written.

use std::io::Write;
use std::path::Path;

use tempfile::NamedTempFile;

/// Atomically replace the file at `path` with `bytes`, creating parent
/// directories as needed.
pub fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = parent_dir(path);
    std::fs::create_dir_all(dir)?;
    let mut tmp = NamedTempFile::new_in(dir)?;
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|err| err.error)?;
    Ok(())
}

/// The directory containing `path`, defaulting to the current directory when
/// `path` has no parent component.
fn parent_dir(path: &Path) -> &Path {
    match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn writes_and_creates_parent_dirs() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("a").join("b").join("file.json");
        write(&path, b"hello").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"hello");
    }

    #[test]
    fn overwrites_existing_file_atomically() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("file.json");
        write(&path, b"first").unwrap();
        write(&path, b"second").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"second");
    }
}

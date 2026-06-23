//! Failures from the persistence layer.

use std::path::PathBuf;

/// Something went wrong reading or writing the theorem store.
///
/// A *missing* store file is deliberately **not** an error: it simply means an
/// empty library, and the repository handles it without surfacing a variant
/// here.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("could not read the theorem store at {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not write the theorem store at {path}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("the theorem store at {path} is corrupt or not valid JSON")]
    Corrupt {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("could not serialize the theorem store")]
    Serialize {
        #[source]
        source: serde_json::Error,
    },

    #[error(
        "theorem store schema version {found} is newer than this build supports ({supported}); upgrade the tool"
    )]
    UnsupportedVersion { found: u32, supported: u32 },
}

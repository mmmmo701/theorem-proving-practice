//! Failures from the vault layer.

use std::path::PathBuf;

use crate::domain::VaultName;
use crate::storage::StorageError;

/// Something went wrong managing vaults or the current-vault pointer.
#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("vault '{name}' does not exist; available vaults: {}", available.join(", "))]
    NotFound { name: String, available: Vec<String> },

    #[error("vault '{name}' already exists")]
    AlreadyExists { name: VaultName },

    #[error("could not read the vaults directory {path}")]
    ListDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not create vault directory {path}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not write the initial store for vault '{name}'")]
    InitialStore {
        name: VaultName,
        #[source]
        source: StorageError,
    },

    #[error("could not read the vault state at {path}")]
    StateRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not write the vault state at {path}")]
    StateWrite {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("the vault state at {path} is corrupt or not valid JSON")]
    StateCorrupt {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error(
        "vault state schema version {found} is newer than this build supports ({supported}); upgrade the tool"
    )]
    StateUnsupportedVersion { found: u32, supported: u32 },

    #[error(
        "both a pre-vault library at {legacy} and vault 'default' at {default_store} exist; \
         remove or rename one before continuing"
    )]
    MigrationConflict {
        legacy: PathBuf,
        default_store: PathBuf,
    },

    #[error("could not migrate the pre-vault library from {from} to {to}")]
    MigrationIo {
        from: PathBuf,
        to: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

impl VaultError {
    /// Process exit code for this error: `1` for a bad name or an unknown
    /// vault the user named directly (`--vault`, `vault switch`, a duplicate
    /// `vault add`); `2` for everything else, which reflects broken or
    /// unreadable on-disk state rather than a mistaken argument.
    pub fn exit_code(&self) -> u8 {
        match self {
            VaultError::NotFound { .. } | VaultError::AlreadyExists { .. } => 1,
            VaultError::ListDir { .. }
            | VaultError::CreateDir { .. }
            | VaultError::InitialStore { .. }
            | VaultError::StateRead { .. }
            | VaultError::StateWrite { .. }
            | VaultError::StateCorrupt { .. }
            | VaultError::StateUnsupportedVersion { .. }
            | VaultError::MigrationConflict { .. }
            | VaultError::MigrationIo { .. } => 2,
        }
    }
}

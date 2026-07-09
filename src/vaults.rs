//! Vault layer: named, isolated theorem libraries.
//!
//! A vault is a directory under `vaults/<name>/` holding that vault's own
//! `theorems.json` (and, lazily, its own output subdirectory). The
//! [`VaultStore`] trait is the extension point (mirroring
//! `storage::Repository`); the only implementation is [`FsVaultStore`], which
//! treats the *directory listing* of `vaults/` as the single source of truth
//! for which vaults exist — there is deliberately no separate registry file
//! that could drift out of sync with the directories on disk.
//!
//! [`migrate_legacy_layout`] handles the one-time move of a pre-vault flat
//! store into the `default` vault.

mod error;
mod fs_store;
mod migrate;

pub use error::VaultError;
pub use fs_store::FsVaultStore;
pub use migrate::migrate_legacy_layout;

use crate::domain::VaultName;

/// Operations on the set of vaults and which one is current.
///
/// A missing current-vault pointer (nothing ever persisted) is not an error:
/// [`VaultStore::current`] reports [`VaultName::default_vault`] in that case,
/// mirroring how an absent theorem store means an empty library rather than a
/// failure.
pub trait VaultStore {
    /// All vaults, in a stable (sorted) order.
    fn list(&self) -> Result<Vec<VaultName>, VaultError>;

    /// Whether a vault with this name exists.
    fn exists(&self, name: &VaultName) -> Result<bool, VaultError>;

    /// Create a new, empty vault, writing its (empty) theorem store
    /// immediately. Errors with [`VaultError::AlreadyExists`] if one with
    /// this name already exists.
    fn create(&self, name: &VaultName) -> Result<(), VaultError>;

    /// The persisted current vault, or [`VaultName::default_vault`] if none
    /// has ever been set.
    fn current(&self) -> Result<VaultName, VaultError>;

    /// Persist `name` as the current vault. Does not require the vault to
    /// exist (recovery from a broken current-vault pointer goes through
    /// `vault add` then `vault switch`, both of which call this).
    fn set_current(&self, name: &VaultName) -> Result<(), VaultError>;
}

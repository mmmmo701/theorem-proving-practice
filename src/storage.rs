//! Storage layer: persistence behind the [`Repository`] trait.
//!
//! The trait is deliberately tiny so it is easy to re-implement (SQLite, etc.)
//! and easy to fake in tests. The prototype ships one implementation,
//! [`JsonStore`].

mod error;
mod json_store;

pub use error::StorageError;
pub use json_store::JsonStore;

use crate::domain::{Theorem, TheoremId};

/// A persistent collection of theorems.
///
/// Implementations own *how* and *where* theorems are stored; callers depend
/// only on this interface. Methods return [`StorageError`] for genuine I/O or
/// data problems — note that an absent store is **not** an error, it is an empty
/// library.
pub trait Repository {
    /// Persist a new theorem.
    fn add(&mut self, theorem: Theorem) -> Result<(), StorageError>;

    /// Remove the theorem with the given id. Returns `true` if one was removed,
    /// `false` if no theorem had that id (which is not an error).
    fn remove(&mut self, id: &TheoremId) -> Result<bool, StorageError>;

    /// Replace the stored theorem that shares `theorem.id`. Returns `true` if a
    /// theorem was replaced, `false` if none had that id (not an error). Used to
    /// persist updated draw stats; also the seam for a future edit feature.
    fn update(&mut self, theorem: Theorem) -> Result<bool, StorageError>;

    /// Fetch a theorem by id, or `None` if no such theorem exists.
    fn get(&self, id: &TheoremId) -> Result<Option<Theorem>, StorageError>;

    /// Return every stored theorem.
    fn all(&self) -> Result<Vec<Theorem>, StorageError>;

    /// Number of stored theorems. Default implementation counts [`all`]; a
    /// backend with a cheaper count can override it.
    ///
    /// [`all`]: Repository::all
    fn count(&self) -> Result<usize, StorageError> {
        Ok(self.all()?.len())
    }
}

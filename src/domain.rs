//! Domain layer: the core types and rules, with no I/O.
//!
//! Everything here is pure data plus validation. This is what keeps storage,
//! selection, and rendering swappable.

mod error;
mod theorem;
mod vault;

pub use error::DomainError;
pub use theorem::{LatexContent, Name, Subject, Theorem, TheoremId};
pub use vault::{VaultName, DEFAULT_VAULT_NAME};

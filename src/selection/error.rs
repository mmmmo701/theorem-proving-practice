//! Failures when drawing theorems from the library.

/// The requested draw could not be satisfied.
#[derive(Debug, thiserror::Error)]
pub enum SelectionError {
    #[error("the theorem library is empty; add some theorems first")]
    EmptyPool,

    #[error("draw count must be greater than zero")]
    ZeroCount,

    #[error("requested {requested} theorems but only {available} are stored")]
    NotEnough { requested: usize, available: usize },
}

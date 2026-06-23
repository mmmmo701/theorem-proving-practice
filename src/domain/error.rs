//! Validation failures when constructing domain values.

/// A domain invariant was violated while building a [`crate::domain`] value.
///
/// Variants carry the offending field name so messages are specific and
/// actionable. `field` is a `&'static str` because it always names a known
/// struct field, never user input.
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("{field} must not be empty")]
    Empty { field: &'static str },

    #[error("{field} must be at most {max} characters (got {actual})")]
    TooLong {
        field: &'static str,
        max: usize,
        actual: usize,
    },

    #[error("{field} contains disallowed control characters")]
    ControlChars { field: &'static str },
}

//! [`VaultName`]: the validated identifier for a vault.
//!
//! A vault name becomes a directory name on disk (see `vaults::fs_store`), so
//! validation here is a safety boundary, not cosmetics: the allowed character
//! set makes path traversal, hidden files, and cross-filesystem oddities
//! impossible to construct, rather than trying to blocklist them.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::DomainError;

/// Maximum length, in characters, of a vault name.
const MAX_VAULT_NAME_LEN: usize = 64;

/// Name of the implicit vault that a fresh install (or a migrated pre-vault
/// library) starts in.
pub const DEFAULT_VAULT_NAME: &str = "default";

/// A validated vault name: non-empty, length-bounded, and restricted to
/// `[a-z0-9_-]` with an alphanumeric first character.
///
/// Constructed from arbitrary input via [`VaultName::new`], which trims and
/// lowercase-folds it — so `"Exams"` and `"exams"` name the same vault, which
/// also sidesteps case-insensitive-filesystem collisions on macOS/Windows.
/// The allowed character set rules out path separators, `.`/`..`, spaces,
/// control characters, and Unicode lookalikes by construction.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct VaultName(String);

impl VaultName {
    /// Validate and construct a vault name. Surrounding whitespace is
    /// trimmed and the result is lowercase-folded.
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let raw = raw.into();
        let field = "vault name";
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(DomainError::Empty { field });
        }
        let len = trimmed.chars().count();
        if len > MAX_VAULT_NAME_LEN {
            return Err(DomainError::TooLong {
                field,
                max: MAX_VAULT_NAME_LEN,
                actual: len,
            });
        }

        let lowered = trimmed.to_lowercase();
        let mut chars = lowered.chars();
        // Non-empty was already checked above, so a first char always exists.
        let starts_alnum = chars.next().is_some_and(|c| c.is_ascii_alphanumeric());
        let rest_ok = chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
        if !starts_alnum || !rest_ok {
            return Err(DomainError::InvalidVaultName { field });
        }

        Ok(Self(lowered))
    }

    /// The name of the implicit vault a fresh install starts in.
    pub fn default_vault() -> Self {
        Self(DEFAULT_VAULT_NAME.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for VaultName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<VaultName> for String {
    fn from(v: VaultName) -> Self {
        v.0
    }
}
impl TryFrom<String> for VaultName {
    type Error = DomainError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_simple_lowercase_name() {
        let v = VaultName::new("exams").unwrap();
        assert_eq!(v.as_str(), "exams");
    }

    #[test]
    fn trims_and_lowercase_folds() {
        let v = VaultName::new("  Exams  ").unwrap();
        assert_eq!(v.as_str(), "exams");
    }

    #[test]
    fn allows_digits_hyphen_and_underscore() {
        assert_eq!(VaultName::new("exams-2026").unwrap().as_str(), "exams-2026");
        assert_eq!(VaultName::new("exams_2026").unwrap().as_str(), "exams_2026");
        assert_eq!(VaultName::new("2026exams").unwrap().as_str(), "2026exams");
    }

    #[test]
    fn empty_or_whitespace_only_is_rejected() {
        assert!(matches!(
            VaultName::new("   "),
            Err(DomainError::Empty { field: "vault name" })
        ));
    }

    #[test]
    fn overlong_name_is_rejected() {
        let long = "a".repeat(MAX_VAULT_NAME_LEN + 1);
        assert!(matches!(
            VaultName::new(long),
            Err(DomainError::TooLong {
                field: "vault name",
                max: MAX_VAULT_NAME_LEN,
                ..
            })
        ));
    }

    #[test]
    fn leading_hyphen_or_underscore_is_rejected() {
        assert!(matches!(
            VaultName::new("-exams"),
            Err(DomainError::InvalidVaultName { field: "vault name" })
        ));
        assert!(matches!(
            VaultName::new("_exams"),
            Err(DomainError::InvalidVaultName { field: "vault name" })
        ));
    }

    #[test]
    fn path_traversal_and_separators_are_rejected() {
        for bad in ["..", "a/b", "a\\b", "a.b", "a b", "a\0b"] {
            assert!(
                matches!(
                    VaultName::new(bad),
                    Err(DomainError::InvalidVaultName { .. }) | Err(DomainError::Empty { .. })
                ),
                "expected {bad:?} to be rejected"
            );
        }
    }

    #[test]
    fn unicode_lookalikes_are_rejected() {
        assert!(matches!(
            VaultName::new("café"),
            Err(DomainError::InvalidVaultName { field: "vault name" })
        ));
    }

    #[test]
    fn default_vault_is_valid_and_named_default() {
        assert_eq!(VaultName::default_vault().as_str(), DEFAULT_VAULT_NAME);
    }

    #[test]
    fn serde_round_trips() {
        let v = VaultName::new("exams").unwrap();
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, "\"exams\"");
        let back: VaultName = serde_json::from_str(&json).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn deserializing_invalid_name_fails() {
        assert!(serde_json::from_str::<VaultName>("\"..\"").is_err());
        assert!(serde_json::from_str::<VaultName>("\"\"").is_err());
    }
}

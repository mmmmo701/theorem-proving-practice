//! The [`Theorem`] aggregate and its validated field newtypes.
//!
//! Invariants are enforced in the newtype constructors, so an invalid
//! [`Theorem`] cannot be built. Validation also runs on deserialization (via
//! `#[serde(try_from = "String")]`), which guards against a hand-edited store
//! smuggling in bad data.

use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::DomainError;

/// Maximum length, in characters, of the single-line `Subject` and `Name`
/// fields. Generous, but bounds pathological input.
const MAX_LABEL_LEN: usize = 200;

/// Maximum length, in characters, of a theorem's LaTeX content.
const MAX_CONTENT_LEN: usize = 100_000;

/// Stable, unique identifier for a theorem.
///
/// Generated on creation and never changes, so identity survives later edits to
/// the subject, name, or content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TheoremId(Uuid);

impl TheoremId {
    /// Generate a fresh, random identifier.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// The underlying UUID.
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for TheoremId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TheoremId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// The mathematical subject a theorem belongs to (e.g. "Real Analysis").
///
/// A trimmed, non-empty, single-line label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Subject(String);

impl Subject {
    /// Validate and construct a subject. Surrounding whitespace is trimmed.
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        Ok(Self(validate_label(raw.into(), "subject")?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The name of a theorem (e.g. "Monotone Convergence Theorem").
///
/// A trimmed, non-empty, single-line label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Name(String);

impl Name {
    /// Validate and construct a name. Surrounding whitespace is trimmed.
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        Ok(Self(validate_label(raw.into(), "name")?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The body of a theorem, as raw LaTeX.
///
/// Non-empty and length-bounded. Whitespace and line breaks are preserved (they
/// are meaningful in LaTeX); only NUL and other non-whitespace control
/// characters are rejected. This is intentionally *not* a LaTeX parser.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct LatexContent(String);

impl LatexContent {
    /// Validate and construct LaTeX content. Content is stored verbatim (not
    /// trimmed) to preserve formatting.
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let raw = raw.into();
        let field = "content";
        if raw.trim().is_empty() {
            return Err(DomainError::Empty { field });
        }
        let len = raw.chars().count();
        if len > MAX_CONTENT_LEN {
            return Err(DomainError::TooLong {
                field,
                max: MAX_CONTENT_LEN,
                actual: len,
            });
        }
        // Newlines, tabs, and carriage returns are legitimate in LaTeX source;
        // anything else in the control range (NUL, escapes, etc.) is rejected.
        if raw
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t' | '\r'))
        {
            return Err(DomainError::ControlChars { field });
        }
        Ok(Self(raw))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Shared validation for the single-line label newtypes (`Subject`, `Name`):
/// trim, require non-empty, bound length, reject control characters.
fn validate_label(raw: String, field: &'static str) -> Result<String, DomainError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(DomainError::Empty { field });
    }
    let len = trimmed.chars().count();
    if len > MAX_LABEL_LEN {
        return Err(DomainError::TooLong {
            field,
            max: MAX_LABEL_LEN,
            actual: len,
        });
    }
    if trimmed.chars().any(char::is_control) {
        return Err(DomainError::ControlChars { field });
    }
    Ok(trimmed.to_string())
}

// `From`/`TryFrom<String>` power both the public constructors' ergonomics and
// the serde `into`/`try_from` round-trip.
impl From<Subject> for String {
    fn from(v: Subject) -> Self {
        v.0
    }
}
impl TryFrom<String> for Subject {
    type Error = DomainError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}
impl From<Name> for String {
    fn from(v: Name) -> Self {
        v.0
    }
}
impl TryFrom<String> for Name {
    type Error = DomainError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}
impl From<LatexContent> for String {
    fn from(v: LatexContent) -> Self {
        v.0
    }
}
impl TryFrom<String> for LatexContent {
    type Error = DomainError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

/// A theorem: its identity, classification, statement, and when it was added.
///
/// Construct via [`Theorem::new`], which validates the fields and stamps a
/// fresh id and timestamp. [`Theorem::from_parts`] reconstructs one from
/// already-validated components (e.g. when loading from storage).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Theorem {
    pub id: TheoremId,
    pub subject: Subject,
    pub name: Name,
    pub content: LatexContent,
    pub added_at: DateTime<Utc>,
    /// Number of times this theorem has been drawn for practice. Drives the
    /// forgetting-curve weighting (see `selection/forgetting.rs`).
    #[serde(default)]
    pub draw_count: u32,
    /// When this theorem was last drawn, or `None` if never. `#[serde(default)]`
    /// keeps pre-forgetting-curve stores loadable (the field defaults to `None`).
    #[serde(default)]
    pub last_drawn_at: Option<DateTime<Utc>>,
}

impl Theorem {
    /// Validate raw inputs and create a new theorem, stamping a fresh
    /// [`TheoremId`] and the current UTC time as `added_at`.
    pub fn new(
        subject: impl Into<String>,
        name: impl Into<String>,
        content: impl Into<String>,
    ) -> Result<Self, DomainError> {
        Ok(Self {
            id: TheoremId::new(),
            subject: Subject::new(subject)?,
            name: Name::new(name)?,
            content: LatexContent::new(content)?,
            added_at: Utc::now(),
            draw_count: 0,
            last_drawn_at: None,
        })
    }

    /// Reconstruct a theorem from already-validated parts, without changing its
    /// identity or timestamp.
    pub fn from_parts(
        id: TheoremId,
        subject: Subject,
        name: Name,
        content: LatexContent,
        added_at: DateTime<Utc>,
        draw_count: u32,
        last_drawn_at: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            id,
            subject,
            name,
            content,
            added_at,
            draw_count,
            last_drawn_at,
        }
    }

    /// Record that this theorem was drawn at `at`: bump the draw count
    /// (saturating, so it can never overflow) and stamp the time. This is the
    /// reinforcement step of the forgetting-curve model.
    pub fn record_draw(&mut self, at: DateTime<Utc>) {
        self.draw_count = self.draw_count.saturating_add(1);
        self.last_drawn_at = Some(at);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subject_trims_and_accepts_valid_input() {
        let s = Subject::new("  Real Analysis  ").unwrap();
        assert_eq!(s.as_str(), "Real Analysis");
    }

    #[test]
    fn empty_or_whitespace_label_is_rejected() {
        assert!(matches!(
            Subject::new("   "),
            Err(DomainError::Empty { field: "subject" })
        ));
        assert!(matches!(
            Name::new(""),
            Err(DomainError::Empty { field: "name" })
        ));
    }

    #[test]
    fn overlong_label_is_rejected() {
        let long = "x".repeat(MAX_LABEL_LEN + 1);
        assert!(matches!(
            Name::new(long),
            Err(DomainError::TooLong {
                field: "name",
                max: MAX_LABEL_LEN,
                ..
            })
        ));
    }

    #[test]
    fn control_characters_in_label_are_rejected() {
        assert!(matches!(
            Subject::new("Real\nAnalysis"),
            Err(DomainError::ControlChars { field: "subject" })
        ));
        assert!(matches!(
            Name::new("bad\u{0}name"),
            Err(DomainError::ControlChars { field: "name" })
        ));
    }

    #[test]
    fn content_preserves_whitespace_and_newlines() {
        let raw = "\\begin{theorem}\n  $a^2 + b^2 = c^2$\n\\end{theorem}\n";
        let c = LatexContent::new(raw).unwrap();
        assert_eq!(c.as_str(), raw);
    }

    #[test]
    fn empty_content_is_rejected() {
        assert!(matches!(
            LatexContent::new("   \n\t "),
            Err(DomainError::Empty { field: "content" })
        ));
    }

    #[test]
    fn content_rejects_nul_but_allows_tabs() {
        assert!(LatexContent::new("a\tb").is_ok());
        assert!(matches!(
            LatexContent::new("a\u{0}b"),
            Err(DomainError::ControlChars { field: "content" })
        ));
    }

    #[test]
    fn theorem_new_stamps_id_and_timestamp() {
        let before = Utc::now();
        let t = Theorem::new("Analysis", "MCT", "$\\lim$").unwrap();
        let after = Utc::now();
        assert_eq!(t.subject.as_str(), "Analysis");
        assert_eq!(t.name.as_str(), "MCT");
        assert!(t.added_at >= before && t.added_at <= after);
    }

    #[test]
    fn new_theorem_starts_undrawn() {
        let t = Theorem::new("Analysis", "MCT", "$\\lim$").unwrap();
        assert_eq!(t.draw_count, 0);
        assert_eq!(t.last_drawn_at, None);
    }

    #[test]
    fn record_draw_bumps_count_and_stamps_time() {
        let mut t = Theorem::new("Analysis", "MCT", "$\\lim$").unwrap();
        let at = Utc::now();
        t.record_draw(at);
        assert_eq!(t.draw_count, 1);
        assert_eq!(t.last_drawn_at, Some(at));

        let later = at + chrono::Duration::days(1);
        t.record_draw(later);
        assert_eq!(t.draw_count, 2);
        assert_eq!(t.last_drawn_at, Some(later));
    }

    #[test]
    fn legacy_json_without_draw_fields_loads_with_defaults() {
        // A store written before the forgetting-curve fields existed must still
        // deserialize, defaulting to "never drawn".
        let json = r#"{
            "id": "00000000-0000-0000-0000-000000000000",
            "subject": "Topology",
            "name": "Urysohn",
            "content": "$f: X \\to [0,1]$",
            "added_at": "2026-06-18T00:00:00Z"
        }"#;
        let t: Theorem = serde_json::from_str(json).unwrap();
        assert_eq!(t.draw_count, 0);
        assert_eq!(t.last_drawn_at, None);
    }

    #[test]
    fn theorem_new_propagates_field_errors() {
        assert!(matches!(
            Theorem::new("Analysis", "", "content"),
            Err(DomainError::Empty { field: "name" })
        ));
    }

    #[test]
    fn distinct_theorems_get_distinct_ids() {
        let a = Theorem::new("S", "N", "C").unwrap();
        let b = Theorem::new("S", "N", "C").unwrap();
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn theorem_json_roundtrips_and_revalidates() {
        let t = Theorem::new("Topology", "Urysohn", "$f: X \\to [0,1]$").unwrap();
        let json = serde_json::to_string(&t).unwrap();
        let back: Theorem = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn deserializing_invalid_field_fails() {
        // An empty subject smuggled into the store must be rejected on load.
        let json = r#"{
            "id": "00000000-0000-0000-0000-000000000000",
            "subject": "",
            "name": "N",
            "content": "C",
            "added_at": "2026-06-18T00:00:00Z"
        }"#;
        assert!(serde_json::from_str::<Theorem>(json).is_err());
    }
}

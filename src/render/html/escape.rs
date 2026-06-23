//! Encode the three structural HTML characters so a string is safe to drop into
//! element text.
//!
//! Used for *both* text metadata (subject, name, dates) and theorem *content*.
//! For content this is the minimal encoding required for valid markup —
//! MathJax decodes these entities back before typesetting, so `$0 < x$` renders
//! exactly as authored. Only `&`, `<`, `>` are touched; backslashes, `$`, and
//! everything else pass through unchanged so the LaTeX/math is preserved.

/// Encode `&`, `<`, and `>` as HTML entities, leaving all other characters
/// (notably `\` and `$`) untouched.
pub fn escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            // `&` must be handled first so the entities below aren't re-encoded.
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_is_unchanged() {
        assert_eq!(escape("Real Analysis"), "Real Analysis");
    }

    #[test]
    fn each_structural_character_is_encoded() {
        assert_eq!(escape("&"), "&amp;");
        assert_eq!(escape("<"), "&lt;");
        assert_eq!(escape(">"), "&gt;");
    }

    #[test]
    fn ampersand_is_not_double_encoded() {
        // The `<` becomes `&lt;`, and that leading `&` must stay literal.
        assert_eq!(escape("a < b & c > d"), "a &lt; b &amp; c &gt; d");
    }

    #[test]
    fn backslashes_and_dollars_pass_through() {
        // Math content must survive verbatim apart from the structural chars.
        assert_eq!(escape("$\\leq$"), "$\\leq$");
        assert_eq!(escape("\\begin{aligned} a &= b \\end{aligned}"),
                   "\\begin{aligned} a &amp;= b \\end{aligned}");
    }
}

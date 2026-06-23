//! Escape user-controlled *text* (subject, name, formatted date) so it is
//! interpolated into LaTeX safely.
//!
//! This applies only to metadata. Theorem *content* is deliberately passed
//! through raw — the whole point is that the user writes LaTeX/math there.

/// Escape the ten LaTeX special characters so `input` renders as literal text.
pub fn escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '\\' => out.push_str("\\textbackslash{}"),
            '&' => out.push_str("\\&"),
            '%' => out.push_str("\\%"),
            '$' => out.push_str("\\$"),
            '#' => out.push_str("\\#"),
            '_' => out.push_str("\\_"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '~' => out.push_str("\\textasciitilde{}"),
            '^' => out.push_str("\\textasciicircum{}"),
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
    fn each_special_character_is_escaped() {
        assert_eq!(escape("&"), "\\&");
        assert_eq!(escape("%"), "\\%");
        assert_eq!(escape("$"), "\\$");
        assert_eq!(escape("#"), "\\#");
        assert_eq!(escape("_"), "\\_");
        assert_eq!(escape("{"), "\\{");
        assert_eq!(escape("}"), "\\}");
        assert_eq!(escape("~"), "\\textasciitilde{}");
        assert_eq!(escape("^"), "\\textasciicircum{}");
        assert_eq!(escape("\\"), "\\textbackslash{}");
    }

    #[test]
    fn mixed_content_is_escaped_in_place() {
        assert_eq!(
            escape("a_b & 100% of $x$"),
            "a\\_b \\& 100\\% of \\$x\\$"
        );
    }

    #[test]
    fn backslash_does_not_double_escape_following_chars() {
        // A literal backslash followed by '&' must not produce a command.
        assert_eq!(escape("\\&"), "\\textbackslash{}\\&");
    }
}

//! Assemble a self-contained HTML document from a set of theorems.
//!
//! Pure string construction — no I/O — so it is fully unit-testable. Math is
//! typeset in the browser by MathJax (loaded from a CDN). Both metadata and
//! content are run through [`escape`], which encodes only `&`/`<`/`>`; MathJax
//! decodes those back before typesetting, so the math renders as authored.

use chrono::Local;

use super::escape::escape;
use crate::domain::Theorem;

/// Document head: charset/viewport, a small readable stylesheet, and the MathJax
/// loader configured so existing `$...$` content is treated as inline math.
const HEAD: &str = "\
<head>
<meta charset=\"utf-8\">
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">
<title>Theorem Practice</title>
<style>
  body { max-width: 48rem; margin: 2rem auto; padding: 0 1rem;
         font-family: Georgia, 'Times New Roman', serif; line-height: 1.5;
         color: #1a1a1a; }
  header { text-align: center; margin-bottom: 2rem; }
  header .title { font-size: 1.6rem; font-weight: bold; }
  header .date { color: #555; }
  section { margin: 1.5rem 0; }
  h2 { margin-bottom: 0.25rem; }
  .meta { color: #555; font-size: 0.9rem; margin-top: 0; }
  hr { border: none; border-top: 1px solid #ccc; margin: 2rem 0; }
  .empty { text-align: center; color: #555; }
</style>
<script>
  window.MathJax = { tex: { inlineMath: [['$', '$'], ['\\\\(', '\\\\)']] } };
</script>
<script async src=\"https://cdn.jsdelivr.net/npm/mathjax@3/es5/tex-mml-chtml.js\"></script>
</head>
";

/// Build a complete, self-contained HTML document presenting `theorems`.
pub fn build_document(theorems: &[Theorem]) -> String {
    let mut doc = String::with_capacity(HEAD.len() + 512);
    doc.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n");
    doc.push_str(HEAD);
    doc.push_str("<body>\n");
    doc.push_str(&header());

    if theorems.is_empty() {
        doc.push_str("<p class=\"empty\">No theorems were drawn.</p>\n");
    } else {
        for (i, theorem) in theorems.iter().enumerate() {
            if i > 0 {
                doc.push_str("<hr>\n");
            }
            doc.push_str(&theorem_block(theorem));
        }
    }

    doc.push_str("</body>\n</html>\n");
    doc
}

/// Centered sheet title with the date the sheet was generated.
fn header() -> String {
    let today = escape(&Local::now().format("%B %-d, %Y").to_string());
    format!(
        "<header>\n\
         <div class=\"title\">Theorem Practice</div>\n\
         <div class=\"date\">{today}</div>\n\
         </header>\n"
    )
}

/// One formatted block: name as a heading, a subject/date line, then the
/// content (escaped only for the structural HTML characters).
fn theorem_block(theorem: &Theorem) -> String {
    let name = escape(theorem.name.as_str());
    let subject = escape(theorem.subject.as_str());
    let date = escape(&theorem.added_at.format("%B %-d, %Y").to_string());
    let content = escape(theorem.content.as_str());
    format!(
        "<section>\n\
         <h2>{name}</h2>\n\
         <p class=\"meta\"><strong>Subject:</strong> {subject} &middot; \
         <strong>Date Added:</strong> {date}</p>\n\
         <div class=\"content\">\n{content}\n</div>\n\
         </section>\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theorem(subject: &str, name: &str, content: &str) -> Theorem {
        Theorem::new(subject, name, content).unwrap()
    }

    #[test]
    fn document_has_html_skeleton() {
        let doc = build_document(&[theorem("S", "N", "$x$")]);
        assert!(doc.starts_with("<!DOCTYPE html>"));
        assert!(doc.contains("<head>"));
        assert!(doc.contains("</head>"));
        assert!(doc.contains("<body>"));
        assert!(doc.trim_end().ends_with("</html>"));
    }

    #[test]
    fn loads_mathjax_with_dollar_inline_delimiters() {
        let doc = build_document(&[theorem("S", "N", "$x$")]);
        assert!(doc.contains("mathjax@3"));
        assert!(doc.contains("inlineMath"));
    }

    #[test]
    fn metadata_is_escaped_but_math_delimiters_survive() {
        let doc = build_document(&[theorem("Set <Theory>", "A & B", "$a_1 \\leq b_1$")]);
        // Metadata text is HTML-encoded...
        assert!(doc.contains("Set &lt;Theory&gt;"));
        assert!(doc.contains("A &amp; B"));
        // ...content keeps its `$` delimiters and backslash command verbatim.
        assert!(doc.contains("$a_1 \\leq b_1$"));
    }

    #[test]
    fn content_structural_chars_are_encoded_for_the_browser() {
        // `<` is common in math (`0 < x < 1`); it must be encoded so MathJax
        // (which decodes it) sees the intended inequality and the markup stays
        // valid.
        let doc = build_document(&[theorem("S", "N", "$0 < x < 1$")]);
        assert!(doc.contains("$0 &lt; x &lt; 1$"));
    }

    #[test]
    fn content_alignment_ampersand_is_encoded() {
        let doc = build_document(&[theorem("S", "N", "\\begin{aligned} a &= b \\end{aligned}")]);
        assert!(doc.contains("a &amp;= b"));
    }

    #[test]
    fn multiple_theorems_are_separated() {
        let doc = build_document(&[theorem("S", "One", "$1$"), theorem("S", "Two", "$2$")]);
        assert!(doc.contains("<h2>One</h2>"));
        assert!(doc.contains("<h2>Two</h2>"));
        assert!(doc.contains("<hr>"));
    }

    #[test]
    fn empty_input_still_produces_a_valid_document() {
        let doc = build_document(&[]);
        assert!(doc.starts_with("<!DOCTYPE html>"));
        assert!(doc.trim_end().ends_with("</html>"));
        assert!(doc.contains("No theorems were drawn"));
    }
}

//! Assemble the LaTeX document from a set of theorems.
//!
//! Pure string construction — no I/O — so it is fully unit-testable without a
//! LaTeX engine. Metadata is escaped; content is emitted verbatim.

use chrono::Local;

use super::escape::escape;
use crate::domain::Theorem;

/// Preamble: a plain article with the math packages, plus common `amsthm`
/// environments pre-defined so user content can use `\begin{theorem}`,
/// `\begin{lemma}`, etc. out of the box.
const PREAMBLE: &str = "\
\\documentclass[11pt]{article}
\\usepackage[margin=1in]{geometry}
\\usepackage{physics}
\\usepackage{amsmath}
\\usepackage{amssymb}
\\usepackage{amsthm}
\\usepackage{parskip}
\\newtheorem{theorem}{Theorem}
\\newtheorem{lemma}{Lemma}
\\newtheorem{corollary}{Corollary}
\\newtheorem{proposition}{Proposition}
\\newtheorem{definition}{Definition}
\\setlength{\\parindent}{0pt}
";

/// Build a complete, compilable LaTeX document presenting `theorems`.
pub fn build_document(theorems: &[Theorem]) -> String {
    let mut doc = String::with_capacity(PREAMBLE.len() + 512);
    doc.push_str(PREAMBLE);
    doc.push_str("\\begin{document}\n\n");
    doc.push_str(&header());

    if theorems.is_empty() {
        doc.push_str("\\noindent No theorems were drawn.\n\n");
    } else {
        for (i, theorem) in theorems.iter().enumerate() {
            if i > 0 {
                doc.push_str("\\bigskip\n\\hrule\n\\bigskip\n\n");
            }
            doc.push_str(&theorem_block(theorem));
        }
    }

    doc.push_str("\\end{document}\n");
    doc
}

/// Centered sheet title with the date the sheet was generated.
fn header() -> String {
    let today = escape(&Local::now().format("%B %-d, %Y").to_string());
    format!(
        "\\begin{{center}}\n\
         {{\\Large\\textbf{{Theorem Practice}}}}\\\\[2pt]\n\
         {today}\n\
         \\end{{center}}\n\\vspace{{1em}}\n\n"
    )
}

/// One formatted block: name as a heading, subject and date added on a line,
/// then the raw content.
fn theorem_block(theorem: &Theorem) -> String {
    let name = escape(theorem.name.as_str());
    let subject = escape(theorem.subject.as_str());
    let date = escape(&theorem.added_at.format("%B %-d, %Y").to_string());
    format!(
        "\\section*{{{name}}}\n\
         \\noindent\\textbf{{Subject:}} {subject} \\hfill \\textbf{{Date Added:}} {date}\n\n\
         \\medskip\n\n\
         {content}\n\n",
        content = theorem.content.as_str(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theorem(subject: &str, name: &str, content: &str) -> Theorem {
        Theorem::new(subject, name, content).unwrap()
    }

    #[test]
    fn document_has_preamble_and_body_delimiters() {
        let doc = build_document(&[theorem("S", "N", "$x$")]);
        assert!(doc.contains("\\documentclass[11pt]{article}"));
        assert!(doc.contains("\\begin{document}"));
        assert!(doc.contains("\\end{document}"));
    }

    #[test]
    fn metadata_is_escaped_but_content_is_raw() {
        let doc = build_document(&[theorem("Set_Theory", "A & B", "$a_1 \\leq b_1$")]);
        // Subject and name (text) are escaped...
        assert!(doc.contains("Set\\_Theory"));
        assert!(doc.contains("A \\& B"));
        // ...content is emitted verbatim (the underscore is real math here).
        assert!(doc.contains("$a_1 \\leq b_1$"));
    }

    #[test]
    fn theorem_environments_are_predefined() {
        let doc = build_document(&[theorem("S", "N", "\\begin{lemma}x\\end{lemma}")]);
        assert!(doc.contains("\\newtheorem{lemma}"));
        assert!(doc.contains("\\begin{lemma}x\\end{lemma}"));
    }

    #[test]
    fn multiple_theorems_are_separated() {
        let doc = build_document(&[theorem("S", "One", "$1$"), theorem("S", "Two", "$2$")]);
        assert!(doc.contains("\\section*{One}"));
        assert!(doc.contains("\\section*{Two}"));
        assert!(doc.contains("\\hrule"));
    }

    #[test]
    fn empty_input_still_produces_a_valid_document() {
        let doc = build_document(&[]);
        assert!(doc.contains("\\begin{document}"));
        assert!(doc.contains("\\end{document}"));
        assert!(doc.contains("No theorems were drawn"));
    }
}

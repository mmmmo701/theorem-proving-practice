//! HTML [`Renderer`] implementation.
//!
//! Builds a single self-contained HTML document (math typeset in-browser by
//! MathJax) and writes it to the destination atomically. No external engine is
//! involved, so — unlike the LaTeX renderer — this needs no toolchain and its
//! tests run in the normal suite.

mod escape;
mod template;

use std::io::Write;
use std::path::Path;

use tempfile::NamedTempFile;

use super::{RenderError, RenderOutput, Renderer};
use crate::domain::Theorem;

/// Renders theorems to a self-contained HTML file.
#[derive(Debug, Clone, Default)]
pub struct HtmlRenderer;

impl HtmlRenderer {
    /// Create an HTML renderer.
    pub fn new() -> Self {
        Self
    }
}

impl Renderer for HtmlRenderer {
    fn render(&self, theorems: &[Theorem], dest: &Path) -> Result<RenderOutput, RenderError> {
        let document = template::build_document(theorems);
        write_atomically(document.as_bytes(), dest)?;
        Ok(RenderOutput {
            path: dest.to_path_buf(),
        })
    }
}

/// Write `bytes` to `dest`, creating its parent directory and writing
/// atomically (temp file in the destination dir, then rename) — mirroring how
/// the LaTeX renderer places its PDF.
fn write_atomically(bytes: &[u8], dest: &Path) -> Result<(), RenderError> {
    let dir = match dest.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    std::fs::create_dir_all(dir).map_err(|source| RenderError::Output {
        path: dir.to_path_buf(),
        source,
    })?;

    let mut tmp = NamedTempFile::new_in(dir).map_err(|source| RenderError::Output {
        path: dir.to_path_buf(),
        source,
    })?;
    tmp.write_all(bytes)
        .and_then(|()| tmp.as_file().sync_all())
        .map_err(|source| RenderError::Output {
            path: dest.to_path_buf(),
            source,
        })?;
    tmp.persist(dest).map_err(|err| RenderError::Output {
        path: dest.to_path_buf(),
        source: err.error,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn theorem() -> Theorem {
        Theorem::new("Analysis", "MCT", "$\\int f = \\lim \\int f_n$").unwrap()
    }

    #[test]
    fn writes_an_html_file_at_the_destination() {
        let renderer = HtmlRenderer::new();
        let dir = tempdir().unwrap();
        let dest = dir.path().join("out.html");

        let out = renderer.render(&[theorem()], &dest).unwrap();

        assert_eq!(out.path, dest);
        let text = std::fs::read_to_string(&dest).unwrap();
        assert!(text.starts_with("<!DOCTYPE html>"));
        assert!(text.contains("MCT"));
    }

    #[test]
    fn creates_missing_parent_directories() {
        let renderer = HtmlRenderer::new();
        let dir = tempdir().unwrap();
        let dest = dir.path().join("nested").join("deeper").join("out.html");

        renderer.render(&[theorem()], &dest).unwrap();

        assert!(dest.exists());
    }

    #[test]
    fn empty_input_still_writes_a_document() {
        let renderer = HtmlRenderer::new();
        let dir = tempdir().unwrap();
        let dest = dir.path().join("out.html");

        renderer.render(&[], &dest).unwrap();

        let text = std::fs::read_to_string(&dest).unwrap();
        assert!(text.contains("No theorems were drawn"));
    }
}

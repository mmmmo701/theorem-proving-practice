//! LaTeX-to-PDF [`Renderer`] implementation.
//!
//! Pipeline: build the document, write it into an isolated temp
//! build directory, run the engine there, then place the resulting PDF at the
//! destination. The temp directory keeps `.aux`/`.log` litter out of the repo,
//! and is preserved only when compilation fails (so its log can be inspected).

mod engine;
mod escape;
mod template;

use std::io::Write;
use std::path::Path;
use std::time::Duration;

use tempfile::NamedTempFile;

use super::{RenderError, RenderOutput, Renderer};
use crate::domain::Theorem;

/// Default ceiling on a single compilation, guarding against a hung engine.
const DEFAULT_TIMEOUT_SECS: u64 = 120;
/// Name of the source file written into the build directory.
const TEX_FILENAME: &str = "main.tex";

/// Renders theorems to PDF by driving an external LaTeX engine.
#[derive(Debug, Clone)]
pub struct LatexRenderer {
    engine: String,
    timeout: Duration,
}

impl LatexRenderer {
    /// Create a renderer using `engine` (e.g. `"latexmk"`, `"pdflatex"`).
    pub fn new(engine: impl Into<String>) -> Self {
        Self {
            engine: engine.into(),
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        }
    }

    /// Override the per-compilation timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

impl Renderer for LatexRenderer {
    fn render(&self, theorems: &[Theorem], dest: &Path) -> Result<RenderOutput, RenderError> {
        let document = template::build_document(theorems);

        let build = tempfile::Builder::new()
            .prefix("tpp-build-")
            .tempdir()
            .map_err(|source| RenderError::BuildDir { source })?;

        std::fs::write(build.path().join(TEX_FILENAME), document)
            .map_err(|source| RenderError::BuildDir { source })?;

        let pdf = match engine::compile(&self.engine, build.path(), TEX_FILENAME, self.timeout) {
            Ok(pdf) => pdf,
            Err(err) => {
                // Preserve the build dir when the compile itself failed: its log
                // is the diagnostic, and the error message points the user to it.
                if matches!(err, RenderError::Compilation { .. }) {
                    let _ = build.keep();
                }
                return Err(err);
            }
        };

        place_pdf(&pdf, dest)?;
        Ok(RenderOutput {
            path: dest.to_path_buf(),
        })
    }
}

/// Copy the produced PDF to `dest`, creating its parent directory and writing
/// atomically (temp file in the destination dir, then rename).
fn place_pdf(src: &Path, dest: &Path) -> Result<(), RenderError> {
    let dir = match dest.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    std::fs::create_dir_all(dir).map_err(|source| RenderError::Output {
        path: dir.to_path_buf(),
        source,
    })?;

    let bytes = std::fs::read(src).map_err(|source| RenderError::Output {
        path: src.to_path_buf(),
        source,
    })?;

    let mut tmp = NamedTempFile::new_in(dir).map_err(|source| RenderError::Output {
        path: dir.to_path_buf(),
        source,
    })?;
    tmp.write_all(&bytes)
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
    fn missing_engine_is_reported_clearly() {
        let renderer = LatexRenderer::new("definitely-not-a-real-engine-xyz");
        let dir = tempdir().unwrap();
        let dest = dir.path().join("out.pdf");

        let err = renderer.render(&[theorem()], &dest).unwrap_err();
        assert!(matches!(err, RenderError::EngineNotFound { .. }));
        assert!(!dest.exists());
    }

    // Requires a real LaTeX install; run with `cargo test -- --ignored`.
    #[test]
    #[ignore = "requires a LaTeX engine (latexmk) installed"]
    fn compiles_a_real_pdf() {
        let renderer = LatexRenderer::new("latexmk");
        let t1 = theorem();
        let t2 = Theorem::new(
            "Topology",
            "Urysohn's Lemma",
            "\\begin{lemma}Normal spaces separate closed sets.\\end{lemma}",
        )
        .unwrap();

        let dir = tempdir().unwrap();
        let dest = dir.path().join("nested").join("out.pdf");
        let out = renderer.render(&[t1, t2], &dest).unwrap();

        assert_eq!(out.path, dest);
        let bytes = std::fs::read(&out.path).unwrap();
        assert!(bytes.starts_with(b"%PDF"), "output is not a PDF");
    }

    // Requires a real LaTeX install; run with `cargo test -- --ignored`.
    #[test]
    #[ignore = "requires a LaTeX engine (latexmk) installed"]
    fn broken_latex_yields_compilation_error_with_log_and_kept_build_dir() {
        let renderer = LatexRenderer::new("latexmk");
        // `\undefinedcommand` is not defined and fails under -halt-on-error.
        let broken = Theorem::new("S", "N", "\\thisCommandDoesNotExist").unwrap();
        let dir = tempdir().unwrap();
        let dest = dir.path().join("out.pdf");

        let err = renderer.render(&[broken], &dest).unwrap_err();
        match err {
            RenderError::Compilation {
                build_dir,
                log_tail,
                ..
            } => {
                assert!(build_dir.exists(), "build dir should be kept on failure");
                assert!(!log_tail.is_empty() && log_tail != "(no log file was produced)");
                std::fs::remove_dir_all(&build_dir).ok();
            }
            other => panic!("expected Compilation error, got {other:?}"),
        }
        assert!(!dest.exists(), "no PDF should be written on failure");
    }
}

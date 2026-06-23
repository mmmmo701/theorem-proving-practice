//! Render layer: turning drawn theorems into output, behind the [`Renderer`]
//! trait.
//!
//! The prototype ships [`LatexRenderer`], which assembles a LaTeX document and
//! drives an engine to produce a PDF. Alternative outputs (HTML, Markdown, …)
//! would implement the same trait.

mod error;
mod html;
mod latex;

pub use error::RenderError;
pub use html::HtmlRenderer;
pub use latex::LatexRenderer;

use std::path::{Path, PathBuf};

use crate::domain::Theorem;

/// The output format a draw renders to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum OutputFormat {
    /// LaTeX-compiled PDF (requires a LaTeX engine on `PATH`).
    #[default]
    Pdf,
    /// Self-contained HTML, with math typeset in-browser by MathJax.
    Html,
}

impl OutputFormat {
    /// File extension for this format's artifact (no leading dot).
    pub fn extension(self) -> &'static str {
        match self {
            OutputFormat::Pdf => "pdf",
            OutputFormat::Html => "html",
        }
    }
}

/// The result of a successful render: where the artifact was written.
#[derive(Debug, Clone)]
pub struct RenderOutput {
    pub path: PathBuf,
}

/// A strategy for turning a set of theorems into a rendered artifact.
pub trait Renderer {
    /// Render `theorems` into a single artifact at `dest`, returning where it
    /// was written.
    fn render(&self, theorems: &[Theorem], dest: &Path) -> Result<RenderOutput, RenderError>;
}

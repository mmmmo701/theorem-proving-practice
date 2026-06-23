//! Failures from the rendering pipeline — the most failure-prone path, so its
//! variants are deliberately detailed and diagnosable.

use std::path::PathBuf;

/// Something went wrong assembling the document or driving the LaTeX engine.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("LaTeX engine '{engine}' was not found on PATH; {hint}")]
    EngineNotFound { engine: String, hint: String },

    #[error("could not prepare the LaTeX build directory")]
    BuildDir {
        #[source]
        source: std::io::Error,
    },

    #[error("could not invoke LaTeX engine '{engine}'")]
    Spawn {
        engine: String,
        #[source]
        source: std::io::Error,
    },

    #[error(
        "LaTeX compilation failed: engine '{engine}' exited with status {code}. \
         Build files kept at {build_dir} for inspection.\n--- log tail ---\n{log_tail}"
    )]
    Compilation {
        engine: String,
        code: i32,
        build_dir: PathBuf,
        log_tail: String,
    },

    #[error("LaTeX engine '{engine}' timed out after {seconds}s")]
    Timeout { engine: String, seconds: u64 },

    #[error("LaTeX engine reported success but produced no PDF at {expected}")]
    MissingOutput { expected: PathBuf },

    #[error("could not write the output file to {path}")]
    Output {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

//! Drive an external LaTeX engine to compile a prepared `.tex` file into a PDF.
//!
//! Engine stdout/stderr are discarded; diagnostics come from the `.log` file
//! the engine writes, whose tail we surface on failure. Discarding the pipes
//! also sidesteps the classic deadlock of polling `try_wait` while a full pipe
//! blocks the child.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use super::super::RenderError;

/// Number of trailing log lines included in a compilation error.
const LOG_TAIL_LINES: usize = 40;

/// Compile `tex_filename` (located in `build_dir`) with `engine`, returning the
/// path to the produced PDF.
pub fn compile(
    engine: &str,
    build_dir: &Path,
    tex_filename: &str,
    timeout: Duration,
) -> Result<PathBuf, RenderError> {
    let args = engine_args(engine, tex_filename);
    log::info!("compiling with {engine} in {}", build_dir.display());
    log::debug!("engine args: {args:?}");

    let mut command = Command::new(engine);
    command
        .current_dir(build_dir)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let mut child = command.spawn().map_err(|source| {
        if source.kind() == ErrorKind::NotFound {
            RenderError::EngineNotFound {
                engine: engine.to_string(),
                hint: format!("install '{engine}' or configure a different LaTeX engine"),
            }
        } else {
            RenderError::Spawn {
                engine: engine.to_string(),
                source,
            }
        }
    })?;

    let status = match wait_with_timeout(&mut child, timeout) {
        Ok(Some(status)) => status,
        Ok(None) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(RenderError::Timeout {
                engine: engine.to_string(),
                seconds: timeout.as_secs(),
            });
        }
        Err(source) => {
            return Err(RenderError::Spawn {
                engine: engine.to_string(),
                source,
            });
        }
    };

    if !status.success() {
        return Err(RenderError::Compilation {
            engine: engine.to_string(),
            code: status.code().unwrap_or(-1),
            build_dir: build_dir.to_path_buf(),
            log_tail: read_log_tail(build_dir, tex_filename),
        });
    }

    let pdf_path = build_dir.join(Path::new(tex_filename).with_extension("pdf"));
    if !pdf_path.exists() {
        return Err(RenderError::MissingOutput { expected: pdf_path });
    }
    Ok(pdf_path)
}

/// Engine-appropriate arguments. `latexmk` needs `-pdf` to select PDF output;
/// the `*latex` engines produce PDF directly and would reject it.
fn engine_args(engine: &str, tex_filename: &str) -> Vec<String> {
    let base = Path::new(engine)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(engine);

    let mut args = Vec::new();
    if base == "latexmk" {
        args.push("-pdf".to_string());
    }
    args.push("-interaction=nonstopmode".to_string());
    args.push("-halt-on-error".to_string());
    args.push(tex_filename.to_string());
    args
}

/// Wait for the child, polling so we can enforce a timeout. `Ok(None)` means the
/// timeout elapsed and the caller should kill the child.
fn wait_with_timeout(
    child: &mut Child,
    timeout: Duration,
) -> std::io::Result<Option<ExitStatus>> {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if start.elapsed() >= timeout {
            return Ok(None);
        }
        sleep(Duration::from_millis(50));
    }
}

/// Read the last [`LOG_TAIL_LINES`] lines of the engine's `.log` file, for
/// diagnostics. Best-effort: returns a placeholder if the log is unavailable.
fn read_log_tail(build_dir: &Path, tex_filename: &str) -> String {
    let log_path = build_dir.join(Path::new(tex_filename).with_extension("log"));
    match std::fs::read_to_string(&log_path) {
        Ok(contents) => {
            let lines: Vec<&str> = contents.lines().collect();
            let start = lines.len().saturating_sub(LOG_TAIL_LINES);
            lines[start..].join("\n")
        }
        Err(_) => "(no log file was produced)".to_string(),
    }
}

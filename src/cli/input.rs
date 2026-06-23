//! Terminal-input helpers for interactive entry: single-line prompts, a yes/no
//! confirmation, and an `$EDITOR`-backed editor for multi-line content.
//!
//! These live in the CLI layer — the only place that talks to the terminal —
//! and keep the `add` handler readable. Prompts are written to stderr so that a
//! command's real output on stdout stays clean.

use std::io::{self, Read, Write};
use std::process::Command;

use crate::cli::CliError;

/// Prompt on stderr for a single line and return it trimmed.
///
/// `default`, when given, is shown in brackets and returned if the user just
/// presses Enter.
pub fn prompt_line(label: &str, default: Option<&str>) -> Result<String, CliError> {
    let mut err = io::stderr();
    match default {
        Some(d) if !d.is_empty() => write!(err, "{label} [{d}]: ")?,
        _ => write!(err, "{label}: ")?,
    }
    err.flush()?;

    let mut line = String::new();
    let read = io::stdin().read_line(&mut line)?;
    if read == 0 {
        // EOF (Ctrl-D or a closed stdin): accept the default if there is one,
        // otherwise abort rather than spin on an unsatisfiable prompt.
        return default.map(str::to_string).ok_or(CliError::Aborted);
    }

    let trimmed = line.trim();
    Ok(match (trimmed.is_empty(), default) {
        // A bare Enter keeps the default, if any.
        (true, Some(d)) => d.to_string(),
        _ => trimmed.to_string(),
    })
}

/// Ask a yes/no question, defaulting to "no". Anything but `y`/`yes` is `false`.
pub fn confirm(question: &str) -> Result<bool, CliError> {
    let answer = prompt_line(&format!("{question} [y/N]"), None)?;
    Ok(parse_yes(&answer))
}

/// Parse an affirmative answer (case-insensitive `y`/`yes`).
fn parse_yes(answer: &str) -> bool {
    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

/// Open the user's editor on a temp file seeded with `initial`, returning the
/// saved contents.
///
/// Honors `$VISUAL`, then `$EDITOR`, falling back to `vi`. The editor string may
/// carry arguments (e.g. `code --wait`). The temp file is re-read by path after
/// the editor exits, so editors that replace the file in place are handled.
pub fn edit_in_editor(initial: &str) -> Result<String, CliError> {
    let mut file = tempfile::Builder::new()
        .prefix("theorem-")
        .suffix(".tex")
        .tempfile()?;
    file.write_all(initial.as_bytes())?;
    file.flush()?;

    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_string());
    let mut parts = editor.split_whitespace();
    let program = parts.next().unwrap_or("vi");

    let status = Command::new(program)
        .args(parts)
        .arg(file.path())
        .status()
        .map_err(|source| CliError::EditorLaunch {
            editor: editor.clone(),
            source,
        })?;
    if !status.success() {
        return Err(CliError::EditorFailed { editor });
    }

    let mut contents = String::new();
    std::fs::File::open(file.path())?.read_to_string(&mut contents)?;
    Ok(contents)
}

#[cfg(test)]
mod tests {
    use super::parse_yes;

    #[test]
    fn affirmative_answers_are_recognized_case_insensitively() {
        assert!(parse_yes("y"));
        assert!(parse_yes("Y"));
        assert!(parse_yes("yes"));
        assert!(parse_yes("  YES  "));
    }

    #[test]
    fn anything_else_is_negative() {
        assert!(!parse_yes(""));
        assert!(!parse_yes("n"));
        assert!(!parse_yes("nope"));
        assert!(!parse_yes("sure"));
    }
}

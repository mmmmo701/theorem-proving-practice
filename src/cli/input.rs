//! Terminal-input helpers for interactive entry: single-line prompts, a yes/no
//! confirmation, and an `$EDITOR`-backed editor for multi-line content.
//!
//! These live in the CLI layer — the only place that talks to the terminal —
//! and keep the `add` handler readable. Prompts are written to stderr so that a
//! command's real output on stdout stays clean.

use std::io::{self, Read, Write};
use std::path::Path;
use std::process::Command;

use crate::cli::CliError;

/// Env var overriding the program used to open generated files (e.g. set it to
/// `evince` or `firefox`). Whitespace-separated args are honored, like `$EDITOR`.
const OPENER_ENV: &str = "THEOREM_PROVING_PRACTICE_OPENER";

/// Platform default file opener, used when [`OPENER_ENV`] is unset.
#[cfg(target_os = "macos")]
const DEFAULT_OPENER: &str = "open";
#[cfg(not(target_os = "macos"))]
const DEFAULT_OPENER: &str = "xdg-open";

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

/// Loop prompting `prompt` (on stderr) so the user can pick an item by bare
/// number or by free-text lookup, retrying on an unmatched entry. Returns
/// `None` if the user cancels with a blank line.
///
/// `count` is the number of numbered items shown above the prompt (used only
/// for the "choose a number between..." message); `by_number` maps a 1-based
/// choice to an item. `resolve` looks up free text (typically a name/id
/// prefix): `Ok(None)` means "no match, already reported via `eprintln!`, let
/// the user retry"; any `Err` aborts the menu immediately rather than
/// retrying, since it signals a real failure (e.g. a storage error), not a
/// bad selection.
///
/// Shared by every `-i` numbered-menu prompt (`delete -i`, `open`, `vault
/// switch -i`) so each only has to print its menu and describe how to look an
/// entry up.
pub fn pick_from_menu<T>(
    prompt: &str,
    count: usize,
    by_number: impl Fn(usize) -> Option<T>,
    resolve: impl Fn(&str) -> Result<Option<T>, CliError>,
) -> Result<Option<T>, CliError> {
    loop {
        let choice = prompt_line(prompt, None)?;
        if choice.is_empty() {
            return Ok(None);
        }

        if let Ok(n) = choice.parse::<usize>() {
            if let Some(item) = (n >= 1).then(|| by_number(n)).flatten() {
                return Ok(Some(item));
            }
            eprintln!("  ! choose a number between 1 and {count}");
            continue;
        }

        if let Some(item) = resolve(&choice)? {
            return Ok(Some(item));
        }
    }
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

/// Open `path` with the system file handler (or the [`OPENER_ENV`] override).
///
/// Defaults to `open` on macOS and `xdg-open` elsewhere. We wait for the opener
/// and treat a non-zero exit as a failure; many openers return immediately
/// after handing off to the real viewer, so this rarely blocks for long.
pub fn open_in_viewer(path: &Path) -> Result<(), CliError> {
    let opener = opener_program();
    let mut parts = opener.split_whitespace();
    let program = parts.next().unwrap_or(DEFAULT_OPENER);

    let status = Command::new(program)
        .args(parts)
        .arg(path)
        .status()
        .map_err(|source| CliError::OpenerLaunch {
            opener: opener.clone(),
            source,
        })?;
    if !status.success() {
        return Err(CliError::OpenerFailed { opener });
    }
    Ok(())
}

/// The configured opener command, honoring [`OPENER_ENV`] (empty = unset).
fn opener_program() -> String {
    std::env::var(OPENER_ENV)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_OPENER.to_string())
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

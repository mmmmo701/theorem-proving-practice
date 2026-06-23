//! `open` command: pick a generated output file from the output directory (or
//! name one directly) and hand it to the system viewer.

use std::path::{Path, PathBuf};

use crate::app::App;
use crate::cli::args::OpenArgs;
use crate::cli::{CliError, input};

pub fn run(app: &App, args: OpenArgs) -> Result<(), CliError> {
    let outputs = app.list_outputs()?;
    if outputs.is_empty() {
        return Err(CliError::NoOutputs);
    }

    let chosen = match args.name {
        // A name (or unique substring) skips the menu.
        Some(name) => resolve_by_name(&outputs, &name)?,
        None => match select_from_menu(&outputs)? {
            Some(path) => path,
            None => {
                eprintln!("Cancelled; nothing opened.");
                return Ok(());
            }
        },
    };

    eprintln!("Opening {}", chosen.display());
    input::open_in_viewer(&chosen)
}

/// Present a numbered menu of output files and return the chosen path, or `None`
/// if the user cancels with a blank entry.
fn select_from_menu(outputs: &[PathBuf]) -> Result<Option<PathBuf>, CliError> {
    eprintln!("Generated outputs:");
    for (i, path) in outputs.iter().enumerate() {
        eprintln!("{:>3}. {}", i + 1, file_label(path));
    }

    loop {
        let choice = input::prompt_line("Open which? (number or name, blank to cancel)", None)?;
        if choice.is_empty() {
            return Ok(None);
        }

        // A bare number selects by position in the menu above.
        if let Ok(n) = choice.parse::<usize>() {
            match outputs.get(n.wrapping_sub(1)) {
                Some(path) if n >= 1 => return Ok(Some(path.clone())),
                _ => {
                    eprintln!("  ! choose a number between 1 and {}", outputs.len());
                    continue;
                }
            }
        }

        // Otherwise treat it as a file name / substring.
        match resolve_by_name(outputs, &choice) {
            Ok(path) => return Ok(Some(path)),
            Err(CliError::OutputNotFound { .. }) => eprintln!("  ! no output matches '{choice}'"),
            Err(CliError::AmbiguousOutput { count, .. }) => {
                eprintln!("  ! '{choice}' matches {count} files; be more specific")
            }
            Err(other) => return Err(other),
        }
    }
}

/// Resolve a query to exactly one output file: an exact (case-insensitive) file
/// name wins; otherwise a unique substring match. Zero or many become the
/// standard CLI errors.
fn resolve_by_name(outputs: &[PathBuf], query: &str) -> Result<PathBuf, CliError> {
    let needle = query.trim().to_lowercase();

    if let Some(path) = outputs
        .iter()
        .find(|p| file_label(p).to_lowercase() == needle)
    {
        return Ok(path.clone());
    }

    let hits: Vec<&PathBuf> = outputs
        .iter()
        .filter(|p| file_label(p).to_lowercase().contains(&needle))
        .collect();
    match hits.as_slice() {
        [] => Err(CliError::OutputNotFound {
            name: query.to_string(),
        }),
        [path] => Ok((*path).clone()),
        many => Err(CliError::AmbiguousOutput {
            name: query.to_string(),
            count: many.len(),
        }),
    }
}

/// The file name of `path` for display/matching, or a placeholder if it has none.
fn file_label(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("<unknown>")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outputs() -> Vec<PathBuf> {
        vec![
            PathBuf::from("/out/2026-06-23.pdf"),
            PathBuf::from("/out/2026-06-23.html"),
            PathBuf::from("/out/2026-06-22.pdf"),
        ]
    }

    #[test]
    fn exact_name_match_wins_over_substring() {
        let chosen = resolve_by_name(&outputs(), "2026-06-23.pdf").unwrap();
        assert_eq!(chosen, PathBuf::from("/out/2026-06-23.pdf"));
    }

    #[test]
    fn unique_substring_resolves() {
        let chosen = resolve_by_name(&outputs(), "06-22").unwrap();
        assert_eq!(chosen, PathBuf::from("/out/2026-06-22.pdf"));
    }

    #[test]
    fn ambiguous_substring_errors_with_count() {
        let err = resolve_by_name(&outputs(), "2026-06-23").unwrap_err();
        assert!(matches!(err, CliError::AmbiguousOutput { count: 2, .. }));
    }

    #[test]
    fn unknown_name_errors() {
        let err = resolve_by_name(&outputs(), "nope").unwrap_err();
        assert!(matches!(err, CliError::OutputNotFound { .. }));
    }
}

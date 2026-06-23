//! `add` command: gather a theorem from flags or interactively, then hand off
//! to `app::add`.

use std::path::PathBuf;

use crate::app::{AddRequest, App};
use crate::cli::args::AddArgs;
use crate::cli::{CliError, input};
use crate::domain::{DomainError, LatexContent, Name, Subject};

pub fn run(app: &mut App, args: AddArgs) -> Result<(), CliError> {
    let request = if args.interactive {
        gather_interactively(args)?
    } else {
        gather_from_flags(args)?
    };

    let theorem = app.add(request)?;

    println!("Added theorem {}", theorem.id);
    println!("  subject: {}", theorem.subject.as_str());
    println!("  name:    {}", theorem.name.as_str());
    Ok(())
}

/// Build the request from flags (the default, non-interactive mode). clap's
/// `required_unless_present` guarantees `subject`/`name` are present here; the
/// content source is enforced by [`resolve_content`].
fn gather_from_flags(args: AddArgs) -> Result<AddRequest, CliError> {
    let subject = args
        .subject
        .expect("clap requires --subject unless --interactive");
    let name = args
        .name
        .expect("clap requires --name unless --interactive");
    let content = resolve_content(args.content, args.content_file)?;
    Ok(AddRequest {
        subject,
        name,
        content,
    })
}

/// Resolve the LaTeX content from whichever flag was provided. The arg group
/// guarantees the two are not both set; neither being set is a user error.
fn resolve_content(inline: Option<String>, file: Option<PathBuf>) -> Result<String, CliError> {
    match (inline, file) {
        (Some(content), None) => Ok(content),
        (None, Some(path)) => std::fs::read_to_string(&path)
            .map_err(|source| CliError::ContentFileRead { path, source }),
        (None, None) => Err(CliError::MissingContent),
        (Some(_), Some(_)) => unreachable!("clap's arg group rejects both content sources"),
    }
}

/// Build the request by prompting for each field, opening the editor for the
/// content. Flags passed alongside `--interactive` pre-fill their fields.
fn gather_interactively(args: AddArgs) -> Result<AddRequest, CliError> {
    eprintln!("Enter the theorem's details (press Enter to accept a [default]).");

    let subject = prompt_label("Subject", args.subject, |s| Subject::new(s).map(drop))?;
    let name = prompt_label("Name", args.name, |s| Name::new(s).map(drop))?;
    let seed = resolve_seed(args.content, args.content_file)?;
    let content = prompt_content(seed)?;

    Ok(AddRequest {
        subject,
        name,
        content,
    })
}

/// Prompt for a single-line label, re-prompting until it validates. A rejected
/// entry becomes the next default so the user can edit rather than retype it.
fn prompt_label(
    label: &str,
    prefilled: Option<String>,
    validate: impl Fn(&str) -> Result<(), DomainError>,
) -> Result<String, CliError> {
    let mut default = prefilled;
    loop {
        let value = input::prompt_line(label, default.as_deref())?;
        match validate(&value) {
            Ok(()) => return Ok(value),
            Err(err) => {
                eprintln!("  ! {err}; please try again.");
                default = (!value.is_empty()).then_some(value);
            }
        }
    }
}

/// Open the editor for the content, re-validating after each save and offering
/// to re-open on failure (seeded with the just-edited text so work isn't lost).
fn prompt_content(initial: String) -> Result<String, CliError> {
    let mut seed = initial;
    loop {
        eprintln!("Opening your editor for the LaTeX content (save and quit when done)…");
        let content = input::edit_in_editor(&seed)?;
        match LatexContent::new(content.as_str()) {
            Ok(_) => return Ok(content),
            Err(err) => {
                eprintln!("  ! {err}");
                if !input::confirm("Re-open the editor to fix it?")? {
                    return Err(CliError::Aborted);
                }
                seed = content;
            }
        }
    }
}

/// Resolve the initial editor buffer for interactive mode: an inline `--content`
/// value, the contents of `--content-file`, or empty when neither is given.
fn resolve_seed(inline: Option<String>, file: Option<PathBuf>) -> Result<String, CliError> {
    match (inline, file) {
        (Some(content), _) => Ok(content),
        (None, Some(path)) => std::fs::read_to_string(&path)
            .map_err(|source| CliError::ContentFileRead { path, source }),
        (None, None) => Ok(String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_content_accepts_inline() {
        assert_eq!(
            resolve_content(Some("$x$".into()), None).unwrap(),
            "$x$".to_string()
        );
    }

    #[test]
    fn resolve_content_without_a_source_is_a_user_error() {
        assert!(matches!(
            resolve_content(None, None),
            Err(CliError::MissingContent)
        ));
    }

    #[test]
    fn resolve_seed_defaults_to_empty() {
        assert_eq!(resolve_seed(None, None).unwrap(), String::new());
    }

    #[test]
    fn resolve_seed_prefers_inline_content() {
        assert_eq!(resolve_seed(Some("seed".into()), None).unwrap(), "seed");
    }
}

//! `edit` command: resolve a theorem by id/prefix, gather replacement fields
//! (from flags or interactively), then hand off to `app::edit`.

use std::path::PathBuf;

use crate::app::{App, EditRequest};
use crate::cli::CliError;
use crate::cli::args::EditArgs;
use crate::domain::{Name, Subject, Theorem};

pub fn run(app: &mut App, args: EditArgs) -> Result<(), CliError> {
    let theorem = super::resolve_unique(app, &args.id)?;

    let request = if args.interactive {
        gather_interactively(&theorem, args)?
    } else {
        gather_from_flags(args)?
    };
    if request.is_empty() {
        return Err(CliError::NoEditsRequested);
    }

    let vault = app.vault_name().clone();
    match app.edit(&theorem.id, request)? {
        Some(updated) => {
            println!("Edited theorem {}", updated.id);
            println!("  subject: {}", updated.subject.as_str());
            println!("  name:    {}", updated.name.as_str());
            println!("  vault:   {vault}");
            Ok(())
        }
        // Resolved a moment ago but gone now (e.g. a concurrent delete).
        None => Err(CliError::TheoremNotFound {
            query: theorem.id.to_string(),
        }),
    }
}

/// Build the request from flags (the default, non-interactive mode). Every
/// field is optional; an all-empty request is rejected by the caller.
fn gather_from_flags(args: EditArgs) -> Result<EditRequest, CliError> {
    let content = match (args.content, args.content_file) {
        (Some(content), None) => Some(content),
        (None, Some(path)) => Some(
            std::fs::read_to_string(&path)
                .map_err(|source| CliError::ContentFileRead { path, source })?,
        ),
        (None, None) => None,
        (Some(_), Some(_)) => unreachable!("clap's arg group rejects both content sources"),
    };
    Ok(EditRequest {
        subject: args.subject,
        name: args.name,
        content,
    })
}

/// Build the request by prompting for every field, pre-filled with the
/// theorem's current values (or with flags passed alongside `--interactive`,
/// which take precedence). The editor opens seeded with the current content.
fn gather_interactively(current: &Theorem, args: EditArgs) -> Result<EditRequest, CliError> {
    eprintln!(
        "Editing theorem {} (press Enter to accept a [default]).",
        current.id
    );

    let subject = super::prompt_label(
        "Subject",
        Some(
            args.subject
                .unwrap_or_else(|| current.subject.as_str().to_string()),
        ),
        |s| Subject::new(s).map(drop),
    )?;
    let name = super::prompt_label(
        "Name",
        Some(args.name.unwrap_or_else(|| current.name.as_str().to_string())),
        |s| Name::new(s).map(drop),
    )?;
    let seed = resolve_seed(args.content, args.content_file, current)?;
    let content = super::prompt_content(seed)?;

    Ok(EditRequest {
        subject: Some(subject),
        name: Some(name),
        content: Some(content),
    })
}

/// Resolve the initial editor buffer for interactive mode: an inline
/// `--content` value, the contents of `--content-file`, or the theorem's
/// current content when neither is given.
fn resolve_seed(
    inline: Option<String>,
    file: Option<PathBuf>,
    current: &Theorem,
) -> Result<String, CliError> {
    match (inline, file) {
        (Some(content), _) => Ok(content),
        (None, Some(path)) => std::fs::read_to_string(&path)
            .map_err(|source| CliError::ContentFileRead { path, source }),
        (None, None) => Ok(current.content.as_str().to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flag_args(
        subject: Option<&str>,
        content: Option<&str>,
        content_file: Option<PathBuf>,
    ) -> EditArgs {
        EditArgs {
            id: "abc".into(),
            interactive: false,
            subject: subject.map(Into::into),
            name: None,
            content: content.map(Into::into),
            content_file,
        }
    }

    #[test]
    fn flags_without_content_leave_content_unset() {
        let request = gather_from_flags(flag_args(Some("Algebra"), None, None)).unwrap();
        assert_eq!(request.subject.as_deref(), Some("Algebra"));
        assert!(request.content.is_none());
    }

    #[test]
    fn flags_with_no_fields_build_an_empty_request() {
        let request = gather_from_flags(flag_args(None, None, None)).unwrap();
        assert!(request.is_empty());
    }

    #[test]
    fn resolve_seed_defaults_to_current_content() {
        let current = Theorem::new("S", "N", "$x^2$").unwrap();
        assert_eq!(resolve_seed(None, None, &current).unwrap(), "$x^2$");
    }

    #[test]
    fn resolve_seed_prefers_inline_content() {
        let current = Theorem::new("S", "N", "$x^2$").unwrap();
        assert_eq!(
            resolve_seed(Some("seed".into()), None, &current).unwrap(),
            "seed"
        );
    }
}

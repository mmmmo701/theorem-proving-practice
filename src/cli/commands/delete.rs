//! `delete` command: resolve a theorem (by id/prefix or from a menu), confirm,
//! then remove it via `app::delete`.

use crate::app::App;
use crate::cli::args::DeleteArgs;
use crate::cli::{CliError, input};
use crate::domain::Theorem;

pub fn run(app: &mut App, args: DeleteArgs) -> Result<(), CliError> {
    // Resolve which theorem to delete. Either path may decline (None).
    let theorem = if args.interactive {
        select_from_menu(app)?
    } else {
        let query = args.id.as_deref().ok_or(CliError::MissingId)?;
        Some(super::resolve_unique(app, query)?)
    };
    let Some(theorem) = theorem else {
        eprintln!("Cancelled; nothing deleted.");
        return Ok(());
    };

    if !args.yes {
        eprintln!(
            "About to delete  {}  {} — {}",
            theorem.id,
            theorem.subject.as_str(),
            theorem.name.as_str()
        );
        if !input::confirm("Delete this theorem?")? {
            eprintln!("Cancelled; nothing deleted.");
            return Ok(());
        }
    }

    let vault = app.vault_name().clone();
    if app.delete(&theorem.id)? {
        println!("Deleted theorem {} from vault '{vault}'", theorem.id);
        Ok(())
    } else {
        // Resolved a moment ago but gone now (e.g. a concurrent delete).
        Err(CliError::TheoremNotFound {
            query: theorem.id.to_string(),
        })
    }
}

/// Present a numbered menu of all theorems and return the chosen one, or `None`
/// if the library is empty or the user cancels with a blank entry.
fn select_from_menu(app: &App) -> Result<Option<Theorem>, CliError> {
    let theorems = app.list()?;
    if theorems.is_empty() {
        eprintln!("The library is empty; nothing to delete.");
        return Ok(None);
    }

    eprintln!("Theorems:");
    for (i, t) in theorems.iter().enumerate() {
        eprintln!(
            "{:>3}. {:.8}  {} — {}",
            i + 1,
            t.id.to_string(),
            t.subject.as_str(),
            t.name.as_str()
        );
    }

    input::pick_from_menu(
        "Delete which? (number or id, blank to cancel)",
        theorems.len(),
        |n| theorems.get(n - 1).cloned(),
        |choice| match super::resolve_unique(app, choice) {
            Ok(t) => Ok(Some(t)),
            Err(CliError::TheoremNotFound { .. }) => {
                eprintln!("  ! no theorem matches '{choice}'");
                Ok(None)
            }
            Err(CliError::AmbiguousId { count, .. }) => {
                eprintln!("  ! '{choice}' matches {count} theorems; use a longer prefix");
                Ok(None)
            }
            Err(other) => Err(other),
        },
    )
}

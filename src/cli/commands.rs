//! Per-command handlers: thin adapters that turn parsed arguments into an app
//! use-case call and present the result. Each handler stays small; all real
//! logic lives in the app layer and below.

pub mod add;
pub mod delete;
pub mod draw;
pub mod edit;
pub mod list;
pub mod open;
pub mod show;
pub mod vault;

use crate::app::App;
use crate::cli::{CliError, input};
use crate::domain::{DomainError, LatexContent, Theorem};

/// Resolve an id or unique id-prefix to exactly one theorem, mapping the
/// no-match and multiple-match cases to the standard CLI errors. Shared by the
/// `show` and `delete` handlers.
fn resolve_unique(app: &App, query: &str) -> Result<Theorem, CliError> {
    let matches = app.find_by_id_prefix(query)?;
    match matches.as_slice() {
        [] => Err(CliError::TheoremNotFound {
            query: query.to_string(),
        }),
        [t] => Ok(t.clone()),
        many => Err(CliError::AmbiguousId {
            query: query.to_string(),
            count: many.len(),
        }),
    }
}

/// Prompt for a single-line label, re-prompting until it validates. A rejected
/// entry becomes the next default so the user can edit rather than retype it.
/// Shared by the interactive modes of `add` and `edit`.
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
/// Shared by the interactive modes of `add` and `edit`.
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

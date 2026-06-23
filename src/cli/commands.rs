//! Per-command handlers: thin adapters that turn parsed arguments into an app
//! use-case call and present the result. Each handler stays small; all real
//! logic lives in the app layer and below.

pub mod add;
pub mod delete;
pub mod draw;
pub mod list;
pub mod show;

use crate::app::App;
use crate::cli::CliError;
use crate::domain::Theorem;

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

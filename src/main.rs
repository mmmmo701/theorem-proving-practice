//! Thin binary entry point.
//!
//! Responsibilities, and nothing more: initialize logging, hand off to
//! [`theorem_proving_practice::cli::run`], then translate the final `Result`
//! into a user-facing message plus a process exit code. All real work lives in
//! the library crate.

use std::process::ExitCode;

use theorem_proving_practice::cli;

fn main() -> ExitCode {
    // Logging is initialized inside `cli::run`, once the `-v/--verbose` flag has
    // been parsed (see `cli::run`).
    match cli::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            report_error(&err);
            ExitCode::from(err.exit_code())
        }
    }
}

/// Print the error and its full `source()` chain to stderr, so no context from
/// the underlying layer error is lost.
fn report_error(err: &dyn std::error::Error) {
    eprintln!("error: {err}");
    let mut source = err.source();
    while let Some(cause) = source {
        eprintln!("  caused by: {cause}");
        source = cause.source();
    }
}

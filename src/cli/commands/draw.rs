//! `draw` command: run the daily routine and report what was produced.

use crate::app::{App, DrawOptions};
use crate::cli::CliError;
use crate::cli::args::DrawArgs;

pub fn run(app: &mut App, args: DrawArgs) -> Result<(), CliError> {
    let dry_run = args.dry_run;
    let format = args.format;
    let outcome = app.draw(DrawOptions {
        count: args.count,
        seed: args.seed,
        out_dir: args.out_dir,
        overwrite: !args.no_clobber,
        dry_run,
        uniform: args.uniform,
        format,
    })?;

    println!("Drew {} theorem(s):", outcome.theorems.len());
    for theorem in &outcome.theorems {
        println!(
            "  - [{}] {}",
            theorem.subject.as_str(),
            theorem.name.as_str()
        );
    }
    println!(
        "\nWrote {}: {}",
        format.extension().to_uppercase(),
        outcome.output_path.display()
    );
    if dry_run {
        println!("(dry run: draw not recorded)");
    }
    Ok(())
}

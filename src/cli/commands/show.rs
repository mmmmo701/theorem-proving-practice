//! `show` command: print one theorem in full, resolved by id or unique prefix.

use crate::app::App;
use crate::cli::CliError;
use crate::cli::args::ShowArgs;

pub fn run(app: &App, args: ShowArgs) -> Result<(), CliError> {
    let theorem = super::resolve_unique(app, &args.id)?;

    println!("Theorem {}", theorem.id);
    println!("  Subject:    {}", theorem.subject.as_str());
    println!("  Name:       {}", theorem.name.as_str());
    println!(
        "  Date Added: {}",
        theorem.added_at.format("%Y-%m-%d %H:%M UTC")
    );
    println!("  Drawn:      {} time(s)", theorem.draw_count);
    println!(
        "  Last Drawn: {}",
        match theorem.last_drawn_at {
            Some(at) => at.format("%Y-%m-%d %H:%M UTC").to_string(),
            None => "never".to_string(),
        }
    );
    println!("  Content:");
    for line in theorem.content.as_str().lines() {
        println!("    {line}");
    }
    Ok(())
}

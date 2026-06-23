//! `list` command: print a summary table of stored theorems.

use crate::app::App;
use crate::cli::CliError;
use crate::domain::Theorem;

/// Number of leading id characters shown in the table (enough to be a unique
/// prefix for `show` in practice).
const SHORT_ID_LEN: usize = 8;
/// Upper bounds on column widths, so long labels don't blow out the table.
const MAX_SUBJECT_COL: usize = 24;
const MAX_NAME_COL: usize = 40;

pub fn run(app: &App) -> Result<(), CliError> {
    let theorems = app.list()?;
    if theorems.is_empty() {
        println!("No theorems stored yet. Add one with `add`.");
        return Ok(());
    }

    let subject_w = column_width(theorems.iter().map(|t| t.subject.as_str()), MAX_SUBJECT_COL, "SUBJECT");
    let name_w = column_width(theorems.iter().map(|t| t.name.as_str()), MAX_NAME_COL, "NAME");

    println!(
        "{:<id$}  {:<sw$}  {:<nw$}  {:<10}  {:>5}  {}",
        "ID",
        "SUBJECT",
        "NAME",
        "ADDED",
        "DRAWS",
        "LAST DRAWN",
        id = SHORT_ID_LEN,
        sw = subject_w,
        nw = name_w,
    );
    for t in &theorems {
        println!(
            "{:<id$}  {:<sw$}  {:<nw$}  {:<10}  {:>5}  {}",
            short_id(t),
            truncate(t.subject.as_str(), subject_w),
            truncate(t.name.as_str(), name_w),
            t.added_at.format("%Y-%m-%d"),
            t.draw_count,
            last_drawn(t),
            id = SHORT_ID_LEN,
            sw = subject_w,
            nw = name_w,
        );
    }
    println!("\n{} theorem(s).", theorems.len());
    Ok(())
}

/// Width for a column: the longest value (capped at `max`), but at least as wide
/// as the header.
fn column_width<'a>(values: impl Iterator<Item = &'a str>, max: usize, header: &str) -> usize {
    let longest = values
        .map(|s| s.chars().count().min(max))
        .max()
        .unwrap_or(0);
    longest.max(header.len())
}

/// The first [`SHORT_ID_LEN`] characters of a theorem's id.
fn short_id(t: &Theorem) -> String {
    t.id.to_string().chars().take(SHORT_ID_LEN).collect()
}

/// Truncate to `max` characters, marking truncation with an ellipsis.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// The last-drawn date as `YYYY-MM-DD` (matching the `ADDED` column), or `—`
/// for a theorem that has never been drawn.
fn last_drawn(t: &Theorem) -> String {
    match t.last_drawn_at {
        Some(date) => date.format("%Y-%m-%d").to_string(),
        None => "—".to_string(),
    }
}
//! clap argument and subcommand definitions.

use std::path::PathBuf;

use clap::{ArgAction, ArgGroup, Args, Parser, Subcommand};

use crate::render::OutputFormat;

/// Build a library of theorems and draw a daily set to practice proving.
#[derive(Debug, Parser)]
#[command(name = "theorem-proving-practice", version, about, long_about = None)]
pub struct Cli {
    /// Increase logging verbosity: `-v` for info, `-vv` for debug. The
    /// `RUST_LOG` environment variable, if set, takes precedence.
    #[arg(short, long, global = true, action = ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Add a theorem to the library.
    Add(AddArgs),

    /// Draw theorems and compile them into today's practice PDF.
    Draw(DrawArgs),

    /// List stored theorems as a summary table.
    List,

    /// Show one theorem in full, by id or a unique id prefix.
    Show(ShowArgs),

    /// Edit a theorem's subject, name, or content, by id or a unique id prefix.
    Edit(EditArgs),

    /// Delete a theorem, by id/prefix or by picking from a menu.
    Delete(DeleteArgs),

    /// Open a generated output (PDF/HTML) with the system viewer.
    Open(OpenArgs),
}

/// Arguments for the `open` command.
///
/// With no argument it lists the files in the output directory and prompts you
/// to pick one. Give a file name (or a unique substring of it) to open that file
/// directly without the menu.
#[derive(Debug, Args)]
pub struct OpenArgs {
    /// Output file name, or a unique substring of it, to open without prompting.
    pub name: Option<String>,
}

/// Arguments for the `draw` command.
#[derive(Debug, Args)]
pub struct DrawArgs {
    /// Number of theorems to draw (default: the configured count, 3).
    #[arg(long, short = 'n', value_name = "N")]
    pub count: Option<usize>,

    /// Seed for a reproducible draw (same seed + library → same set).
    #[arg(long)]
    pub seed: Option<u64>,

    /// Directory to write the PDF into (default: the configured output dir).
    #[arg(long = "out-dir", value_name = "DIR")]
    pub out_dir: Option<PathBuf>,

    /// Refuse to overwrite today's PDF if it already exists.
    #[arg(long = "no-clobber")]
    pub no_clobber: bool,

    /// Draw and render the PDF without recording it (draw stats are left
    /// untouched, so the forgetting-curve weighting doesn't change).
    #[arg(long = "dry-run")]
    pub dry_run: bool,

    /// Use plain uniform random selection, ignoring the forgetting curve.
    #[arg(long)]
    pub uniform: bool,

    /// Output format: `pdf` (default; needs a LaTeX engine) or `html`
    /// (self-contained, math typeset in-browser by MathJax).
    #[arg(long, value_enum, default_value_t = OutputFormat::Pdf)]
    pub format: OutputFormat,
}

/// Arguments for the `show` command.
#[derive(Debug, Args)]
pub struct ShowArgs {
    /// Theorem id, or a unique prefix of it (as shown in `list`).
    pub id: String,
}

/// Arguments for the `delete` command.
///
/// Give an `id` (or unique prefix) to delete that theorem, or pass
/// `--interactive` to choose one from a numbered menu. Deletion asks for
/// confirmation unless `--yes` is given.
#[derive(Debug, Args)]
pub struct DeleteArgs {
    /// Theorem id, or a unique prefix of it (as shown in `list`). Omit it when
    /// using `--interactive`.
    pub id: Option<String>,

    /// Pick the theorem to delete from a numbered menu instead of giving an id.
    #[arg(long, short)]
    pub interactive: bool,

    /// Delete without the confirmation prompt (useful in scripts).
    #[arg(long, short = 'y')]
    pub yes: bool,
}

/// Arguments for the `edit` command.
///
/// In the default (flag-driven) mode, the given fields replace the theorem's
/// current values and omitted fields are kept; giving no field at all is a user
/// error. With `--interactive`, every field is prompted for, pre-filled with
/// its current value (the LaTeX content opens in `$EDITOR`); flags you also
/// pass override those pre-fills. The arg group keeps the two content sources
/// mutually exclusive in either mode.
#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("content_src").args(["content", "content_file"])
))]
pub struct EditArgs {
    /// Theorem id, or a unique prefix of it (as shown in `list`).
    pub id: String,

    /// Prompt for each field pre-filled with its current value, opening
    /// `$EDITOR` for the LaTeX content. Sidesteps shell-escaping pitfalls
    /// (`$`, `\`, …).
    #[arg(long, short)]
    pub interactive: bool,

    /// New subject for the theorem.
    #[arg(long)]
    pub subject: Option<String>,

    /// New name for the theorem.
    #[arg(long)]
    pub name: Option<String>,

    /// New LaTeX content, given inline.
    #[arg(long)]
    pub content: Option<String>,

    /// Path to a file whose contents become the theorem's new LaTeX.
    #[arg(long = "content-file", value_name = "PATH")]
    pub content_file: Option<PathBuf>,
}

/// Arguments for the `add` command.
///
/// In the default (flag-driven) mode, `--subject`, `--name`, and exactly one of
/// `--content` / `--content-file` are required; clap enforces the labels, and
/// the handler enforces the content source. With `--interactive`, every field
/// is prompted for instead, so all flags become optional pre-fills — `--content`
/// or `--content-file` seeds the editor buffer. The arg group keeps the two
/// content sources mutually exclusive in either mode.
#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("content_src").args(["content", "content_file"])
))]
pub struct AddArgs {
    /// Prompt for each field instead of taking them as flags, opening `$EDITOR`
    /// for the LaTeX content. Sidesteps shell-escaping pitfalls (`$`, `\`, …);
    /// any flags you also pass pre-fill their fields.
    #[arg(long, short)]
    pub interactive: bool,

    /// Subject the theorem belongs to (e.g. "Real Analysis").
    #[arg(long, required_unless_present = "interactive")]
    pub subject: Option<String>,

    /// Name of the theorem (e.g. "Monotone Convergence Theorem").
    #[arg(long, required_unless_present = "interactive")]
    pub name: Option<String>,

    /// LaTeX content of the theorem, given inline.
    #[arg(long)]
    pub content: Option<String>,

    /// Path to a file whose contents are the theorem's LaTeX (handy for long
    /// content, where shell-escaping inline is painful).
    #[arg(long = "content-file", value_name = "PATH")]
    pub content_file: Option<PathBuf>,
}

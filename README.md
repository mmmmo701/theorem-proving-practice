# theorem-proving-practice

A small command-line tool for practicing proofs. Build a personal library of
theorems written in LaTeX, then each day draw a few and compile them into a
nicely formatted PDF to prove by hand. The draw is weighted by a **forgetting
curve**: theorems you have practiced a lot, and recently, are less likely to come
up than neglected or never-seen ones.

Each theorem has a **Subject**, a **Name**, the **Content** (LaTeX), a
**Date Added** (stamped automatically), and practice stats (**how many times**
it has been drawn and **when last**).

## Requirements

- A recent Rust toolchain (edition 2024).
- A LaTeX engine on your `PATH`. The default is [`latexmk`]; `pdflatex`,
  `xelatex`, and `lualatex` are also supported.

[`latexmk`]: https://ctan.org/pkg/latexmk

## Build

```sh
cargo build --release
# binary at target/release/theorem-proving-practice
```

The examples below use `cargo run --` for convenience.

## Usage

### Add a theorem

Inline content:

```sh
cargo run -- add \
  --subject "Real Analysis" \
  --name "Monotone Convergence Theorem" \
  --content 'If $f_n \uparrow f$ with $f_n \ge 0$ measurable, then $\int f_n \to \int f$.'
```

Or read the content from a file (handy for long LaTeX, avoiding shell-escaping):

```sh
cargo run -- add --subject "Topology" --name "Urysohn's Lemma" --content-file lemma.tex
```

**Interactive mode** (recommended for anything with `$`, `\`, or quotes — the
shell never sees the content, so there is nothing to escape):

```sh
cargo run -- add -i
```

You are prompted for the subject and name, then your editor (`$VISUAL`,
`$EDITOR`, else `vi`) opens for the LaTeX content; save and quit to store it.
Any flag you also pass pre-fills its field — e.g. `add -i --subject "Topology"`
skips the subject prompt, and `--content`/`--content-file` seed the editor
buffer.

The theorem content may use `\begin{theorem}`, `\begin{lemma}`,
`\begin{corollary}`, `\begin{proposition}`, and `\begin{definition}` — those
environments are predefined in the generated document.

### List and inspect

```sh
cargo run -- list            # summary table of all theorems
cargo run -- show dbbe       # one theorem in full, by id or unique id prefix
```

`list` includes `DRAWS` (times practiced) and `LAST DRAWN` columns; `show` adds
`Drawn:` and `Last Drawn:` lines. A never-drawn theorem shows `—` / `never`.

### Delete a theorem

By id or a unique id prefix (asks to confirm first):

```sh
cargo run -- delete dbbe         # confirm with y/N
cargo run -- delete dbbe --yes   # skip the prompt (for scripts)
```

Or pick one from a numbered menu:

```sh
cargo run -- delete -i           # choose by number or id; blank cancels
```

### Draw the daily set

```sh
cargo run -- draw                 # draw 3 (default) and write today's PDF
cargo run -- draw -n 5            # draw 5
cargo run -- draw --seed 42       # reproducible draw (within a fixed library state)
cargo run -- draw --no-clobber    # refuse to overwrite today's PDF
cargo run -- draw --out-dir /tmp  # write to a specific dir instead of the default output dir
cargo run -- draw --dry-run       # write the PDF but don't record the draw
cargo run -- draw --uniform       # ignore the forgetting curve; pick uniformly
cargo run -- draw --format html   # write a self-contained HTML sheet instead
```

The sheet is written as `practice-YYYY-MM-DD.<ext>` (`.pdf` by default, or
`.html` with `--format html`) in the output directory — see
[Where things live](#where-things-live) for its location, or override per-draw
with `--out-dir`. The PDF format needs a LaTeX engine on `PATH`;
the HTML format needs none — it typesets math in the browser via MathJax loaded
from a CDN, so **viewing** an HTML sheet requires an internet connection. (Math
written as `\begin{theorem}…` environments isn't rendered as boxes in HTML;
inline/display math `$…$`, `\[…\]` works.)

By default the draw is **weighted by the forgetting curve** and **records
itself**: each chosen theorem's draw count and last-drawn time are updated so it
becomes less likely next time and resurfaces as time passes. Because recording
changes the library, re-running the same `--seed` later yields a different set;
use `--dry-run` to preview a draw without recording it, or `--uniform` for a
plain equal-probability draw.

### Open a generated sheet

Open one of the generated sheets with your system viewer (the file handler your
desktop uses — e.g. your PDF reader or browser):

```sh
cargo run -- open                       # list the output files and pick one
cargo run -- open practice-2026-06-23   # open by name, or a unique substring
```

With no argument, `open` lists the files in the output directory newest-first
and prompts you to pick one (by number or name; blank cancels). Giving a name —
or any unique substring of one — opens it directly.

The opener is `xdg-open` on Linux and `open` on macOS. Override it with
`THEOREM_PROVING_PRACTICE_OPENER` (it may include arguments, like `$EDITOR`):

```sh
THEOREM_PROVING_PRACTICE_OPENER=evince cargo run -- open
```

## Where things live

By default the tool keeps everything under a fixed, per-user location so it
behaves the same no matter which directory you run it from (important once it is
installed system-wide, e.g. from a `.deb`):

- **Data** (`theorems.json`, the library): `$XDG_DATA_HOME/theorem-proving-practice/data`,
  falling back to `~/.local/share/theorem-proving-practice/data`.
- **Output** (`practice-YYYY-MM-DD.pdf` / `.html` sheets):
  `…/theorem-proving-practice/output`.

Both directories are created on first use. You can override either location:

| Variable | Overrides |
|----------|-----------|
| `THEOREM_PROVING_PRACTICE_DATA_DIR`   | where the theorem library is stored |
| `THEOREM_PROVING_PRACTICE_OUTPUT_DIR` | where practice sheets are written, and what `open` lists |
| `THEOREM_PROVING_PRACTICE_OPENER`     | the command `open` uses to launch files |

A single `draw` can also write elsewhere with `--out-dir` (see above). If neither
`XDG_DATA_HOME` nor `HOME` is set, the tool falls back to `data/` and `output/`
relative to the current directory.

## Diagnostics

Increase verbosity with `-v` (info) or `-vv` (debug):

```sh
cargo run -- -vv draw
```

Setting `RUST_LOG` (e.g. `RUST_LOG=debug`) overrides the `-v` level.

## Exit codes

| Code | Meaning |
|------|---------|
| `0`  | success |
| `1`  | user/input error (bad field, too few theorems, file exists, not found) |
| `2`  | configuration or storage error |
| `3`  | rendering error (LaTeX engine missing, compilation failed) |

When a LaTeX compilation fails, the relevant tail of the engine log is printed
and the build directory is preserved for inspection.

## Design

The architecture is a pure domain core with `storage`, `selection`, and
`render` as trait-backed extension points, wired together by an `app` layer
beneath a thin `cli`. See [`CLAUDE.md`](CLAUDE.md) for the per-layer breakdown
and the conventions to follow when extending it.

## Tests

```sh
cargo test                 # unit tests (no LaTeX required)
cargo test -- --ignored    # also run tests that invoke a real LaTeX engine
```

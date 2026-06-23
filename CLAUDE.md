# CLAUDE.md

Guidance for working in this repo. For *what the project is* and *how a user
runs it*, see `README.md`. This file captures the non-obvious things.

## What this is

A Rust CLI to maintain a LaTeX theorem library and draw a daily set into a
formatted PDF. 

## Build / test / run

```sh
cargo build
cargo test                 # 75 unit tests; no LaTeX engine needed
cargo test -- --ignored    # 2 extra tests that invoke real `latexmk`
cargo run -- <add|list|show|draw|delete> ...
```

The two `#[ignore]`d tests in `src/render/latex.rs` actually shell out to
`latexmk`, so they only run under `--ignored`. Keep them ignored — CI/dev
machines may lack a TeX install.

## Architecture (where to make changes)

Dependencies point inward to a pure `domain` core (no I/O). The three
trait-backed layers are the extension points — adding a variant is a new file
plus one line of wiring in `app`:

- `domain` — `Theorem` + validated newtypes (`Subject`, `Name`, `LatexContent`,
  `TheoremId`). No I/O. Construction validates; so does deserialization.
- `storage` — `Repository` trait; `JsonStore` impl (single JSON file).
- `selection` — `Selector` trait; impls `ForgettingCurveSelector` (the default)
  and `RandomSelector` (uniform; `draw --uniform`).
- `render` — `Renderer` trait; impls `LatexRenderer`
  (`render/latex/{escape,template,engine}.rs`, PDF via an external engine) and
  `HtmlRenderer` (`render/html/{escape,template}.rs`, self-contained HTML, math
  via MathJax CDN, no engine). `OutputFormat` (in `render.rs`) maps a format to
  its file extension and is what `draw --format` selects.
- `app` — `App` holds the trait objects (note: *two* renderers — `renderer` for
  PDF, `html_renderer` for HTML); one use-case per file
  (`add`/`draw`/`list`/`delete`). `AppError` wraps every layer error via `#[from]`.
- `cli` — clap args + one handler per command; `CliError` wraps `AppError`.
  Terminal-interaction helpers (line prompts, `$EDITOR`) live in `cli/input.rs`;
  id/prefix → single-theorem resolution is shared via `commands::resolve_unique`.

It's a library crate (`lib.rs`) with a thin binary (`main.rs`), so every layer
is testable without spawning a process. A new front-end would be another binary
over the same library.

## Conventions / gotchas

- **Module files:** non-leaf modules are `foo.rs` next to a `foo/` dir, NOT
  `foo/mod.rs`. Keep this style for new modules.
- **No warnings policy:** the build is warning-clean; keep it that way.
- **Trait-object method calls don't need the trait imported.** Calling
  `self.repo.add(..)` / `self.selector.draw(..)` on a `Box<dyn Trait>` field
  compiles without `use`ing the trait — importing it yields an unused-import
  warning. Only `impl Trait for FakeX` (e.g. test fakes) needs the trait in scope.
- **Errors are typed per layer** (`thiserror`), composed into `AppError`, then
  `CliError`. `anyhow` is allowed only at the binary boundary. Exit codes:
  1 = user/input, 2 = config/storage, 3 = render. Keep them stable.
- **LaTeX engine args differ by engine:** `latexmk` needs `-pdf`; bare
  `pdflatex`/`xelatex`/`lualatex` reject it. See `engine_args` in
  `render/latex/engine.rs` if adding engines.
- **Theorem content is raw LaTeX, never escaped** in storage and in the LaTeX
  renderer; only metadata (subject/name/date) is escaped
  (`render/latex/escape.rs`). Don't escape content. **One deliberate exception:**
  the *HTML* renderer (`render/html/escape.rs`) encodes the three structural
  chars `&`/`<`/`>` in *both* metadata and content — content too, because
  `$0 < x$` would otherwise break the markup. MathJax decodes those entities
  before typesetting, so the math still renders as authored. `\` and `$` are
  never touched. Stored content is unaffected — this is render-time only.
- **Interactive input (`add -i`, `delete -i`) writes prompts to stderr** so
  stdout stays clean; content is entered via `$VISUAL`/`$EDITOR`/`vi` (so the
  shell never mangles `$`/`\`). EOF on a prompt aborts rather than looping —
  preserve that when adding prompts.
- **`draw` mutates state.** By default it *records* the draw (bumps each chosen
  theorem's `draw_count` / `last_drawn_at` via `Repository::update`) so the
  forgetting-curve weighting stays current — so it takes `&mut App`, and the
  same `--seed` evolves between runs. `--dry-run` renders without recording;
  `--uniform` swaps in `RandomSelector`; `--format html|pdf` picks the renderer
  *and* the output file extension (the use-case maps format→extension, so the
  `Renderer` trait stays unaware of it). Recording happens only *after* a
  successful render. Selectors stay pure: "now" is passed in via
  `DrawRequest.now`, not read from a clock. Curve constants (`S0`, growth,
  floor) live in `selection/forgetting.rs`.
- **Runtime dirs `data/` and `output/` are git-ignored** and created on first
  run. `data/theorems.json` uses a versioned envelope (`{"version":1,...}`) —
  bump `CURRENT_VERSION` in `storage/json_store.rs` and handle migration if the
  on-disk shape changes.
- **Logging:** `log` facade + `env_logger`, initialized in `cli::run`. Use
  `-v`/`-vv`, or `RUST_LOG` (which overrides `-v`).

## Not yet built (planned seams exist)

 - edit theorems, 
 - tags/search, 
 - more renderers (Anki; HTML is built), 
 - SQLite storage

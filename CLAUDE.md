# CLAUDE.md

Guidance for working in this repo. For *what the project is* and *how a user
runs it*, see `README.md`. This file captures the non-obvious things.

## What this is

A Rust CLI to maintain a LaTeX theorem library and draw a daily set into a
formatted PDF. 

## Build / test / run

```sh
cargo build
cargo test                 # 139 unit tests; no LaTeX engine needed
cargo test -- --ignored    # 2 extra tests that invoke real `latexmk`
cargo run -- <add|list|show|edit|draw|delete|open> ...
```

The two `#[ignore]`d tests in `src/render/latex.rs` actually shell out to
`latexmk`, so they only run under `--ignored`. Keep them ignored — CI/dev
machines may lack a TeX install.

## Architecture (where to make changes)

Dependencies point inward to a pure `domain` core (no I/O). The three
trait-backed layers are the extension points — adding a variant is a new file
plus one line of wiring in `app`:

- `domain` — `Theorem` + validated newtypes (`Subject`, `Name`, `LatexContent`,
  `TheoremId`, `VaultName`). No I/O. Construction validates; so does
  deserialization.
- `storage` — `Repository` trait; `JsonStore` impl (single JSON file per
  vault).
- `vaults` — `VaultStore` trait; `FsVaultStore` impl. A vault is a directory
  under `vaults/<name>/`; the directory *listing* is the source of truth for
  which vaults exist (no separate registry file to drift out of sync with the
  disk) — only the *current* vault is persisted, in `state.json`.
  `migrate_legacy_layout` moves a pre-vault flat store into the `default`
  vault, checked on every bootstrap. `JsonStore::save` and
  `FsVaultStore::set_current` both write through the shared top-level
  `fs_atomic::write` helper (temp file → fsync → rename).
- `selection` — `Selector` trait; impls `ForgettingCurveSelector` (the default)
  and `RandomSelector` (uniform; `draw --uniform`).
- `render` — `Renderer` trait; impls `LatexRenderer`
  (`render/latex/{escape,template,engine}.rs`, PDF via an external engine) and
  `HtmlRenderer` (`render/html/{escape,template}.rs`, self-contained HTML, math
  via MathJax CDN, no engine). `OutputFormat` (in `render.rs`) maps a format to
  its file extension and is what `draw --format` selects.
- `app` — `App` holds the trait objects (note: *two* renderers — `renderer` for
  PDF, `html_renderer` for HTML) plus which vault it's bound to
  (`App::vault_name`); one use-case per file
  (`add`/`edit`/`draw`/`list`/`delete`/`open`). Bootstrap is two phases:
  `app::VaultEnv::bootstrap()` (config + migration + the vault store — must
  succeed even when the current vault is broken) then
  `App::bootstrap_in(&vaults, vault_override)` (resolves *which* vault and
  binds `Repository`/`output_dir` to it; see the vaults gotcha below).
  `open.rs` only *lists* the output dir (`App::list_outputs`); the actual
  viewer launch is a CLI/OS side-effect, so it lives in `cli/input.rs`, not
  here. `AppError` wraps every layer error via `#[from]`, including
  `VaultError`.
- `cli` — clap args + one handler per command; `CliError` wraps `AppError`. A
  global `--vault NAME` flag (or `THEOREM_PROVING_PRACTICE_VAULT`) overrides
  the current vault for one invocation; the `vault` subcommand
  (`list`/`current`/`add`/`switch`, flag- and `-i`-driven) manages vaults and
  is dispatched *before* `App::bootstrap_in` runs. Terminal-interaction
  helpers (line prompts, `$EDITOR`, the numbered-menu picker
  `pick_from_menu` shared by `delete -i`/`open`/`vault switch -i`, and the
  `open` viewer launch `open_in_viewer`) live in `cli/input.rs`; id/prefix →
  single-theorem resolution is shared via `commands::resolve_unique`, and the
  validated label/editor prompt loops (`prompt_label`/`prompt_content`, used
  by `add -i`, `edit -i`, and `vault add -i`) also live in `commands.rs`.

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
  1 = user/input, 2 = config/storage, 3 = render. Keep them stable. `VaultError`
  is the one layer error with its *own* `exit_code()` (delegated to by
  `AppError::exit_code`), because a single error type spans both categories —
  `NotFound`/`AlreadyExists` are user mistakes (1), everything else is broken
  on-disk state (2).
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
- **Interactive input (`add -i`, `delete -i`, `open`'s menu, `vault add -i`,
  `vault switch -i`) writes prompts to stderr** so stdout stays clean; content
  is entered via `$VISUAL`/`$EDITOR`/`vi` (so the shell never mangles `$`/`\`).
  EOF on a prompt aborts rather than looping — preserve that when adding
  prompts.
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
- **`edit` is partial-update by design.** `EditRequest` (app layer) carries
  `Option<String>` per field; `None` keeps the current value, so identity,
  `added_at`, and draw stats always survive an edit (`App::edit` mutates the
  fetched `Theorem` and persists via `Repository::update`). It acts on an exact
  `TheoremId` and returns `Ok(None)` for an unknown id (mirrors `delete`'s
  `false`); prefix resolution stays in the CLI (`resolve_unique`). Flag mode
  with *no* fields is `CliError::NoEditsRequested`, not a silent no-op.
  `edit -i` pre-fills prompts with current values and seeds `$EDITOR` with the
  current content; flags passed alongside `-i` override those pre-fills.
- **Runtime dirs are per-user and fixed, not cwd-relative.** `Config::load`
  resolves `vaults_root`/`output_dir` to `$XDG_DATA_HOME/theorem-proving-practice`
  and `.../output`, falling back to `~/.local/share/...`, so an installed
  binary (`.deb`) behaves identically from any directory.
  `Config::vaults_dir()`/`state_path()`/`vault_store_path(&VaultName)`/
  `vault_output_dir(&VaultName)` derive the real per-vault paths from those
  roots. `THEOREM_PROVING_PRACTICE_DATA_DIR`, if set, now names `vaults_root`
  directly (no implicit `data/` leaf); `THEOREM_PROVING_PRACTICE_OUTPUT_DIR`
  names the root under which each vault gets an output subdirectory.
  `Config::data_dir` keeps its *old*, unchanged resolution (`.../data`, or the
  raw env override) — it no longer names live storage, only the legacy
  flat-store location `vaults::migrate_legacy_layout` checks for. **Important:**
  `Config::default` still returns cwd-relative paths — it exists for tests
  (which build `Config { data_dir: tmp, .. }` directly, most bypassing vaults
  entirely via `App::new`); don't route the binary through it. Only if neither
  `XDG_DATA_HOME` nor `HOME` is set do real runs fall back to those
  cwd-relative paths too. Each vault's `theorems.json` uses a versioned
  envelope (`{"version":1,...}`) — bump `CURRENT_VERSION` in
  `storage/json_store.rs` and handle migration if the on-disk *theorem* shape
  changes; `state.json`'s own version (`STATE_VERSION` in `vaults/fs_store.rs`)
  is independent and only covers the current-vault pointer.
- **Vaults have no registry file — the `vaults/` directory listing *is* the
  source of truth** for which vaults exist (a registry can drift from disk; a
  directory scan cannot). Vault names (`domain::VaultName`) are
  lowercase-folded and restricted to `[a-z0-9_-]` with an alphanumeric first
  character *by construction* — that's what makes a vault name safe to use
  directly as a directory name (no path traversal, no dotfiles, no
  case-insensitive-filesystem collisions); don't loosen the charset without
  re-deriving that safety property. `vault add`/`vault switch`
  (`app::VaultEnv`, wired in `cli/commands/vault.rs`) run against
  `VaultEnv::bootstrap()` *before* `App::bootstrap_in` resolves a specific
  vault, so they keep working even when the persisted current vault is
  missing or broken — that's the recovery path, and why bootstrap is split
  into two phases. `--vault NAME`/`THEOREM_PROVING_PRACTICE_VAULT` never
  auto-create a vault (a typo must not silently start an empty one); the
  *implicit* `default` vault is the one exception, and only when reached via
  the persisted current-vault path, never via an explicit override — see
  `VaultEnv::resolve_vault`. A missing *persisted* current vault surfaces as
  `AppError::CurrentVaultMissing` (exit 2: broken environment); an unknown
  name from `--vault` or `vault switch` surfaces as `VaultError::NotFound`
  (exit 1: bad argument) — same underlying condition, different exit code
  depending on whether the user typed the name this run.
  `vaults::migrate_legacy_layout` moves a pre-vault flat store into `default`
  on every bootstrap (a same-filesystem rename: atomic, idempotent, a no-op
  once done) and refuses to guess — errors out — if both the legacy file and
  vault `default`'s store already exist, rather than silently picking one.
- **`open` shells out to a system viewer.** Default `xdg-open` (Linux) / `open`
  (macOS), overridable via `THEOREM_PROVING_PRACTICE_OPENER` (may carry args,
  split on whitespace like `$EDITOR`). The platform default is a `cfg`-gated
  const in `cli/input.rs`. `open` is read-only (`&App`); it lists output files
  newest-first and resolves a name by exact match then unique substring.
- **Logging:** `log` facade + `env_logger`, initialized in `cli::run`. Use
  `-v`/`-vv`, or `RUST_LOG` (which overrides `-v`).

## Not yet built (planned seams exist)

 - tags/search, 
 - more renderers (Anki; HTML is built), 
 - SQLite storage
 - `vault delete`/`vault rename` (destructive; needs its own confirmation
   design, and must refuse to delete/rename the current vault)
 - per-vault metadata (e.g. `vaults/<name>/vault.json` for created-at,
   description) or config overrides (draw count, default format) — would live
   *inside* the vault's own directory, not in a central list, to keep the
   no-registry invariant
 - cross-vault move/copy of a theorem

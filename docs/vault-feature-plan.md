# Vault feature — design and implementation plan

Status: **planned, not yet implemented.**

A *vault* is an independent theorem library: its own theorems, draw
statistics, and generated outputs. The program always operates *in* exactly one
vault — the one you last switched to — and every existing command
(`add`/`list`/`show`/`edit`/`draw`/`delete`/`open`) acts only on that vault.
Vaults never see each other's data.

This document is the implementation plan. It favors reliability (crash-safe
state, safe names, idempotent migration) and first-class support for both
flag-driven and `-i` interactive use, mirroring the existing commands.

---

## 1. Goals and non-goals

**Goals**

- Multiple named vaults; complete data isolation between them (store file,
  draw stats, output directory).
- A persisted *current vault*: opening the program later resumes where you
  were. First run works with zero setup (an implicit `default` vault).
- `vault add` and `vault switch`, each usable via arguments *and* via
  `--interactive`, consistent with the existing `-i` conventions (prompts on
  stderr, EOF aborts, validated re-prompt loops).
- A one-off override (`--vault <name>` global flag) so scripts can target a
  vault without changing the persisted current vault.
- Transparent, crash-safe migration of an existing pre-vault library into the
  `default` vault.

**Non-goals (planned seams, not in this change)**

- `vault delete` / `vault rename` (destructive; needs its own confirmation
  design — see §12).
- Per-vault configuration (draw count, engine) — the config stays global.
- Cross-vault operations (move/copy a theorem between vaults).

---

## 2. CLI surface

### 2.1 New subcommand group

```
theorem-proving-practice vault list
theorem-proving-practice vault current
theorem-proving-practice vault add <NAME>
theorem-proving-practice vault add -i [NAME]
theorem-proving-practice vault switch <NAME>
theorem-proving-practice vault switch -i
```

| Command | Behavior |
|---|---|
| `vault list` | Table of vaults (name, theorem count) to stdout; the current vault is marked with `*`. Never prompts. |
| `vault current` | Prints just the current vault's name to stdout (script-friendly, like `git branch --show-current`). |
| `vault add <NAME>` | Validates the name, creates the vault, prints a confirmation. Does **not** switch to it (predictable for scripts). |
| `vault add -i [NAME]` | Prompts for the name in a validated re-prompt loop (`prompt_label` pattern; an optional positional pre-fills the prompt, same as `add -i` flag pre-fills). Then asks `Switch to '<name>' now? [y/N]`. |
| `vault switch <NAME>` | Resolves `<NAME>` (exact match first, then unique prefix — same resolution style as `open`'s name matching) and persists it as current. |
| `vault switch -i` | Numbered menu of vaults on stderr (current one marked), pick by number; EOF or empty input aborts with `CliError::Aborted`. Mirrors `delete -i`'s menu. |

Notes:

- `vault add` of a name that already exists is an error (exit 1), not a silent
  no-op — comparison is case-insensitive (§4).
- `vault switch` to the vault you are already in succeeds and says so
  ("already in vault 'x'"); it is idempotent, not an error.
- All prompts and menus go to **stderr**; stdout carries only the command's
  real output. EOF on any prompt aborts rather than looping (existing rule).

### 2.2 Global `--vault` override

A new global arg on `Cli` (next to `--verbose`):

```
theorem-proving-practice --vault exams draw -n 5
```

- Ephemeral: runs this one invocation against vault `exams`; the persisted
  current vault is untouched.
- The named vault must already exist; otherwise error (exit 1) listing the
  available vaults. It is never auto-created — a typo must not silently create
  an empty vault and "lose" the library.
- Also honored via `THEOREM_PROVING_PRACTICE_VAULT` (env), for shells/scripts
  that pin a vault. Precedence, highest first:
  `--vault` flag → env var → persisted state → `default`.
- Combining `--vault` with a `vault` subcommand: allowed and ignored for
  `list`/`current`? No — keep it strict: `--vault` combined with the `vault`
  subcommand is a usage error (clap `conflicts_with`), because "switch while
  overriding the vault" has no sensible meaning.

### 2.3 Awareness in existing commands

- Each command's human-facing feedback mentions the vault when it mutates
  state, e.g. `Added "Monotone Convergence" to vault 'default'.` — cheap
  insurance against operating in the wrong vault.
- `list` prints a one-line header naming the vault (to stderr, keeping the
  stdout table clean for pipes).
- `open` lists/opens only the current vault's output directory.

---

## 3. On-disk layout and state

### 3.1 Layout

Pre-vault (today):

```
$DATA_ROOT/theorem-proving-practice/
├── data/theorems.json
└── output/practice-2026-07-09.pdf
```

Post-vault:

```
$DATA_ROOT/theorem-proving-practice/
├── state.json                        # current vault pointer (versioned)
├── vaults/
│   ├── default/theorems.json
│   └── exams/theorems.json
└── output/
    ├── default/practice-2026-07-09.pdf
    └── exams/practice-2026-07-09.pdf
```

**Single source of truth:** the set of vaults **is** the set of directories
under `vaults/`. There is deliberately *no* registry file listing vaults —
a registry can drift from the directories (the classic two-sources-of-truth
bug); a directory scan cannot. Vault metadata (created-at, description) is
future work and would live *inside* each vault (`vaults/<name>/vault.json`),
never in a central list.

- Creating a vault = create `vaults/<name>/` **and immediately write an empty
  store file** (`{"version":1,"theorems":[]}`) via the existing atomic-save
  path. The store file's presence makes the vault durable and visible even
  before the first theorem, and exercises writability at creation time (a
  permissions problem surfaces at `vault add`, not at the first `add`).
- Output directories are created lazily by `draw` (as today).
- `theorems.json`'s schema is **unchanged** → `CURRENT_VERSION` in
  `storage/json_store.rs` stays at 1. The layout change is directory-level,
  handled by migration (§6), not by the store's version envelope.

### 3.2 `state.json`

```json
{ "version": 1, "current_vault": "default" }
```

- Versioned envelope with its own version constant (independent of the store's),
  same forward-compat rule: newer version → typed "unsupported version" error
  (exit 2).
- Written with the exact same crash-safe recipe as `JsonStore::save`: temp file
  in the same directory → write → fsync → rename over the target. Extract that
  recipe into a shared helper (`storage::atomic_write_json` or similar) instead
  of duplicating it.
- Missing file = current vault is `default` (parallel to "missing store is an
  empty library"). Corrupt file = typed error, exit 2 — never silently reset,
  because that would silently switch the user's context.

### 3.3 Startup validation of the current vault

If the resolved current vault's directory does not exist:

- From **persisted state or env**: hard error (exit 2):
  `current vault 'x' no longer exists; run 'vault list' and 'vault switch'`.
  The `vault` subcommands themselves do **not** require the current vault to
  exist (see §7 bootstrap split), so the user can always recover with
  `vault switch` / `vault add`. No silent fallback to `default` — a fallback
  would quietly run the command against the wrong library.
- From **`--vault`**: exit 1 (user input error), message lists existing vaults.
- Exception: `default` is special — if it is the resolved vault and missing,
  it is created on the fly (that is exactly the first-run path, §6).

---

## 4. Vault names — validation rules

Vault names become directory names, so validation is a safety boundary, not
cosmetics. New domain newtype `VaultName` (module `domain/vault.rs`), following
the `Subject`/`Name` pattern: construction validates, and **deserialization
validates too** (`state.json` content is untrusted input).

Rules (allowlist, not blocklist):

- Non-empty after trimming; at most 64 chars.
- Allowed chars: ASCII `a–z`, `0–9`, `-`, `_`. (Uppercase input is accepted
  and folded to lowercase, so `Exams` and `exams` are the same vault — this
  also sidesteps case-insensitive-filesystem collisions on macOS.)
- Must start with an alphanumeric (no leading `-` or `_`; rules out anything
  flag-like or dotfile-like).
- Consequently impossible: path separators, `..`, `.`, spaces, control chars,
  Unicode lookalikes. No blocklist to get wrong.

Uniqueness check at `vault add` is on the normalized (lowercased) name.
Error messages state the rule that failed, and the interactive loop re-prompts
with the rejected input as the editable default (existing `prompt_label`
behavior).

---

## 5. Resolution flow (per run)

```
1. Config::load()            → roots: data_root, vaults_dir, output_root, state_path
2. read state.json           → persisted current vault (or "default")
3. apply overrides           → --vault > THEOREM_PROVING_PRACTICE_VAULT > persisted
4. run migration check (§6)  → idempotent, cheap when already migrated
5. validate vault exists     → per §3.3
6. derive per-vault paths    → store: vaults/<v>/theorems.json
                               output: output/<v>/
7. build App bound to those paths (JsonStore + output dir as today)
```

Everything below `App` (`storage`, `selection`, `render`, `domain::Theorem`)
is untouched: `JsonStore` already takes an arbitrary path, and the renderers
take an arbitrary destination. Vaults are resolved *once at bootstrap*, and the
rest of the program remains vault-oblivious. That is the key design move.

### Config changes (`config.rs`)

- `Config` gains `state_path()` and `vaults_dir()` (derived from `data_dir`'s
  parent root — see below), plus `vault_store_path(&VaultName)` and
  `vault_output_dir(&VaultName)`.
- Env var semantics, kept backward-compatible in spirit:
  `THEOREM_PROVING_PRACTICE_DATA_DIR` now names the root under which
  `vaults/` and `state.json` live; `..._OUTPUT_DIR` names the root under which
  per-vault output dirs live. Document this in README (behavior change for
  anyone using the overrides: one extra directory level).
- `Config::default` stays cwd-relative and test-only, as today.

---

## 6. Migration from the pre-vault layout

Runs inside bootstrap (step 4 above), every run, cheap when nothing to do.

**Trigger:** `data/theorems.json` exists **and** `vaults/default/theorems.json`
does not.

**Steps (crash-safe at every point):**

1. `create_dir_all(vaults/default/)`.
2. `rename(data/theorems.json → vaults/default/theorems.json)` — atomic on the
   same filesystem, which is guaranteed because both live under the same data
   root. No copy-then-delete (a crash between copy and delete would leave two
   diverging libraries).
3. Log at `info`: `migrated existing library into vault 'default'`.

**Idempotence / conflict handling:**

- Already migrated (source absent) → no-op.
- Both files exist (e.g. user restored a backup after migrating): **refuse to
  guess.** Typed error (exit 2) telling the user both exist and that they must
  remove or rename `data/theorems.json`. Never overwrite either side.
- Crash after step 1, before step 2 → next run re-enters cleanly (empty dir is
  harmless; rename still pending).

**Outputs are not migrated.** Files already in `output/` are regenerable
artifacts, not data; they stay where they are and remain openable from the
file manager. New draws in the default vault write to `output/default/`.
(`open` therefore only sees post-migration outputs — acceptable, documented.)

---

## 7. Architecture changes per layer

Follows the existing pattern: a new trait-backed seam, one file per use-case,
errors typed per layer and composed into `AppError` → `CliError`.

### `domain`
- `domain/vault.rs`: `VaultName` newtype (validation per §4, serde with
  validating deserialization). New `DomainError` variants for the name rules.
  No I/O, as required.

### `vaults` (new top-level module, sibling of `storage`)
- `vaults.rs`: `VaultStore` trait —
  `list() -> Vec<VaultName>`, `exists(&VaultName) -> bool`,
  `create(&VaultName)`, `current() -> VaultName`, `set_current(&VaultName)`.
- `vaults/fs_store.rs`: `FsVaultStore` impl over `vaults_dir` + `state.json`
  (directory scan for `list`; atomic JSON write for `set_current`; skips
  non-directory entries and warns on entries whose names fail `VaultName`
  validation rather than erroring — a stray file in `vaults/` must not brick
  the tool).
- `vaults/error.rs`: `VaultError` (thiserror): `NotFound { name, available }`,
  `AlreadyExists { name }`, `StateCorrupt`, `StateUnsupportedVersion`, I/O
  wrappers. Mapped into `AppError` via `#[from]`; exit codes:
  `NotFound`/`AlreadyExists` → 1, state/I-O problems → 2 (§8).
- Migration (§6) lives here too (it is vault-layout logic, not store logic).

### `app`
Bootstrap splits into two phases so vault management never depends on the
current vault being healthy:

```rust
// phase 1: cheap, cannot fail on a missing current vault
let vaults = VaultEnv::bootstrap()?;          // Config::load + FsVaultStore + migration

// phase 2: only for theorem commands
let mut app = App::bootstrap_in(&vaults, cli.vault.as_deref())?; // resolve → JsonStore + output dir
```

- `VaultEnv` (new, `app/vaults.rs`): wraps `Config` + `Box<dyn VaultStore>`,
  exposes use-cases `list_vaults`, `add_vault`, `switch_vault`,
  `current_vault` (one small impl block, or one file if it grows).
- `App` itself is unchanged in shape — it still holds `Config` + the four trait
  objects; only construction changes (`bootstrap_in` replaces `bootstrap`'s
  hardcoded paths with the resolved vault's paths, and `config.output_dir`
  is set to the vault's output dir so `draw`/`open` need no edits).
- `App::new` and the `with_*` test hooks are untouched → the existing 96 tests
  keep compiling; they already build `Config { data_dir: tmp, .. }` directly.

### `cli`
- `args.rs`: global `--vault <NAME>` on `Cli` (`conflicts_with` the `vault`
  subcommand); `Command::Vault(VaultArgs)` with a nested `VaultCommand`
  subcommand enum (`List`, `Current`, `Add(VaultAddArgs)`,
  `Switch(VaultSwitchArgs)`); `-i` flags per §2.
- `commands/vault.rs`: one handler per vault subcommand. Reuses `prompt_label`
  for the interactive name loop and adds a small shared `pick_from_menu`
  helper in `input.rs` (numbered stderr menu + selection parse) — `delete -i`
  already has this logic inline; extract and share rather than duplicate.
- `cli.rs` dispatch becomes:
  ```rust
  let vaults = VaultEnv::bootstrap()?;
  match cli.command {
      Command::Vault(args) => commands::vault::run(&mut vaults, args),
      other => {
          let mut app = App::bootstrap_in(&vaults, cli.vault.as_deref())?;
          /* existing per-command dispatch, unchanged */
      }
  }
  ```
- New `CliError` variants as needed (e.g. `VaultNotFound` is app-level; CLI
  adds only interaction errors, which already exist: `Aborted`).

---

## 8. Errors and exit codes (kept stable)

| Condition | Exit code |
|---|---|
| Bad vault name, unknown `--vault`/`switch` target, `add` of existing vault | 1 (user/input) |
| `state.json` corrupt or unsupported version; migration conflict (§6); current-vault dir missing (from state) | 2 (config/storage) |
| Render failures | 3 (unchanged) |

Every new error carries context (which name, which path, what to do next) and
preserves the `source()` chain, per the existing convention.

---

## 9. Reliability checklist (drives the tests)

- [ ] `state.json` writes are atomic (shared helper with `JsonStore::save`).
- [ ] Corrupt `state.json` → typed error, never silent reset.
- [ ] `state.json` from a newer schema → "unsupported version", exit 2.
- [ ] Vault names: allowlist enforced at construction *and* deserialization;
      normalization (lowercase) applied before uniqueness and path building.
- [ ] Unknown `--vault` / env vault → error; never auto-created.
- [ ] Missing current-vault dir → error (except `default`, auto-created);
      `vault` subcommands still work in that state (recovery path).
- [ ] Migration: atomic rename, idempotent, refuses when both layouts exist.
- [ ] `vault add` writes the empty store immediately (durability + early
      permission check).
- [ ] Stray non-vault entries in `vaults/` are warned about, not fatal.
- [ ] All prompts on stderr; EOF aborts every prompt (including the new menu).
- [ ] First run with no prior data: `default` vault appears; zero prompts.

---

## 10. Testing plan

All new tests are unit tests over tempdirs — no LaTeX engine, no process
spawn, consistent with the existing suite.

- **domain:** `VaultName` accept/reject table (empty, too long, uppercase
  folding, leading `-`, `..`, `/`, unicode); serde round-trip rejects invalid.
- **vaults:** `FsVaultStore` — create/list/current/set_current happy paths;
  missing `state.json` defaults to `default`; corrupt / newer-version state;
  duplicate create; stray file in `vaults/` ignored with warning; atomic-write
  behavior (state file never observed empty — assert via write-then-read).
- **migration:** fresh install (no-op), legacy-only (migrates, then re-run is a
  no-op), both-exist (typed error, both files untouched).
- **app:** `bootstrap_in` binds `JsonStore`/output dir to the resolved vault's
  paths; override precedence flag > env > state > default; isolation test —
  add into vault A, switch to B, `list` is empty, switch back, theorem is
  there with draw stats intact.
- **cli:** resolution errors map to the right exit codes; `vault add -i`
  validated-loop behavior is covered by the shared `prompt_label` tests plus
  a parse test for the new menu helper.

The two `#[ignore]`d latexmk tests stay ignored and untouched.

---

## 11. Implementation order

Each step compiles, passes tests, and stays warning-clean before the next.

1. **`domain::VaultName`** + tests. No behavior change.
2. **`vaults` module**: trait, `FsVaultStore`, errors, atomic-write helper
   extraction (refactor `JsonStore::save` onto it) + tests.
3. **Migration** in the vaults layer + tests.
4. **Config additions** (`vaults_dir`, `state_path`, per-vault path helpers).
5. **App layer**: `VaultEnv`, `App::bootstrap_in`; retire `App::bootstrap`'s
   hardcoded pathing. Existing tests must pass unmodified.
6. **CLI**: args (`--vault`, `vault` subcommands), handlers, menu-helper
   extraction from `delete -i`, dispatch split + tests.
7. **Docs**: README (user-facing: vault concept, commands, env var, migration
   note, `--vault`), CLAUDE.md (layout diagram, new module, gotchas: name
   normalization, no-registry decision, migration rules).

## 12. Future work (seams left ready)

- `vault delete <name>` — destructive; must require typed name confirmation
  (`--yes` alone insufficient) and refuse to delete the current vault.
- `vault rename` — atomic dir rename + state update ordering.
- Per-vault metadata (`vaults/<name>/vault.json`: created-at, description) —
  lives inside the vault by design (§3.1).
- Per-vault config overrides (draw count, default format).
- Cross-vault move/copy of a theorem.

# heist

Deterministic, token-free half of the [Heist](../README.md) pipeline: state tracking, worktree setup/teardown, and validation.md lookup. The plugin's agents shell out to this binary instead of doing this bookkeeping themselves.

## Build / install

```bash
cargo build --manifest-path cli/Cargo.toml
# or
cargo install --path cli
```

## Commands

All commands read/write `.heist/<slug>/state.json` relative to the current directory unless noted.

### `state init <slug>`

Creates `.heist/<slug>/state.json` with defaults (`stage: casing`, counters at 0, `worktree`/`branch`/`base` null). Fails if the slug directory already exists.

### `state get <slug> <field>`

Prints one field's value (or `null`). Fails on unknown field or missing/corrupt state.

### `state set <slug> <field> <value>`

Updates one field and bumps `updated` to today. Validates the value (e.g. `stage` must be a known stage, dates must parse, `slug`/`worktree`/`branch` can't be blank).

### `state incr <slug> <field>`

Reads a numeric field, adds 1, and writes it back (bumping `updated` to today). Fails on a non-numeric field, an unknown field, or overflow past `u32::MAX`. Not atomic: same single-writer contract as `state set`.

### `state schema`

Prints the field list and an example `state.json`. No slug required, deterministic output.

### `worktree add [--base <ref>] <slug>`

Creates a git worktree at `.worktrees/<slug>` on branch `heist/<slug>`, symlinks `.heist/<slug>` into it, and writes `worktree`/`branch` into state. Idempotent: re-running recreates a missing symlink instead of failing. Requires `state init` to have run first.

`--base <ref>` starts the branch from `<ref>` instead of `origin/<default>` (e.g. to stack a split piece on the previous piece's branch) and persists `base` in state. The ref must exist. On an already-existing worktree, re-adding with the same `--base` is idempotent, a different `--base` is refused (exit 2), and omitting `--base` leaves a previously persisted base untouched.

### `worktree remove <slug>`

Removes the worktree and local branch, then sets `stage: done`. Refuses if `heist/<slug>` isn't merged into the repo's default branch.

### `worktree cleanup [--dry-run]`

Removes every heist-owned worktree (path under `.worktrees/<slug>`, actual checked-out branch `heist/<slug>`) whose branch is already merged into the repo's default branch. For each heist-owned worktree prints one line: `removed <slug>`, `skipped <slug> (unmerged)`, `would remove <slug>` (with `--dry-run`), or `failed <slug>: <reason>`. Non-heist worktrees (different branch, detached HEAD, or outside `.worktrees/`) are left untouched and produce no output. Never forces removal of a dirty worktree. Continues past per-item failures (best-effort); exits 3 if any item failed, or if `origin/<default>` can't be resolved (single top-level error, no per-item output in that case). `--dry-run` previews without removing/deleting/saving anything.

### `validation check <path>`

Requires an absolute path; a relative or out-of-project path exits 4. Prints `ok` (exit 0) if a `validation.md` exists in `<path>`'s directory or any ancestor up to the repo root, `missing` (exit 2) otherwise.

### `validation resolve <path>...`

Requires absolute paths; a relative or out-of-project path exits 4. For each path, merges the nearest `validation.md` with the root `validation.md` (leaf sections override root sections `Build`/`Lint`/`Test`; `Docs`/`PR conventions`/`Notes` come from root). Prints one block per distinct scope, deduped.

### `review select <slug>`

Prints the reviewer lanes to run for the diff since the default branch, one bare lane name per line (e.g. `intent`, `coverage`, `quality`, `simplicity`, `rust`). Computes changed paths and classifies each by file type; `intent` always runs, `coverage` runs iff a programming file changed, `quality`/`simplicity` run iff any programming/prose/markup file changed, `rust` runs iff a Rust file changed. Exits 2 if state/branch is missing, or if `origin/<default>` doesn't resolve; exits 3 on any other git failure.

### score check <slug>

Parses `.heist/<slug>/score.md` and checks it for validity. On success prints `ok`, `steps: N`, `waves: M` and exits 0. On any structural or cross-step finding, prints one `step N: <message>` line per finding to stderr and exits 2. Exits 2 if no state or no `score.md` exists for the slug; exits 1 on a true IO read failure.

### score record <slug>

Runs the same parse + check as `score check`; on success additionally persists `score_steps_total`/`score_waves_total` into `state.json` and bumps `updated`, then prints `steps: N`, `waves: M`. On any finding, writes nothing and exits 2 (same output as `score check`).

### score wave <slug> <n>

Parses `.heist/<slug>/score.md` and prints wave `<n>`'s steps verbatim: first line `steps: K`, then each step's exact source text preceded by a `--- step N ---` delimiter line. Exits 2 if the wave number doesn't exist in the file, if no state or no `score.md` exists for the slug, or if the file fails to parse; exits 1 on a true IO read failure.

### `base <slug>`

Resolves the heist's recorded `base` against its PR state and prints three lines: `resolution:` (`null` | `live` | `expired` | `abandoned`), `merge_ref:` (the ref `sync` would use), and `pr_base:` (what the heist's own PR should target). `null` means no base is recorded, `live` means the base's PR is still open, `expired` means it merged, `abandoned` means it was closed unmerged. `abandoned` exits 2; the others exit 0. If the base's PR state can't be verified (no `gh`, no auth, network down), the command halts with exit 3 instead of guessing: user should fix their env.

### `sync <slug>`

Fetches origin, resolves the base like `heist base`, and updates the heist's branch accordingly:

- `null`: rebase onto `origin/<default>`.
- `live`: merge the base branch (never rebase, so a later squash-merge of the base doesn't get its commits replayed).
- `expired`: merge `origin/<default>`.
- `abandoned`: refuse with exit 5; a human must decide whether to drop, salvage, or reopen the base's commits.

Prints one `synced: ...` line naming what it did. Operates on the worktree recorded in state (safe to run from anywhere) and refuses (exit 2) if no worktree is recorded or the worktree is checked out on a different branch. A failed fetch aborts the sync: user should fix their env.

### `resume <slug>`

Prints a short summary (`slug`, `stage`, `mode`, `next`, `worktree`) for picking a heist back up. `next` is the doc file and step to resume from, resolved from `stage`/`mode` (`none` at `done`).

### `list`

Prints one line per heist under `.heist/` (`slug  stage  next  worktree  mode`), sorted by slug, including `done` heists. Empty or missing `.heist/` prints nothing and exits 0.

### `doctor`

Checks whether `git`, `gh`, and `crit` are on `PATH` and prints one `<tool>: ok` or `<tool>: missing` line per tool. Exits 0 if all are present, 2 if any is missing.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Internal error (e.g. unreadable file) |
| 2 | Precondition failed (missing/invalid state, unmerged branch, validation.md missing, bad input, score.md missing/invalid/no-such-wave) |
| 3 | Underlying git command failed |
| 4 | Invalid path argument (not absolute, or outside the project) |
| 5 | Abandoned-base halt (`sync` only): base PR closed unmerged, human decision required |

## Tests

```bash
cargo test --manifest-path cli/Cargo.toml
```

`tests/e2e/` drives the compiled binary end-to-end (including real git repos for worktree flows); `tests/integration/` exercises adapters directly.

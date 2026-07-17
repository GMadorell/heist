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

Creates `.heist/<slug>/state.json` with defaults (`stage: casing`, counters at 0, `worktree`/`branch` null). Fails if the slug directory already exists.

### `state get <slug> <field>`

Prints one field's value (or `null`). Fails on unknown field or missing/corrupt state.

### `state set <slug> <field> <value>`

Updates one field and bumps `updated` to today. Validates the value (e.g. `stage` must be a known stage, dates must parse, `slug`/`worktree`/`branch` can't be blank).

### `state incr <slug> <field>`

Reads a numeric field, adds 1, and writes it back (bumping `updated` to today). Fails on a non-numeric field, an unknown field, or overflow past `u32::MAX`. Not atomic: same single-writer contract as `state set`.

### `state schema`

Prints the field list and an example `state.json`. No slug required, deterministic output.

### `worktree add <slug>`

Creates a git worktree at `.worktrees/<slug>` on branch `heist/<slug>`, symlinks `.heist/<slug>` into it, and writes `worktree`/`branch` into state. Idempotent: re-running recreates a missing symlink instead of failing. Requires `state init` to have run first.

### `worktree remove <slug>`

Removes the worktree and local branch, then sets `stage: done`. Refuses if `heist/<slug>` isn't merged into the repo's default branch.

### `worktree cleanup [--dry-run]`

Removes every heist-owned worktree (path under `.worktrees/<slug>`, actual checked-out branch `heist/<slug>`) whose branch is already merged into the repo's default branch. For each heist-owned worktree prints one line: `removed <slug>`, `skipped <slug> (unmerged)`, `would remove <slug>` (with `--dry-run`), or `failed <slug>: <reason>`. Non-heist worktrees (different branch, detached HEAD, or outside `.worktrees/`) are left untouched and produce no output. Never forces removal of a dirty worktree. Continues past per-item failures (best-effort); exits 3 if any item failed, or if `origin/<default>` can't be resolved (single top-level error, no per-item output in that case). `--dry-run` previews without removing/deleting/saving anything.

### `validation check <path>`

Requires an absolute path; a relative or out-of-project path exits 4. Prints `ok` (exit 0) if a `validation.md` exists in `<path>`'s directory or any ancestor up to the repo root, `missing` (exit 2) otherwise.

### `validation resolve <path>...`

Requires absolute paths; a relative or out-of-project path exits 4. For each path, merges the nearest `validation.md` with the root `validation.md` (leaf sections override root sections `Build`/`Lint`/`Test`; `Docs`/`PR conventions`/`Notes` come from root). Prints one block per distinct scope, deduped.

### `review select <slug>`

Prints the reviewer lanes to run for the diff since the default branch, one bare lane name per line (e.g. `intent`, `coverage`, `quality`, `simplicity`, `rust`). Computes changed paths via `git2` (merge-base of `origin/<default>` and the slug's recorded branch, then a tree diff) and classifies each by file type; `intent` always runs, `coverage` runs iff a programming file changed, `quality`/`simplicity` run iff any programming/prose/markup file changed, `rust` runs iff a Rust file changed. Exits 2 if state/branch is missing, 3 on a git failure.

### `resume <slug>`

Prints a short summary (`slug`, `stage`, `next_step`, `worktree`) for picking a heist back up.

### `list`

Prints one line per heist under `.heist/` (`slug  stage  next_step  worktree`), sorted by slug, including `done` heists. Empty or missing `.heist/` prints nothing and exits 0.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Internal error (e.g. unreadable file) |
| 2 | Precondition failed (missing/invalid state, unmerged branch, validation.md missing, bad input) |
| 3 | Underlying git command failed |
| 4 | Invalid path argument (not absolute, or outside the project) |

## Tests

```bash
cargo test --manifest-path cli/Cargo.toml
```

`tests/e2e/` drives the compiled binary end-to-end (including real git repos for worktree flows); `tests/integration/` exercises adapters directly.

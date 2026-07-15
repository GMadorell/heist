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

### `state schema`

Prints the field list and an example `state.json`. No slug required, deterministic output.

### `worktree add <slug>`

Creates a git worktree at `.worktrees/<slug>` on branch `heist/<slug>`, symlinks `.heist/<slug>` into it, and writes `worktree`/`branch` into state. Idempotent: re-running recreates a missing symlink instead of failing. Requires `state init` to have run first.

### `worktree remove <slug>`

Removes the worktree and local branch, then sets `stage: done`. Refuses if `heist/<slug>` isn't merged into the repo's default branch.

### `validation check <path>`

Prints `ok` (exit 0) if a `validation.md` exists in `<path>`'s directory or any ancestor up to the repo root, `missing` (exit 2) otherwise.

### `validation resolve <path>...`

For each path, merges the nearest `validation.md` with the root `validation.md` (leaf sections override root sections `Build`/`Lint`/`Test`; `Docs`/`PR conventions`/`Notes` come from root). Prints one block per distinct scope, deduped.

### `resume <slug>`

Prints a short summary (`slug`, `stage`, `next_step`, `worktree`) for picking a heist back up.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Internal error (e.g. unreadable file) |
| 2 | Precondition failed (missing/invalid state, unmerged branch, validation.md missing, bad input) |
| 3 | Underlying git command failed |

## Tests

```bash
cargo test --manifest-path cli/Cargo.toml
```

`tests/e2e/` drives the compiled binary end-to-end (including real git repos for worktree flows); `tests/integration/` exercises adapters directly.

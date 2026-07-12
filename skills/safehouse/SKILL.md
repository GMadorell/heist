---
name: safehouse
description: Use when a heist's Forging stage is done and implementation needs a worktree, or when a merged heist branch needs teardown.
argument-hint: "<slug> | cleanup <slug>"
---

# /heist:safehouse

Arg: slug → setup. `cleanup <slug>` → teardown after merge.

## Setup

Precondition: `.heist/<slug>/state.json` and `score.md` exist (Forging ran). Missing → stop, say so.

**Re-entry**: if `../<repo-name>-heist-<slug>` already a worktree (`git worktree list`), skip to step 3 — don't re-add. Verify the symlink exists and resolves correctly (re-create if missing/broken), confirm exclude, re-report path.

1. Main branch name from `validation.md` (`## PR conventions`), else `git remote show origin`.
2. `git worktree add ../<repo-name>-heist-<slug> -b heist/<slug> origin/<main>`. `<repo-name>` = current dir basename.
3. Symlink `.heist/<slug>/` into the worktree at the same relative path, pointing at the main repo's absolute path: `ln -s <main-repo-abs>/.heist/<slug> <worktree-abs>/.heist/<slug>` (create the worktree's `.heist/` dir first if needed). One file, read/written from either location — no copy, no drift.
4. Update `state.json` (single file, via either path): `stage: "implementing"`, `worktree: <abs path>`, `branch: "heist/<slug>"`, `updated: <today>`.
5. Report worktree's absolute path (Wheelman's working dir).

## Cleanup

1. Confirm `heist/<slug>` merged into main (`git branch --merged origin/<main>`). Not merged → stop, ask.
2. `git worktree remove ../<repo-name>-heist-<slug>` (`--force` only if verified clean).
3. `git branch -d heist/<slug>` (lowercase — refuses if unmerged).
4. Leave `.heist/<slug>/` in main repo unless user asks to remove.
5. `state.json` `stage: "done"` if not already.

## Note

Don't use built-in `EnterWorktree`/`ExitWorktree`: fixed path convention, no custom branch naming, cleanup doesn't survive resumed sessions. Plain `git worktree` only.

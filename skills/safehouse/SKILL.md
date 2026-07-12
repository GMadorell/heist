---
name: safehouse
description: Use when a heist's Forging stage is done and implementation needs a worktree, or when a merged heist branch needs teardown.
argument-hint: "<slug> | cleanup <slug>"
---

# /heist:safehouse

Arg: slug → setup. `cleanup <slug>` → teardown after merge.

## Setup

Precondition (context-dependent):
- **Step 2 (Planning-start, first-time)**: `.heist/<slug>/state.json` must exist (slug and initial state created by Slugger). `score.md` not yet present.
- **Step 6 (Safehouse in pipeline, re-entry)**: `.heist/<slug>/state.json` and `score.md` must both exist (Forging ran, implementation ready). Presence of `score.md` indicates which invocation point.

Note: The actual setup steps (worktree add, symlink, state update) don't technically depend on `score.md`. Its presence distinguishes first-time setup from re-entry after Forging. Missing files → stop, report which one(s).

**Re-entry**: if `../<repo-name>-heist-<slug>` already a worktree (`git worktree list`), skip to step 3 — don't re-add. Verify the symlink exists and resolves correctly (re-create if missing/broken), confirm `.heist/<slug>/` is excluded from git (in `.gitignore` or `.git/info/exclude`), re-report path.

1. Main branch name from `validation.md` (`## PR conventions`), else `git remote show origin`.
2. `git worktree add ../<repo-name>-heist-<slug> -b heist/<slug> origin/<main>`. `<repo-name>` = current dir basename. If this fails (permission denied, branch already exists, origin/<main> unreachable), stop and report the git error to the human — don't proceed or guess at a workaround (e.g., force-deleting an existing branch).
3. Symlink `.heist/<slug>/` into the worktree at the same relative path, pointing at the main repo's absolute path: `ln -s <main-repo-abs>/.heist/<slug> <worktree-abs>/.heist/<slug>` (create the worktree's `.heist/` dir first if needed). One file, read/written from either location — no copy, no drift.
4. Update `state.json` (single file, via either path): `stage: "implementing"`, `worktree: <worktree-abs>`, `branch: "heist/<slug>"`, `updated: <today>`.
5. Report worktree's absolute path (Wheelman's working dir).

## Cleanup

1. Confirm `heist/<slug>` merged into main (`git branch --merged origin/<main>`). Not merged → stop, ask.
2. `git worktree remove ../<repo-name>-heist-<slug>` (`--force` only if verified clean).
3. `git branch -d heist/<slug>` (lowercase — refuses if unmerged).
4. Leave `.heist/<slug>/` in main repo unless user asks to remove.
5. `state.json` `stage: "done"` if not already.

## Note

Don't use built-in `EnterWorktree`/`ExitWorktree`: fixed path convention, no custom branch naming, cleanup doesn't survive resumed sessions. Plain `git worktree` only.

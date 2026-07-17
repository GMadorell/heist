---
name: heist
description: Entry point and orchestrator for the heist workflow. Invoke when the user wants to plan/brainstorm/design and implement a difficult task, or when the user wants to continue a heist.
argument-hint: "[heavy|medium|light] <description of change | path(s) to plan file(s)>"
---

# /heist

You are driving the heist orchestrator in the current project.

## No arguments

Run `heist list`, filter to rows whose `stage` isn't `done`.

- Zero active rows: tell the human nothing is in progress, point them to `/heist:heist [<mode>] <description of the change>`. Stop — don't read further docs.
- One or more active rows: read `resume.md` (in this skill's directory) and follow it.

## With a description or a plan

`/heist [<mode>] [--base <branch>] <description>` (either token order accepted: `[--base <branch>] [<mode>] <description>` also valid).

`<mode>` is an optional token, matched case-insensitively against `heavy`, `medium`, `light`.
`--base <branch>` is an optional leading token that specifies the base branch for the worktree. When present, it's stripped and carried through step 2 of `pipeline.md`.

- Mode given: strip it off; the rest is the description.
- `--base <branch>` given: strip it off; carry the branch value through step 2 of `pipeline.md`, so the `heist worktree add <slug>` call becomes `heist worktree add --base <branch> <slug>`.
- `--base <branch>` omitted: step 2 runs `heist worktree add <slug>` exactly as before, no behavior change.
- Mode omitted: ask via `AskUserQuestion` before anything else — don't default silently. Present `heavy` as recommended.
  - `heavy` (recommended) — full pipeline: Fence review, Forger/score.md, Wheelman+Muscle, Cleaner.
  - `medium` — same as heavy, minus Fence review.
  - `light` — plan + human review only, then direct implementation and a manual crit review of the diff. For small, well-understood changes.

### Note on base branches and `heat.md`

When `heat.md` (written during step 2a's split path) generates per-piece `/heist:heist` prompts, each always includes `--base heist/<piece-slug>` verbatim, never a bare branch name like `main`. This avoids staleness: a human hand-typing a base branch name days after `heat.md` was generated could reference an outdated branch.

### Plan detection

After stripping the mode token and `--base <branch>` (if present), check other parts of the input for file paths.
No file paths found -> normal description path, read `pipeline.md` (in this skill directory), and run it from step 1, carrying `<mode>`, `<branch>` (if given), and the description.
File paths found -> if any empty or unreadable, halt. Show diagnostic to user. If all file paths are good, continue to next subsection.

### Plan-based heist: confirm before spending resources

1. Canonicalize filepaths to absolute paths.
2. Ask the human via `AskUserQuestion`: list the source-set file(s) with absolute path and size, the total combined size, the mode; ask whether to proceed as a plan-based heist.
   - Declined: fall back to the normal description path — read `pipeline.md` from step 1, carrying `<mode>`, `<branch>` (if given), the prose (if any) and the files, which in this case are used as context. If there was no description or the files didn't have context, ask the human for a description via `AskUserQuestion` first.
   - Confirmed: read `pipeline.md` (in this skill's directory) and run it from step 1, carrying `<mode>`, `<branch>` (if given), "plan-based heist", the source set (absolute paths), and the prose through.

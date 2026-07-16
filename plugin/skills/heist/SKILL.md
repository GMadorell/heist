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

`/heist [<mode>] <description>`. `<mode>` is an optional first token, matched case-insensitively against `heavy`, `medium`, `light`.

- Mode given: strip it off; the rest is the description.
- Mode omitted: ask via `AskUserQuestion` before anything else — don't default silently. Present `heavy` as recommended.
  - `heavy` (recommended) — full pipeline: Fence review, Forger/score.md, Wheelman+Muscle, Cleaner.
  - `medium` — same as heavy, minus Fence review.
  - `light` — plan + human review only, then direct implementation and a manual crit review of the diff. For small, well-understood changes.

### Plan detection

After stripping the mode token, resolve each remaining whitespace-separated token against the filesystem: strip trailing punctuation, expand shell globs, then check whether it names an existing file relative to the current working directory (the main checkout, not the worktree, which does not exist yet).

- No token resolves to an existing file: normal description path — read `pipeline.md` (in this skill's directory) and run it from step 1, carrying `<mode>` and the raw description through. Stop reading `SKILL.md` here.
- A token resolves to an existing file that is empty or unreadable: halt immediately with a diagnostic naming the file. Do not create state, a slug, or a worktree — there is nothing to clean up yet.
- One or more tokens resolve to existing, readable, non-empty files: plan mode, continue to the next subsection.

### Plan mode: confirm before spending resources

1. Canonicalize every matched file to an absolute path — this is the source set. Every remaining, non-file token is prose.
2. Spawn `heist:slugger` (foreground, one-shot) with the prose (if any) plus the basename(s) of every source-set file — never the file contents. Parse the returned slug.
3. Ask the human via `AskUserQuestion`: list the source-set file(s) with absolute path and size, the total combined size, the mode, and the derived slug; ask whether to proceed as a plan-based heist.
   - Declined: fall back to the normal description path — read `pipeline.md` from step 1, carrying `<mode>` and the prose (if any) as the raw description. If there was no prose, ask the human for a description via `AskUserQuestion` first.
   - Confirmed: read `pipeline.md` (in this skill's directory) and run it from step 1, carrying `<mode>`, "plan mode", the derived slug, the source set (absolute paths), and the prose through.

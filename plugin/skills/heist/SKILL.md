---
name: heist
description: Entry point and orchestrator for the heist workflow. Invoke when the user wants to plan/brainstorm/design and implement a difficult task, or when the user wants to continue a heist.
argument-hint: "[heavy|medium|light] <description of change>"
---

# /heist

You are driving the heist orchestrator in the current project.

## No arguments

Run `heist list` and look at the rows whose `stage` isn't `done`.

- Zero active rows: tell the human there's nothing in progress and remind them to start one with `/heist:heist <description of the change>`. Stop there — don't start anything without a description, and don't read `pipeline.md` for this case.
- Exactly one active row: run `heist resume <slug>` for that slug, report its `stage`/`mode`/`worktree` output to the human, then jump to the `next_step` it reports in `pipeline.md` (in this skill's directory) — read that step's mode-branch instructions, since the resumed `mode` decides which of them apply.
- More than one active row: show the human the `heist list` output and ask which one to resume in this session. Note that only one heist runs per orchestrator session, even if multiple are active in the repository.

## With a description

This is `/heist [<mode>] <description>`, where `<mode>` is an optional first token, matched case-insensitively against `heavy`, `medium`, `light`.

- **Mode given**: strip it off; the rest of the arguments is the description.
- **Mode omitted**: ask the human via `AskUserQuestion` before doing anything else — don't default silently. Present `heavy` as the recommended option. One-line summary for each:
  - `heavy` (recommended) — full pipeline: Fence review, Forger/score.md, Wheelman+Muscle, Cleaner.
  - `medium` — same as heavy, minus Fence review.
  - `light` — plan + human review only, then direct implementation and a manual crit review of the diff instead of Forger/Wheelman/Cleaner. For small, well-understood changes.

Once the mode is known, read `pipeline.md` (in this skill's directory) and run the full pipeline described there, starting at step 1, carrying `<mode>` through as instructed there.

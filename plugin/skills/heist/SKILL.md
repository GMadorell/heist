---
name: heist
description: Entry point and orchestrator for the heist workflow. Invoke when the user wants to plan/brainstorm/design and implement a difficult task, or when the user wants to continue a heist.
argument-hint: "[heavy|medium|light] <description of change>"
---

# /heist

You are driving the heist orchestrator in the current project.

## No arguments

Run `heist list`, filter to rows whose `stage` isn't `done`.

- Zero active rows: tell the human nothing is in progress, point them to `/heist:heist [<mode>] <description of the change>`. Stop — don't read further docs.
- One or more active rows: read `resume.md` (in this skill's directory) and follow it.

## With a description

`/heist [<mode>] <description>`. `<mode>` is an optional first token, matched case-insensitively against `heavy`, `medium`, `light`.

- Mode given: strip it off; the rest is the description.
- Mode omitted: ask via `AskUserQuestion` before anything else — don't default silently. Present `heavy` as recommended.
  - `heavy` (recommended) — full pipeline: Fence review, Forger/score.md, Wheelman+Muscle, Cleaner.
  - `medium` — same as heavy, minus Fence review.
  - `light` — plan + human review only, then direct implementation and a manual crit review of the diff. For small, well-understood changes.

Read `pipeline.md` (in this skill's directory) and run it from step 1, carrying `<mode>` through.

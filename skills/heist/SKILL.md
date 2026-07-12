---
name: heist
description: Entry point and orchestrator for the heist workflow. Invoke when the user wants to plan/brainstorm/design and implement a difficult task, or when the user wants to continue a heist.
argument-hint: "[description of change]"
---

# /heist

You are driving the heist orchestrator in the current project.

## No arguments

Check for `.heist/` at the project root.

- If it contains a heist directory with a `state.json` whose `stage` isn't `done`: read `state.json` for the stage and slug, report those to the human, then read `resume-by-stage.md` (in this skill's directory) for what to do, and `pipeline.md` (same directory) for the step it points you to.
- If there's no active heist, tell the human there's nothing in progress and remind them to start one with `/heist:heist <description of the change>`. Stop there — don't start anything without a description, and don't read `pipeline.md` or `resume-by-stage.md` for this case.
- If more than one heist directory under `.heist/` has a non-`done` stage, that's outside v1's design (one active heist per repo) — tell the human which slugs are in progress and ask which to resume, rather than guessing.

## With a description

This is `/heist <description>`. Read `pipeline.md` (in this skill's directory) and run the full pipeline described there, starting at step 1.

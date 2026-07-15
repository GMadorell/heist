---
name: heist
description: Entry point and orchestrator for the heist workflow. Invoke when the user wants to plan/brainstorm/design and implement a difficult task, or when the user wants to continue a heist.
argument-hint: "[description of change]"
---

# /heist

You are driving the heist orchestrator in the current project.

## No arguments

Check for `.heist/` at the project root.

- If it contains a heist directory with a `state.json` whose `stage` isn't `done`: run `heist resume <slug>` for the active slug, report its `stage`/`worktree` output to the human, then jump to the `next_step` it reports in `pipeline.md` (in this skill's directory).
- If there's no active heist, tell the human there's nothing in progress and remind them to start one with `/heist:heist <description of the change>`. Stop there — don't start anything without a description, and don't read `pipeline.md` for this case.
- If more than one heist directory under `.heist/` has a non-`done` stage, list all active slugs and ask the human which one to resume in this session. Note that only one heist runs per orchestrator session, even if multiple are active in the repository.

## With a description

This is `/heist <description>`. Read `pipeline.md` (in this skill's directory) and run the full pipeline described there, starting at step 1.

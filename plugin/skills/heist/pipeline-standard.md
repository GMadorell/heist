# Heist pipeline: standard delivery (heavy + medium)

Runs after core `pipeline.md` steps 1-4, for `heavy` and `medium` mode.

### 5. Forging

The Mastermind's job ends at approval — forging is a fresh, one-shot transformation.

1. Spawn `heist:forger` (foreground, one-shot) with the worktree absolute path and a `cd` instruction, so it reads `blueprint.md`, resolves validation via `heist validation resolve <absolute-path>`, and writes `score.md`.
2. Run `heist state set <slug> stage safehouse`, `heist state set <slug> score_steps_total <step-count>`, and `heist state set <slug> score_waves_total <wave-count>` (Forger reports both counts).
3. Report to the human: `score.md` path, step count, implicit calls flagged.
4. Continue to implementing.

### 6. Implementing (Wheelman + Muscle)

1. Spawn `heist:wheelman` (foreground) with task `<slug>`.
2. Let it run its full per-wave loop autonomously. Don't intervene per-wave.
3. When done, run `heist state set <slug> stage cleaning`. Wheelman owns `score_wave` via `heist state incr` throughout — don't re-set it here.
4. Report to the human: waves completed, anything Wheelman did itself and why, build status.
5. Continue to cleaning.

### 7. Cleaning (The Cleaner)

1. Spawn `heist:cleaner` (foreground) with task `<slug>`. It runs its own pipeline.
2. Handling output:
   - Adversarial review finds a critical path: surface to human, stop.
   - Mechanical failures: run `heist state set <slug> stage implementing`. Spawn a fresh `heist:wheelman` in the same worktree with the Cleaner's failure report, telling it to fix and re-verify (not re-run the whole score). When done, re-run the Cleaner from step 1.
   - Findings/warnings to ask the user: stop, ask what to do, then re-clean (don't rerun the whole score, just fix the given things).
3. Success: run `heist state set <slug> stage done`. Report PR URL, risk label, and any findings worth attention.
4. Worktree teardown out of scope — reclaim by hand with `heist worktree remove <slug>` after merge.

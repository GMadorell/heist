# Resuming by stage

`/heist` with no args, on a heist whose `stage` isn't `done`: every stage's own work is either fully persisted to a file or safely re-runnable, so resuming almost always means "re-enter that stage's step in `pipeline.md`," not some separate recovery procedure. Stage-by-stage:

| `stage` | What to do on resume |
|---|---|
| `casing` | Nothing heist-specific was written yet. Re-enter step 1 (casing gate). |
| `planning` | If Mastermind subagent is still alive in this session, continue the relay loop where it left off. Otherwise, read slug from `state.json`, then spawn fresh Mastermind with worktree path and slug. Abort if worktree does not exist, requires human intervention. |
| `fence_review` | `blueprint.md` exists. Re-enter step 3 from the top — re-running Fence is cheap and one-shot. |
| `human_review` | `blueprint.md` exists, possibly mid-round. Re-enter step 4 from the top: `crit .heist/<slug>/blueprint.md` reconnects to the review file's persisted state (crit tracks rounds/comments there, not in heist's `state.json`), so this works whether a round was mid-flight or already finished. |
| `forging` | `blueprint.md` is approved. Re-enter step 5 — re-running the Forger just overwrites `score.md` with a fresh transformation (a refresh, same as `heist:casing` re-running). |
| `safehouse` | `score.md` exists. Re-enter step 6. Worktree from step 2 should exist.  Run `git worktree list` and check for `.worktrees/<slug>`. If worktree does not exist raise it to the user.|
| `implementing` | Worktree exists. Read `.heist/<slug>/state.json` (symlinked into the worktree) for `score_step` and `worktree` path, and tell the fresh Wheelman spawn explicitly which step to resume from — don't restart at step 1 and don't re-verify already-committed steps. |
| `cleaning` | Worktree exists, implementation is done. Re-enter step 8 from the top — the Cleaner's pipeline is idempotent (mergeable/build/lint/test/docs all just re-check current state), so a fresh run is always safe here. |
| `done` | Nothing to resume — this is the "no active heist" case. |

If `state.json` itself is missing or unparseable for a slug directory that exists under `.heist/`, don't guess — tell the human the state file is corrupt/missing and ask whether to restart that stage or abandon the slug. Silently reconstructing state from file presence alone (e.g. "score.md exists, so stage must be forging-or-later") is exactly the kind of guess that compounds into a worse mess than asking.

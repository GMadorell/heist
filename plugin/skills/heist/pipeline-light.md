# Heist pipeline: light delivery

Runs after core `pipeline.md` steps 1-4, for `light` mode. No score.md, no Forger, no Wheelman/Muscle, no Cleaner. The orchestrator implements the approved blueprint directly; a manual crit pass over the diff stands in for automated adversarial review.

1. Run `heist state set <slug> stage implementing`.
2. `cd <worktree-path>` and implement the blueprint directly — no score.md, no sub-agent dispatch. Use your own judgment on ordering and scope.
3. Run `heist state set <slug> stage cleaning`.
4. Manual crit review of the diff: use `crit` to review the actual code changes (not `blueprint.md` — the diff). Same defensive PATH check as core step 4. If the human leaves comments, address them directly in the worktree and re-request review. Repeat until no comments.
5. Mergeable: ensure everything is committed, then run `heist sync <slug>`. Exit 0: proceed. Nonzero exit from a real conflict: resolve trivial conflicts yourself in the worktree and re-run `heist sync <slug>` until it exits 0, or surface genuine conflicts to the human and stop. If it exits with the abandoned-base precondition code (2), stop and report to the human immediately: this is a deliberate halt requiring a human decision (drop, salvage, or reopen the base), not something to auto-resolve.
6. Mechanical: run build, lint, full test suite per `heist validation resolve <absolute-path>`. All green, or fix and re-run until they are — don't proceed on red.
7. Docs pass: per `heist validation resolve <absolute-path>`'s Docs section.
8. Getaway: run `heist base <slug>` first and pass its `pr_base` value to `gh pr create --base <pr_base>`. PR body: summary, note this heist ran in `light` mode (diff reviewed manually via crit, not the automated Cleaner review agents). If `heist base <slug>`'s `stale` field prints `true`, note it in the PR body as a heads-up that the base may already be merging, with the remedy `gh pr edit <n> --base <main>` once confirmed.
9. Run `heist state set <slug> stage done`. Report the PR URL to the human.

Worktree teardown out of scope — reclaim by hand with `heist worktree remove <slug>` after merge.

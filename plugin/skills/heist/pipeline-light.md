# Heist pipeline: light delivery

Runs after core `pipeline.md` steps 1-4, for `light` mode. No score.md, no Forger, no Wheelman/Muscle, no Cleaner. The orchestrator implements the approved blueprint directly; a manual crit pass over the diff stands in for automated adversarial review.

1. Run `heist state set <slug> stage implementing`.
2. `cd <worktree-path>` and implement the blueprint directly — no score.md, no sub-agent dispatch. Use your own judgment on ordering and scope.
3. Run `heist state set <slug> stage cleaning`.
4. Manual crit review of the diff: use `crit` to review the actual code changes (not `blueprint.md` — the diff). Same defensive PATH check as core step 4. If the human leaves comments, address them directly in the worktree and re-request review. Repeat until no comments.
5. Mergeable: ensure everything is committed, then run `heist sync <slug>`; exit 0 proceed. Exit 5 is the abandoned-base halt: stop and hand to the human, do not auto-resolve. Any other nonzero exit is a conflict/env failure: resolve trivial conflicts yourself and re-run until 0, or surface genuine ones. Follow its stderr.
6. Mechanical: run build, lint, full test suite per `heist validation resolve <absolute-path>`. All green, or fix and re-run until they are — don't proceed on red.
7. Docs pass: per `heist validation resolve <absolute-path>`'s Docs section.
8. Getaway: run `heist base <slug>`, pass its `pr_base` to `gh pr create --base <pr_base>`; on exit 3 stop and report the environment problem to the human instead of opening the PR. PR body: summary, note this heist ran in `light` mode (diff reviewed manually via crit, not the automated Cleaner review agents).
9. Run `heist state set <slug> stage done`. Report the PR URL to the human.

Worktree teardown out of scope — reclaim by hand with `heist worktree remove <slug>` after merge.

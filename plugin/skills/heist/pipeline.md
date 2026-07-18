# Heist pipeline: core (steps 1-4)

Covers casing, planning, fence review, human review — shared by every mode. After human review, mode routes to a delivery tail: `pipeline-standard.md` (heavy/medium) or `pipeline-light.md` (light).

## Modes

- `heavy` (default): full pipeline.
- `medium`: skip fence review (step 3).
- `light`: skip fence review; delivery tail is `pipeline-light.md`.

Mode is chosen before step 2, persisted via `heist state set <slug> mode <mode>`, fixed for the heist's lifetime.

## Preflight

If `heist` isn't on `PATH`, halt and point the human to the README install section. Otherwise proceed.

### 1. Casing gate

Run `heist validation check <repo-root-absolute-path>`.

- exit 0: proceed.
- exit 2: invoke the `heist:casing` skill yourself, then continue.
- any other nonzero exit (e.g. 4): halt, surface the raw stderr to the human.

### 2. Planning

1. Determine the slug. If a `<slug>` was carried in (a piece from a `heat.md` prompt passes `--slug`), use it verbatim and skip the Slugger. Otherwise spawn `heist:slugger` (foreground, one-shot) with the info you have on the input and parse the returned slug.
2. Run `heist state init <slug>`.
3. Run `heist state set <slug> mode <mode>`.
4. Run `heist worktree add <slug>` (append `--base <branch>` when a `<branch>` was carried in).
5. Run `heist state set <slug> stage planning`.

#### 2a. No plan detected: relay loop with the Mastermind

1. Spawn `heist:mastermind` (foreground) with: raw description, slug, worktree absolute path, explicit `cd <worktree-path>` instruction.
2. Relay loop — each Mastermind reply is a structured question, `SPLIT_PROPOSED`, or `INTERVIEW_COMPLETE`.
   - Every structured question gets an `AskUserQuestion` call. Never answer on the human's behalf.
   - Structured question has `QUESTION:`, `OPTIONS:`, `RECOMMENDATION:` lines. Map to `AskUserQuestion`: `question` = QUESTION text; `header` = short invented label; `options` = OPTIONS reordered with the recommended one first, `(Recommended)` appended to its label. Relay the human's answer verbatim via `SendMessage`, wait for the next reply. Loop.
   - `SPLIT_PROPOSED`: show the human the piece list via `AskUserQuestion` with exactly three options: `accept`, `reject (continue unsplit)`, `redraw`.
     - Reject: relay `SPLIT_REJECTED` to the Mastermind, then continue the existing relay loop unchanged (treat as any other turn).
     - Redraw: relay `SPLIT_REDRAW` plus the human's stated feedback, expect a fresh `SPLIT_PROPOSED` reply, and re-run this gate.
     - Accept: relay `SPLIT_ACCEPTED`. The Mastermind writes `.heist/<slug>/heat.md`. Run it through `crit` the same way step 4 (human review) runs `crit` over `blueprint.md`: relay any comments to the Mastermind, ask it to apply them, repeat until the human leaves no comments. Once approved: run `heist worktree remove <slug>`. Tell the human the parent heist is done, and that `.heist/<slug>/heat.md` now holds one copy-pasteable `/heist:heist` prompt per piece plus how to sequence them.
   - `INTERVIEW_COMPLETE`: the Mastermind has written `.heist/<slug>/blueprint.md`. Run `heist state set <slug> stage fence_review` (heavy) or `stage human_review` (medium/light). Show the human the summary and blueprint path. Keep the Mastermind subagent alive.
   - Reply matches neither shape: remind it of the expected format once; if it repeats, stop and show the human the raw reply.
3. heavy: continue to fence review below. medium/light: continue to human review below (stage already set).

Resuming the Mastermind after turn 1: `SendMessage` to the still-alive subagent, if same session. After a session restart, spawn a fresh `heist:mastermind` with `blueprint.md`'s current content plus what needs applying — it doesn't need the old transcript.

#### 2b. Plan detected: one-shot import with the Mastermind

1. Spawn `heist:mastermind` (foreground) in import mode with: the absolute path(s) of every source-set file, the prose (if any), the slug, the worktree absolute path, an explicit `cd <worktree-path>` instruction, and an explicit instruction to use its import mode.
2. The Mastermind replies once with `INTERVIEW_COMPLETE` — it has written `.heist/<slug>/blueprint.md`. Run `heist state set <slug> stage fence_review` (heavy) or `stage human_review` (medium/light). Show the human the summary (including any gaps or stale/false plan assertions the Mastermind flagged) and the blueprint path. Keep the Mastermind subagent alive.
3. heavy: continue to fence review below. medium/light: continue to human review below (stage already set).

Session restart while `stage` is `planning` for a plan-based heist: the plan file paths and prose are not persisted in state, so resume cannot re-run the import. If `.heist/<slug>/blueprint.md` already exists, resume by spawning a fresh `heist:mastermind` with its current content (same as 2a's resume note). If it doesn't exist yet, tell the human the import didn't finish and ask them to re-invoke `/heist:heist` with the same plan file(s).

### 3. Fence review

Heavy only. medium/light skip this (stage is already `human_review`).

1. Spawn `heist:fence` (foreground, one-shot) with the worktree absolute path and a `cd` instruction. Read its findings.
2. No findings above low, or Fence says the blueprint holds: run `heist state set <slug> stage human_review`. Tell the human it passed clean. Continue to human review.
3. Findings exist: relay them to the Mastermind, ask it to revise `blueprint.md`. Run `heist state incr <slug> fence_rounds`.
4. Mastermind revises, replies with a summary and any finding it disagreed with.
5. One auto-revision round only — do not send the revision back to Fence. Run `heist state set <slug> stage human_review`.
6. Report to the human: Fence's findings, the revision summary, any disagreement. Continue to human review.

### 4. Human review (crit)

If `crit` isn't on `PATH` (`command -v crit`), print the install command (`claude plugin marketplace add tomasz-tomczyk/crit && claude plugin install crit@crit`) and halt.

Use `crit` to review `<worktree-path>/.heist/<slug>/blueprint.md`. If the human leaves comments, relay them to the Mastermind, ask it to apply them, answer each comment with what the Mastermind decided. Repeat until the human leaves no comments — that's approval.

- heavy/medium: continue to `pipeline-standard.md`.
- light: continue to `pipeline-light.md`.

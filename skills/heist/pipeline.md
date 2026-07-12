# Heist pipeline (steps 1-8)

This is the full `/heist <description>` pipeline: casing → planning → fence review → human review → forging → safehouse → implementing → cleaning. All stages are wired end to end — run the full pipeline per the steps below.

The pipeline runs stage-to-stage without stopping; the only stage that waits on a human is human review (step 4). Steps below say "continue into X" as a pointer, not a reminder — no need to restate "don't stop" each time.

### 1. Casing gate

If `validation.md` doesn't exist at the repo root, invoke the `heist:casing` skill's instructions yourself before continuing (don't ask the human to do it as a separate step — this is the "auto-triggered by /heist when validation.md missing" behavior). If it exists, proceed directly.

### 2. Planning: relay loop with the Mastermind

1. Spawn `heist:slugger` (foreground, one-shot) with the raw change description the user gave `/heist`. The answer will be a slug, parse it.
2. Ensure `.heist/<slug>/` at the repo root exists.
3. Write `.heist/<slug>/state.json` from `templates/state.json` (in this plugin's directory) with `slug` set, `stage: "planning"`, `created`/`updated` set to today.
4. Spawn `heist:mastermind` (foreground, i.e. `run_in_background: false` on the Agent tool call) with a task message containing: the raw change description and the slug.
5. **The relay loop**: each Mastermind reply is either a structured question or the completion signal.
   - **Structured question** — it has `QUESTION:`, `OPTIONS:`, `RECOMMENDATION:` lines. Map it onto `AskUserQuestion`:
     - `question` = the QUESTION text
     - `header` = a short (≤12 char) label you invent from the topic
     - `options` = the OPTIONS list, reordered so the recommended option is **first** with `(Recommended)` appended to its label, per `AskUserQuestion`'s own convention
     - Present it to the human, get their answer (they may pick an option or type free text via "Other")
     - Relay the answer back via `SendMessage` to the Mastermind (send the option's label/description or the free text verbatim — don't paraphrase), then wait for its next reply. Loop.
   - **`INTERVIEW_COMPLETE`** — the Mastermind will have written `.heist/<slug>/blueprint.md` itself and replied with a short summary. Update `state.json`: `stage: "fence_review"`, `updated` to today. Show the human the summary and tell them the blueprint is at `.heist/<slug>/blueprint.md`. **Do not end the Mastermind subagent here** — keep it alive; fence review may need to resume it for a revision.
   - If a reply matches neither shape, treat it as a protocol violation: resume once with a reminder of the expected format; if it happens twice, stop and show the human the raw reply rather than looping forever.
6. Once `stage` is `"fence_review"`, continue into fence review below.

**Talking to the Mastermind after turn 1, in general**: "relay to the Mastermind" means `SendMessage` to the still-alive subagent, if this is the same session it was spawned in. After a session restart there's no live subagent to resume (subagent conversations don't survive it) — spawn a **fresh** `heist:mastermind` with `blueprint.md`'s current content plus whatever needs applying (findings, comments); it doesn't need the old interview transcript to revise a document it can just read. Only the interview itself (turn-by-turn question relay) can't be resumed cross-session, since the questions aren't persisted anywhere.

### 3. Fence review

1. Spawn `heist:fence` (foreground, one-shot — no relay loop for Fence itself) with the path to `.heist/<slug>/blueprint.md` plus `validation.md`. Read its findings.
2. **No findings above `low`, or Fence explicitly says the blueprint holds up**: stage → `"human_review"`, `updated` to today. Tell the human the blueprint passed contrarian review clean, then continue into human review below.
3. **Findings exist**: relay them to the Mastermind (see "Talking to the Mastermind after turn 1" above) and ask it to revise `blueprint.md`. Increment `fence_rounds` in `state.json`.
4. The Mastermind revises and replies with a short summary of what changed, plus any finding it explicitly disagreed with and why.
5. **This is the one auto-revision round — do not send the revised blueprint back to Fence again.** Regardless of whether Fence would still object, move on: stage → `"human_review"`, `updated` to today.
6. Report to the human in one place: Fence's original findings, the Mastermind's revision summary, and any disagreement the Mastermind raised with a Fence finding it chose not to apply. Then continue into human review below.

### 4. Human review (crit)

Crit (https://crit.md) is a separate installed plugin (`crit@crit`) that runs a browser-based inline-comment review loop. Defensive check first: if the `crit` binary isn't on `PATH` (`command -v crit`), print the install command (`claude plugin marketplace add tomasz-tomczyk/crit && claude plugin install crit@crit`) and halt — there's no fallback path to maintain here.

Drive this stage using crit's own `/crit` skill protocol (read `crit`'s installed skill for the authoritative step-by-step — this summarizes it for the heist context):

1. Launch `crit .heist/<slug>/blueprint.md` **in the background** (`run_in_background: true`). It prints a review URL on startup (or connects to an already-running daemon from earlier in this session).
2. Relay the URL to the human verbatim: *"Crit is open at \<url\>. Leave inline comments, then click Finish Review."* Then wait for the background task to finish — don't ask the human to type anything, don't read the review file early, don't poll.
3. When it completes, read stdout for the review comments (same schema as `crit comments --json`) and check stderr for `approved: true`/`false`.
4. **Zero comments (`approved: true`)**: this is approval. Stage → `"forging"`, `updated` to today. Tell the human the blueprint is approved, then continue into forging below.
5. **Comments exist**: relay them to the Mastermind (see "Talking to the Mastermind after turn 1" above) and ask it to apply them to `blueprint.md`. The Mastermind owns `blueprint.md`; don't Edit it yourself even though `crit`'s own default protocol would have the driving agent do that directly.
6. For each comment, post a reply summarizing what the Mastermind did: `crit comment --reply-to <id> --author 'Claude Code' '<summary>'`. **Never pass `--resolve`** — resolving is the human's call, not yours.
7. Signal completion and start the next round using the command `crit` printed on finish (its live-reload means the human sees the revised blueprint in the browser immediately). Loop back to step 2.
8. Repeat until a round finishes with zero comments. That's approval — go to step 4.

Note: `crit` also supports `crit share <file>` for a shareable URL/QR code if the human asks for one — relay that output verbatim if it comes up, per crit's own skill instructions.

### 5. Forging

The Mastermind's job ends at approval — forging is a fresh, one-shot transformation, not a continuation of its conversation.

1. Spawn `heist:forger` (foreground, one-shot) with `.heist/<slug>/blueprint.md` and `validation.md`. It writes `.heist/<slug>/score.md` directly and replies with a summary: step count, and anything it had to make an implicit call on.
2. Update `state.json`: `stage: "safehouse"`, `score_steps_total` set to the step count the Forger reported, `updated` to today.
3. Report to the human: `score.md` path, step count, and any implicit calls the Forger flagged — worth a quick skim before implementation starts.
4. Continue into safehouse below.

### 6. Safehouse (in pipeline)

Run the setup half of the `heist:safehouse` skill's instructions for `<slug>` (same logic as invoking `/heist:safehouse <slug>` directly — worktree + branch, symlink `.heist/<slug>/` into it, confirm exclude, update `state.json` to `stage: "implementing"`). Get the worktree's absolute path back. Continue into implementing below.

### 7. Implementing (Wheelman + Muscle)

1. Spawn `heist:wheelman` (foreground — you need its final report before cleaning). Its task message must include, as the explicit first instruction, to `cd` into the worktree's absolute path before doing anything else — a subagent's Bash tool starts in the orchestrator's own working directory, not the worktree, so this has to be stated, not assumed. Give it the worktree path, and tell it to read `score.md` and `validation.md` from there (its own copies, already present from safehouse).
2. Nested spawning note: the Wheelman needs `Agent` in its own `tools` frontmatter to spawn Muscle workers — it already has this (see `agents/wheelman.md`).
3. Let the Wheelman run its full per-step loop autonomously (it's already instructed on the mechanics in its own definition — dispatch, verify red-then-green honestly, build, commit, advance `score_step`, fall back to doing a step itself after two Muscle failures). Don't intervene per-step.
4. When it reports done, update `state.json` (single file, symlinked into the worktree): `stage: "cleaning"`, `score_step` at final value, `updated` to today.
5. Report to the human: steps completed, anything the Wheelman had to do itself and why, final build status.
6. Continue into cleaning below.

### 8. Cleaning (The Cleaner)

1. Spawn `heist:cleaner` (foreground) with the worktree's absolute path (same `cd`-first instruction as the Wheelman — state it explicitly), plus `.heist/<slug>/blueprint.md`, `.heist/<slug>/score.md`, and `validation.md`. It runs its full pipeline per its own definition: mergeable → adversarial review → mechanical → docs → getaway.
2. **Mechanical failure (bounced back)**: stage → `"implementing"`. Spawn a fresh `heist:wheelman` in the same worktree with the Cleaner's failure report as its task, telling it to fix the failure and re-verify (not re-run the whole score — just fix what broke). When it reports done, go back to step 1 (re-run the Cleaner from the top — mergeable state may have changed).
3. **Adversarial review lands `critical`**: the Cleaner stops before push per its own instructions. Surface this plainly to the human — findings, risk label, and that nothing has been pushed — and stop. This is a human decision, not yours to make.
4. **Success**: stage → `"done"`, `updated` to today. Report the PR URL, risk label, and any findings from the adversarial review worth a human's attention, even at a passing risk level.

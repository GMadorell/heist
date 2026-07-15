# Heist pipeline (steps 1-8)

This is the full `/heist <description>` pipeline: casing → planning → fence review → human review → forging → safehouse → implementing → cleaning. All stages are wired end to end — run the full pipeline per the steps below.

The pipeline runs stage-to-stage without stopping; the only stage that waits on a human is human review (step 4). Steps below say "continue into X" as a pointer, not a reminder — no need to restate "don't stop" each time.

## Preflight

If `heist` isn't on `PATH`, halt and point the user to the README's install section. Otherwise proceed.

### 1. Casing gate

Run `heist validation check .` at the repo root. If it prints `missing`, invoke the `heist:casing` skill's instructions yourself before continuing (don't ask the human to do it as a separate step — this is the "auto-triggered by /heist when validation.md missing" behavior). If it prints `ok`, proceed directly.

### 2. Planning: relay loop with the Mastermind

1. Spawn `heist:slugger` (foreground, one-shot) with the raw change description the user gave `/heist`. The answer will be a slug, parse it.
2. Run `heist state init <slug>`.
3. Run `heist worktree add <slug>`.
4. Run `heist state set <slug> stage planning`.
5. Spawn `heist:mastermind` (foreground, i.e. `run_in_background: false` on the Agent tool call) with a task message containing: the raw change description, the slug, the worktree's absolute path, and an explicit `cd <worktree-path>` instruction.
6. **The relay loop**: each Mastermind reply is either a structured question or the completion signal. The Mastermind runs in the worktree context via the explicit cd instruction from step 5.
   - **Mandatory routing rule**: every structured question, with no exceptions, gets an `AskUserQuestion` call before you do anything else with it. Never answer on the human's behalf, never treat the `RECOMMENDATION:` line as an implicit answer, never skip the call because the question "seems minor" or "seems obvious" or because you're confident what the human would pick. The Mastermind already did the work of deciding this is a human decision (see its interview protocol) — your only job here is to relay, not to re-judge that call.
   - **Structured question** — it has `QUESTION:`, `OPTIONS:`, `RECOMMENDATION:` lines. Map it onto `AskUserQuestion`:
     - `question` = the QUESTION text
     - `header` = a short (≤12 char) label you invent from the topic
     - `options` = the OPTIONS list, reordered so the recommended option is **first** with `(Recommended)` appended to its label, per `AskUserQuestion`'s own convention
     - Present it to the human, get their answer (they may pick an option or type free text via "Other")
     - Relay the answer back via `SendMessage` to the Mastermind (send the option's label/description or the free text verbatim — don't paraphrase), then wait for its next reply. Loop.
   - **`INTERVIEW_COMPLETE`** — the Mastermind will have written `.heist/<slug>/blueprint.md` itself and replied with a short summary. Run `heist state set <slug> stage fence_review`. Show the human the summary and tell them the blueprint is at `.heist/<slug>/blueprint.md`. **Do not end the Mastermind subagent here** — keep it alive; fence review may need to resume it for a revision.
   - If a reply matches neither shape, treat it as a protocol violation: resume once with a reminder of the expected format; if it happens twice, stop and show the human the raw reply rather than looping forever.
7. Once `stage` is `"fence_review"`, continue into fence review below.

**Talking to the Mastermind after turn 1, in general**: "relay to the Mastermind" means `SendMessage` to the still-alive subagent, if this is the same session it was spawned in. After a session restart there's no live subagent to resume (subagent conversations don't survive it) — spawn a **fresh** `heist:mastermind` with `blueprint.md`'s current content plus whatever needs applying (findings, comments); it doesn't need the old interview transcript to revise a document it can just read. Only the interview itself (turn-by-turn question relay) can't be resumed cross-session, since the questions aren't persisted anywhere.

### 3. Fence review

1. Spawn `heist:fence` (foreground, one-shot — no relay loop for Fence itself) with the worktree's absolute path and an explicit `cd <worktree-path>` instruction in the task message. Read its findings.
2. **No findings above `low`, or Fence explicitly says the blueprint holds up**: Run `heist state set <slug> stage human_review`. Tell the human the blueprint passed contrarian review clean, then continue into human review below.
3. **Findings exist**: relay them to the Mastermind (see "Talking to the Mastermind after turn 1" above) and ask it to revise `blueprint.md`. Run `heist state incr <slug> fence_rounds`.
4. The Mastermind revises and replies with a short summary of what changed, plus any finding it explicitly disagreed with and why.
5. **This is the one auto-revision round — do not send the revised blueprint back to Fence again.** Regardless of whether Fence would still object, move on: Run `heist state set <slug> stage human_review`.
6. Report to the human in one place: Fence's original findings, the Mastermind's revision summary, and any disagreement the Mastermind raised with a Fence finding it chose not to apply. Then continue into human review below.

### 4. Human review (crit)

Crit (https://crit.md) is a separate installed plugin (`crit@crit`) that runs a browser-based inline-comment review loop. Defensive check first: if the `crit` binary isn't on `PATH` (`command -v crit`), print the install command (`claude plugin marketplace add tomasz-tomczyk/crit && claude plugin install crit@crit`) and halt — there's no fallback path to maintain here.

Lean on the installed `crit` skills to understand usage.

Use `crit` tool to review the blueprint found in `<worktree-path>/.heist/<slug>/blueprint.md`. If the human left comments on the review, rely them to the Mastermind, ask it to apply them to `blueprint.md`. Answer the comments by what the Mastermind decided. Repeat until the human left no comments, that means approval. Continue to step 5, forging.

### 5. Forging

The Mastermind's job ends at approval — forging is a fresh, one-shot transformation, not a continuation of its conversation.

1. Spawn `heist:forger` (foreground, one-shot) with the worktree's absolute path and an explicit `cd <worktree-path>` instruction in the task message, so Forger reads `blueprint.md` from the worktree, runs `heist validation resolve <path>` for the effective validation sections, and writes `score.md` there.
2. Run `heist state set <slug> stage safehouse` and `heist state set <slug> score_steps_total <step-count>` where `<step-count>` is the value the Forger reported.
3. Report to the human: `score.md` path, step count, and any implicit calls the Forger flagged — worth a quick skim before implementation starts.
4. Continue into the `implementing` flow below.

### 6. Implementing (Wheelman + Muscle)

1. Spawn `heist:wheelman` (foreground — you need its final report before cleaning). As input, it will receive the task `<slug>`.
2. Let the Wheelman run its full per-step loop autonomously. Don't intervene per-step.
3. When it reports done, run `heist state set <slug> stage cleaning`. Wheelman owns `score_step` live via `heist state incr` throughout implementation — don't re-set it here.
4. Report to the human: steps completed, anything the Wheelman had to do itself and why, final build status.
5. Continue into cleaning below.

### 7. Cleaning (The Cleaner)

1. Spawn `heist:cleaner` (foreground) with the task `<slug>` as input. It runs its full pipeline per its own definition.
2. **Mechanical failure (bounced back)**: Run `heist state set <slug> stage implementing`. Spawn a fresh `heist:wheelman` in the same worktree with the Cleaner's failure report as its task, telling it to fix the failure and re-verify (not re-run the whole score — just fix what broke). When it reports done, go back to step 1 (re-run the Cleaner from the top — mergeable state may have changed).
3. **Adversarial review lands `critical`**: the Cleaner stops before push per its own instructions. Surface this plainly to the human — findings, risk label, and that nothing has been pushed — and stop. This is a human decision, not yours to make.
4. **Success**: Run `heist state set <slug> stage done`. Report the PR URL, risk label, and any findings from the adversarial review worth a human's attention, even at a passing risk level.

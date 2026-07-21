---
name: decide
description: Use when the user is unsure whether a task is worth running through the heist pipeline, or wants a mode recommendation before committing.
argument-hint: "<description of the change>"
---

# /heist:decide

Cheap triage: is this task worth a heist, and if so, which mode? No subagents: main-thread judgment only. Never take action beyond this skill's own report. Don't do side effects. This skill's job is just to make a decision, not to implement it.

## Judge the task

Read the description (and skim relevant files/dirs if it's ambiguous from prose alone, a quick `grep`/`glob`, not a full Explore agent). Weigh:

- **Blast radius**: one file/function vs multiple modules vs cross-cutting (API contracts, shared state, migrations).
- **Reversibility**: easy to revert/redo if wrong, or does it touch data, external contracts, security, or things hard to walk back.
- **Ambiguity**: is the "right" implementation obvious, or are there real design decisions/tradeoffs to work through first.
- **Novelty**: routine pattern already used elsewhere in this codebase, or new territory.
- **Cost of being wrong**: silent bug vs loud failure; low-traffic path vs hot path.

## Verdict

Report in this shape, terse:

```
Verdict: <worth it | not worth it | borderline>
Why: <1-3 bullets, the deciding factors above, only the ones that actually moved the needle>
```

Then one of:

- **Worth it**: recommend a mode (`heavy`/`medium`/`light`, same criteria `pipeline.md` step 2 mode-selection uses) with a one-line reason, and give the exact copy-pasteable command: `/heist:heist <mode> <description>`.
- **Not worth it**: say so and suggest the lighter path instead: implement directly (no heist), or if it's genuinely trivial, a one-line diff. Don't hedge into recommending heist "just in case."
- **Borderline**: name the one or two factors driving the uncertainty, recommend `light` mode as the low-cost way to find out (plan + human review, direct implementation, manual crit: cheapest heist has to offer), or ask the human directly if it's their call to make.

Never run the interview yourself, and don't half-start a blueprint. This skill only produces a recommendation: starting the heist (if the human agrees) is a separate, explicit `/heist:heist` invocation.

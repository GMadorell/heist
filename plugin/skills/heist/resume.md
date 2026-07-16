# Heist resume

- One active row: run `heist resume <slug>`. It reports `stage`, `mode`, `worktree` — report this to the human.
- More than one: show the human the `heist list` output, ask which slug to resume. Only one heist runs per orchestrator session.

Once a slug's `stage`/`mode` are known, route by the table below and jump into that step.

| stage | mode | file | step |
|---|---|---|---|
| casing | any | `pipeline.md` | 1 |
| planning | any | `pipeline.md` | 2 |
| fence_review | heavy | `pipeline.md` | 3 |
| human_review | any | `pipeline.md` | 4 |
| forging | heavy/medium | `pipeline-standard.md` | 5 |
| safehouse | heavy/medium | `pipeline-standard.md` | 6 |
| implementing | heavy/medium | `pipeline-standard.md` | 6 |
| implementing | light | `pipeline-light.md` | 2 |
| cleaning | heavy/medium | `pipeline-standard.md` | 7 |
| cleaning | light | `pipeline-light.md` | 3 |

`fence_review`, `forging`, `safehouse` don't occur in `light` mode. `implementing`/`cleaning` are shared stage names — `mode` disambiguates which tail file to read.

If `stage` is `planning` and this heist was started from a plan file (plan mode): plan file paths and prose aren't persisted in state. When `pipeline.md` step 2b tells you to resume, if `.heist/<slug>/blueprint.md` already exists, resume normally by respawning `heist:mastermind` with its current content. If it doesn't exist yet, the import never finished — tell the human to re-invoke `/heist:heist` with the same plan file(s) instead of resuming.

Read only the file the table points to before continuing.

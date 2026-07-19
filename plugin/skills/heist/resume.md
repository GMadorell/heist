# Heist resume

- One active row: run `heist resume <slug>`. Read the file and step its `next:` line names, and jump there.
- More than one: run `heist list`, ask the human which slug to resume. Only one heist runs per orchestrator session.

If `stage` is `planning` and this heist was started from a plan file (plan-based heist): plan file paths and prose aren't persisted in state. When `pipeline.md` step 2b tells you to resume, if `.heist/<slug>/blueprint.md` already exists, resume normally by respawning `heist:mastermind` with its current content. If it doesn't exist yet, the import never finished: tell the human to re-invoke `/heist:heist` with the same plan file(s) instead of resuming.

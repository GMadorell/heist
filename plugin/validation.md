# Validation

Scope: `plugin/` — the Claude Code plugin (markdown skill/agent/template files plus JSON manifests). No build step.

## Build
None. `plugin/` is markdown skill/agent definitions plus JSON manifests (`.claude-plugin/marketplace.json` at the repo root, `plugin/.claude-plugin/plugin.json`). No compile step.

## Lint
None configured for markdown. Sanity-check any edited JSON parses (e.g. `jq . <file>`).

## Test
No automated test suite (no CI, no test scripts). Validate changes by reading the edited skill/agent/template files for internal consistency — cross-references between `plugin/skills/heist/SKILL.md`, `pipeline.md`, `pipeline-standard.md`, `pipeline-light.md`, `resume.md`, and the `heist` commands they call must stay in sync. The state schema is owned by the CLI; run `heist state schema` to check it. Where practical, walk through the `/heist:heist` flow manually.

## Docs
`README.md` (at the repo root) documents the pipeline (including a mermaid diagram), the terms table, and the `plugin/`+`cli/` layout — keep it in sync with `plugin/skills/heist/pipeline.md`/`pipeline-standard.md`/`pipeline-light.md` when stage names, order, or agent responsibilities change, and with `cli/src/main.rs`'s subcommand surface when `heist` commands change.

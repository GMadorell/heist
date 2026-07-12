# Validation

## Build
None — this is a Claude Code plugin: markdown skill/agent definitions plus JSON manifests (`.claude-plugin/plugin.json`, `.claude-plugin/marketplace.json`, `templates/state.json`). No compile step.

## Lint
None. Sanity-check any edited JSON parses (e.g. `jq . <file>`); no linter configured for markdown.

## Test
No automated test suite (no CI, no test scripts). Validate changes by reading the edited skill/agent/template files for internal consistency (cross-references between `pipeline.md`, `resume-by-stage.md`, `SKILL.md` files, and `templates/state.json` must stay in sync) and, where practical, by walking through the `/heist:heist` flow manually.

## Docs
`README.md` documents the pipeline (including a mermaid diagram) and the terms table — keep it in sync with `skills/heist/pipeline.md` when stage names, order, or agent responsibilities change.

## PR conventions
- Main branch: `main`
- Commit style: short, lowercase, imperative summary (e.g. "add mit license", "publish first version of the flow"); no prefix convention (no `feat:`/`fix:`) observed.
- No PR template found.

## Notes
No CI configured (no `.github/workflows/`). No flaky tests or required env vars — there's no test suite to be flaky.

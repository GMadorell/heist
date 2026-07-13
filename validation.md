# Validation

Repo root scope. This is a monorepo split into `plugin/` (the Claude Code plugin, markdown/JSON, no build step) and `cli/` (the Rust crate `heist-cli`). `cli/validation.md` overrides `## Build`/`## Lint`/`## Test` below for anything under `cli/` via nested-validation whole-section-replace (resolved by `heist-cli validation resolve`); the sections below apply as-is to everything else, including `plugin/`.

## Build
None for `plugin/` — markdown skill/agent definitions plus JSON manifests (`.claude-plugin/plugin.json`, `plugin/.claude-plugin/plugin.json`). No compile step. (`cli/` has its own build step — see `cli/validation.md`.)

## Lint
None for `plugin/`. Sanity-check any edited JSON parses (e.g. `jq . <file>`); no linter configured for markdown. (`cli/` has its own lint step — see `cli/validation.md`.)

## Test
No automated test suite for `plugin/` (no CI, no test scripts). Validate changes by reading the edited skill/agent/template files for internal consistency (cross-references between `plugin/skills/heist/pipeline.md`, `plugin/skills/heist/SKILL.md`, and the `heist-cli` commands they call must stay in sync — state schema is owned by the CLI, run `heist-cli state schema` to check it) and, where practical, by walking through the `/heist:heist` flow manually. (`cli/` has its own automated test suite — see `cli/validation.md`.)

## Docs
`README.md` documents the pipeline (including a mermaid diagram), the terms table, and the `plugin/`+`cli/` layout — keep it in sync with `plugin/skills/heist/pipeline.md` when stage names, order, or agent responsibilities change, and with `cli/src/main.rs`'s subcommand surface when `heist-cli` commands change.

## PR conventions
- Main branch: `main`
- Commit style: short, lowercase, imperative summary (e.g. "add mit license", "publish first version of the flow"); no prefix convention (no `feat:`/`fix:`) observed.
- No PR template found.

## Notes
No CI configured (no `.github/workflows/`) for either `plugin/` or `cli/` — tests run locally. `plugin/` has no automated test suite so nothing there can be flaky. `cli/`'s test suite has its own flakiness/env-var notes in `cli/validation.md`. `heist-cli` is a hard runtime dependency of the pipeline (installed via `cargo install --git`, see `README.md`); a missing binary halts `/heist` at the preflight check rather than degrading gracefully.

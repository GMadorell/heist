# Validation

Repo root scope. This is a monorepo split into `plugin/` (the Claude Code plugin, markdown/JSON, no build step) and `cli/` (the Rust crate `heist-cli`). Build/Lint/Test/Docs are defined by the nested `validation.md` in each of those directories and resolved per path by `heist-cli validation resolve` (nested-validation whole-section-replace, nearest file wins per section). This root file carries only repo-global conventions; every real source path lives under `plugin/` or `cli/`, which supply the required Build/Lint/Test sections.

## PR conventions
- Main branch: `main`
- Commit style: short, lowercase, imperative summary (e.g. "add mit license", "publish first version of the flow"); no prefix convention (no `feat:`/`fix:`) observed.
- No PR template found.

## Notes
No CI configured (no `.github/workflows/`) for either `plugin/` or `cli/` — tests run locally. `plugin/` has no automated test suite so nothing there can be flaky. `cli/`'s test suite has its own flakiness/env-var notes in `cli/validation.md`. `heist-cli` is a hard runtime dependency of the pipeline; a missing binary halts `/heist` at the preflight check rather than degrading gracefully (see `README.md` for install).

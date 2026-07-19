# Validation

## PR conventions
- Main branch: `main`
- Commit style: short, lowercase, imperative summary (e.g. "add mit license", "publish first version of the flow"); no prefix convention (no `feat:`/`fix:`) observed.
- No PR template found.

## Notes
No CI configured anywhere in this repo (no `.github/workflows/`). `heist` is a hard runtime dependency of the pipeline; a missing binary halts `/heist` at the preflight check rather than degrading gracefully (see `README.md` for install).

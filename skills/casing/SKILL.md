---
name: casing
description: Use when asked to case/scout a repo's build/lint/test/PR conventions, or to write/refresh validation.md.
---

# /heist:casing

Scout this repo's conventions once; write `validation.md` so Wheelman/Cleaner never re-derive them. Keep it short — loaded on every run, cost compounds.

## Discover

Read manifests/lockfiles/CI/scripts directly — never guess stack from project name.

1. **Build**: manifest (`package.json`, `Cargo.toml`, `pyproject.toml`, `go.mod`, `pom.xml`...), `Makefile`/`justfile`, existing scripts → exact build command.
2. **Test**: exact full-suite command AND exact single-file/single-test command (Muscle runs one test at a time). Prefer existing manifest scripts.
3. **Lint/format**: exact commands, lint-check and format separately if distinct.
4. **Docs**: note existing convention (docs/, doc comments, changelog) — don't prescribe one if none exists.
5. **CI**: `.github/workflows/`, `.gitlab-ci.yml`, etc. — ground truth for exact invocations when present.
6. **Main branch**: `git branch --show-current` or remote default.
7. **PR conventions**: `git log --oneline -20` for commit style; check for PR template.
8. **Quirks**: flaky tests, slow suites, required env/services — only with concrete evidence (comment, skip annotation, README), never speculate.

## Output

Write `validation.md` at repo root — exact commands, no prose padding:

```markdown
# Validation

## Build
<exact command>

## Lint
<exact command(s) — lint-check and format, if distinct>

## Test
- All: <exact command>
- Single file: <exact command, with placeholder for path>

## Docs
<conventions and/or commands for keeping docs in sync — omit if none>

## PR conventions
- Main branch: <name>
- Commit style: <convention observed>
- <PR template location, if any>

## Notes
<quirks: flaky tests, slow suites, required env vars — omit section if none found>
```

If `validation.md` exists, refresh: re-verify each section, update stale parts, report what changed — don't start over.

Unverifiable command (no scripts, no CI, ambiguous manifest) → say so in `## Notes`, never invent a plausible-looking one; a wrong command silently breaks every later stage.

**Never mutate working tree to verify a command** — no `git stash`/`checkout`/discarding uncommitted changes, even temporarily. If uncommitted changes block clean verification, note it and verify against last commit by reading only.

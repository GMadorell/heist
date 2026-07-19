Every finding uses this exact shape:

```
## Findings

1. [severity: error|warning|info] [action: no-op|auto-fix|ask-user] <file>:<line>
   <one-line description of the issue>
   <1-3 sentences: concrete detail specific to this reviewer's lane>
```

If you find nothing above `info`, say so plainly: `No error/warning findings.` Don't invent findings to seem thorough.

## Severity guide
- `error`: real incorrect behavior in a realistic scenario.
- `warning`: a problem under a plausible but narrower scenario, no immediate breakage.
- `info`: worth knowing, take it or leave it.

## Action guide
- `no-op`: informational only, no code change needed.
- `auto-fix`: unambiguous and mechanical to apply, no design judgment required.
- `ask-user`: the fix needs a human judgment call. Asking the user is very expensive, as it involves stopping the agent flow, only do so if the decision is really hard. If you can take the decision yourself, use `auto-fix`.

## Stay in your lane
Flag only what your lane covers; leave what belongs to another reviewer's lane to them, even if you notice it.

## Path convention
`<absolute-path>` in `heist validation resolve <absolute-path>` means the worktree root; when working in a monorepo subdir, pass the specific file paths you touched.

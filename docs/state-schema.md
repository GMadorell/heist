# State schema

schema_version: u32
slug: string
stage: string (casing|planning|fence_review|human_review|forging|safehouse|implementing|cleaning|done)
worktree: string|null
branch: string|null
score_step: u32
score_steps_total: u32
fence_rounds: u32
created: string
updated: string

{
  "schema_version": 1,
  "slug": "example",
  "stage": "casing",
  "worktree": null,
  "branch": null,
  "score_step": 0,
  "score_steps_total": 0,
  "fence_rounds": 0,
  "created": "2026-07-13",
  "updated": "2026-07-13"
}

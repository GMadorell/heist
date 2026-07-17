use assert_cmd::Command;

#[test]
fn prints_field_list_and_example() {
    // Run heist state schema
    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .arg("state")
        .arg("schema")
        .output()
        .expect("failed to run command");

    assert!(
        output.status.success(),
        "command should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // The example's created/updated dates are a fixed constant (not "today"),
    // so the schema output is fully deterministic.
    let expected = r#"schema_version: u32
slug: string
stage: string (casing|planning|fence_review|human_review|forging|safehouse|implementing|cleaning|done)
mode: string (heavy|medium|light), defaults to heavy if unset
worktree: string|null
branch: string|null
base: string|null
score_wave: u32
score_waves_total: u32
score_steps_total: u32
fence_rounds: u32
created: string
updated: string

{
  "schema_version": 1,
  "slug": "example",
  "stage": "casing",
  "mode": "heavy",
  "worktree": null,
  "branch": null,
  "base": null,
  "score_wave": 0,
  "score_waves_total": 0,
  "score_steps_total": 0,
  "fence_rounds": 0,
  "created": "2026-01-01",
  "updated": "2026-01-01"
}"#;

    assert_eq!(
        stdout.trim(),
        expected.trim(),
        "stdout should match expected golden text"
    );
}

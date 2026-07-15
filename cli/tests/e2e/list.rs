use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn write_state(temp_path: &std::path::Path, slug: &str, stage: &str, worktree: &str) {
    fs::create_dir_all(temp_path.join(".heist").join(slug))
        .expect("failed to create .heist/<slug> directory");

    let state_json = format!(
        r#"{{
  "schema_version": 1,
  "slug": "{slug}",
  "stage": "{stage}",
  "worktree": {worktree},
  "branch": null,
  "score_step": 0,
  "score_steps_total": 0,
  "fence_rounds": 0,
  "created": "2024-01-01",
  "updated": "2024-01-01"
}}"#
    );

    fs::write(
        temp_path.join(".heist").join(slug).join("state.json"),
        state_json,
    )
    .expect("failed to write state.json");
}

#[test]
fn empty_dot_heist_prints_nothing() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(temp_dir.path())
        .arg("list")
        .output()
        .expect("failed to run list command");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
}

#[test]
fn missing_dot_heist_prints_nothing() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(temp_dir.path())
        .arg("list")
        .output()
        .expect("failed to run list command");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
}

#[test]
fn single_active_heist_is_listed() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    write_state(
        temp_dir.path(),
        "my-slug",
        "forging",
        "\"/abs/path/to/worktree\"",
    );

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(temp_dir.path())
        .arg("list")
        .output()
        .expect("failed to run list command");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "my-slug  forging  forging  /abs/path/to/worktree\n"
    );
}

#[test]
fn multiple_active_heists_are_listed_sorted_by_slug() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    write_state(temp_dir.path(), "zeta", "planning", "null");
    write_state(temp_dir.path(), "alpha", "casing", "null");

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(temp_dir.path())
        .arg("list")
        .output()
        .expect("failed to run list command");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "alpha  casing  casing  none\nzeta  planning  planning  none\n"
    );
}

#[test]
fn done_heist_is_listed_with_no_next_step() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    write_state(temp_dir.path(), "finished", "done", "null");

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(temp_dir.path())
        .arg("list")
        .output()
        .expect("failed to run list command");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "finished  done  none  none\n"
    );
}

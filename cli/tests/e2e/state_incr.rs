use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn write_fixture(temp_path: &std::path::Path, stage: &str) {
    fs::create_dir_all(temp_path.join(".heist/my-slug")).expect("failed to create state directory");

    let state_json = format!(
        r#"{{
  "schema_version": 1,
  "slug": "my-slug",
  "stage": "{}",
  "worktree": null,
  "branch": null,
  "score_step": 0,
  "score_steps_total": 0,
  "fence_rounds": 0,
  "created": "2026-07-13",
  "updated": "2026-07-13"
}}"#,
        stage
    );
    fs::write(temp_path.join(".heist/my-slug/state.json"), state_json)
        .expect("failed to write state.json");
}

fn run_incr(temp_path: &std::path::Path, field: &str) -> std::process::Output {
    Command::cargo_bin("heist")
        .expect("failed to get cargo bin")
        .current_dir(temp_path)
        .arg("state")
        .arg("incr")
        .arg("my-slug")
        .arg(field)
        .output()
        .expect("failed to run command")
}

fn read_state(temp_path: &std::path::Path) -> serde_json::Value {
    let content = fs::read_to_string(temp_path.join(".heist/my-slug/state.json"))
        .expect("failed to read state.json");
    serde_json::from_str(&content).expect("failed to parse state.json")
}

fn get_today_date() -> String {
    let output = std::process::Command::new("date")
        .args(["-u", "+%Y-%m-%d"])
        .output()
        .expect("failed to get date");
    String::from_utf8(output.stdout)
        .expect("invalid utf8 from date command")
        .trim()
        .to_string()
}

#[test]
fn increments_score_step_twice_and_bumps_updated() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    let temp_path = temp_dir.path();
    write_fixture(temp_path, "implementing");
    let today = get_today_date();

    let first = run_incr(temp_path, "score_step");
    assert!(
        first.status.success(),
        "expected success, got {:?}, stderr: {}",
        first.status,
        String::from_utf8_lossy(&first.stderr)
    );
    let state = read_state(temp_path);
    assert_eq!(
        state["score_step"], 1,
        "score_step should be 1 after first incr"
    );
    assert_eq!(
        state["updated"], today,
        "updated should be today after first incr"
    );

    let second = run_incr(temp_path, "score_step");
    assert!(
        second.status.success(),
        "expected success, got {:?}, stderr: {}",
        second.status,
        String::from_utf8_lossy(&second.stderr)
    );
    let state = read_state(temp_path);
    assert_eq!(
        state["score_step"], 2,
        "score_step should be 2 after second incr"
    );
}

#[test]
fn increments_fence_rounds() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    let temp_path = temp_dir.path();
    write_fixture(temp_path, "fence_review");

    let output = run_incr(temp_path, "fence_rounds");
    assert!(
        output.status.success(),
        "expected success, got {:?}, stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let state = read_state(temp_path);
    assert_eq!(
        state["fence_rounds"], 1,
        "fence_rounds should be 1 after incr"
    );
}

#[test]
fn rejects_non_numeric_field_stage() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    let temp_path = temp_dir.path();
    write_fixture(temp_path, "planning");
    let before = fs::read_to_string(temp_path.join(".heist/my-slug/state.json"))
        .expect("failed to read fixture back");

    let output = run_incr(temp_path, "stage");
    assert_eq!(
        output.status.code(),
        Some(2),
        "expected exit code 2, got {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("stage"),
        "stderr should contain 'stage', got: {:?}",
        stderr
    );

    let after = fs::read_to_string(temp_path.join(".heist/my-slug/state.json"))
        .expect("failed to read state.json");
    assert_eq!(before, after, "state.json should be unchanged on rejection");
}

#[test]
fn rejects_non_numeric_field_slug() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    let temp_path = temp_dir.path();
    write_fixture(temp_path, "planning");

    let output = run_incr(temp_path, "slug");
    assert_eq!(
        output.status.code(),
        Some(2),
        "expected exit code 2, got {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("slug"),
        "stderr should contain 'slug', got: {:?}",
        stderr
    );
}

#[test]
fn rejects_unknown_field() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    let temp_path = temp_dir.path();
    write_fixture(temp_path, "planning");

    let output = run_incr(temp_path, "bogus_field");
    assert_eq!(
        output.status.code(),
        Some(2),
        "expected exit code 2, got {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("bogus_field"),
        "stderr should contain 'bogus_field', got: {:?}",
        stderr
    );
}

#[test]
fn rejects_missing_slug() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    let temp_path = temp_dir.path();

    let output = run_incr(temp_path, "score_step");
    assert_eq!(
        output.status.code(),
        Some(2),
        "expected exit code 2, got {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("my-slug"),
        "stderr should contain 'my-slug', got: {:?}",
        stderr
    );
}

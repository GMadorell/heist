use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn write_fixture(temp_path: &std::path::Path, stage: &str, schema_version: u32) {
    fs::create_dir_all(temp_path.join(".heist/my-slug")).expect("failed to create state directory");

    let state_json = format!(
        r#"{{
  "schema_version": {},
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
        schema_version, stage
    );
    fs::write(temp_path.join(".heist/my-slug/state.json"), state_json)
        .expect("failed to write state.json");
}

#[test]
fn updates_field_and_bumps_updated() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    let temp_path = temp_dir.path();
    write_fixture(temp_path, "planning", 1);

    let today = get_today_date();

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(temp_path)
        .arg("state")
        .arg("set")
        .arg("my-slug")
        .arg("stage")
        .arg("fence_review")
        .output()
        .expect("failed to run command");

    assert!(
        output.status.success(),
        "expected success, got {:?}, stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let content = fs::read_to_string(temp_path.join(".heist/my-slug/state.json"))
        .expect("failed to read state.json");
    let state: serde_json::Value =
        serde_json::from_str(&content).expect("failed to parse state.json");

    assert_eq!(
        state["stage"], "fence_review",
        "stage should be 'fence_review', got: {}",
        state["stage"]
    );
    assert_eq!(
        state["updated"], today,
        "updated should be today's date ({}), got: {}",
        today, state["updated"]
    );

    // Everything but stage/updated should be untouched by this call.
    assert_eq!(
        state["schema_version"], 1,
        "schema_version should be unchanged"
    );
    assert_eq!(state["slug"], "my-slug", "slug should be unchanged");
    assert_eq!(
        state["worktree"],
        serde_json::Value::Null,
        "worktree should be unchanged"
    );
    assert_eq!(
        state["branch"],
        serde_json::Value::Null,
        "branch should be unchanged"
    );
    assert_eq!(state["score_step"], 0, "score_step should be unchanged");
    assert_eq!(
        state["score_steps_total"], 0,
        "score_steps_total should be unchanged"
    );
    assert_eq!(state["fence_rounds"], 0, "fence_rounds should be unchanged");
    assert_eq!(
        state["created"], "2026-07-13",
        "created should be unchanged"
    );
}

#[test]
fn numeric_field_is_stored_as_a_number() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    let temp_path = temp_dir.path();
    write_fixture(temp_path, "implementing", 1);

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(temp_path)
        .arg("state")
        .arg("set")
        .arg("my-slug")
        .arg("score_step")
        .arg("5")
        .output()
        .expect("failed to run command");

    assert!(
        output.status.success(),
        "expected success, got {:?}, stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let content = fs::read_to_string(temp_path.join(".heist/my-slug/state.json"))
        .expect("failed to read state.json");
    let state: serde_json::Value =
        serde_json::from_str(&content).expect("failed to parse state.json");

    assert_eq!(
        state["score_step"],
        serde_json::Value::Number(5.into()),
        "score_step should be stored as a JSON number, got: {}",
        state["score_step"]
    );
}

#[test]
fn rejects_unknown_field() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    let temp_path = temp_dir.path();
    write_fixture(temp_path, "planning", 1);

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(temp_path)
        .arg("state")
        .arg("set")
        .arg("my-slug")
        .arg("bogus_field")
        .arg("x")
        .output()
        .expect("failed to run command");

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
fn rejects_schema_version_mismatch() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    let temp_path = temp_dir.path();
    write_fixture(temp_path, "planning", 99);

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(temp_path)
        .arg("state")
        .arg("set")
        .arg("my-slug")
        .arg("stage")
        .arg("done")
        .output()
        .expect("failed to run command");

    assert_eq!(
        output.status.code(),
        Some(2),
        "expected exit code 2, got {:?}",
        output.status.code()
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("99"),
        "stderr should contain '99', got: {:?}",
        stderr
    );
    assert!(
        stderr.contains("1"),
        "stderr should contain '1', got: {:?}",
        stderr
    );
}

#[test]
fn rejects_invalid_slug() {
    assert_rejects_value("slug", "Not_A_Slug", "invalid value for field 'slug'");
}

#[test]
fn rejects_blank_worktree() {
    assert_rejects_value("worktree", "  ", "invalid value for field 'worktree'");
}

#[test]
fn rejects_blank_branch() {
    assert_rejects_value("branch", "  ", "invalid value for field 'branch'");
}

#[test]
fn rejects_malformed_created_date() {
    assert_rejects_value("created", "07/13/2026", "invalid value for field 'created'");
}

#[test]
fn rejects_malformed_updated_date() {
    assert_rejects_value("updated", "not-a-date", "invalid value for field 'updated'");
}

fn assert_rejects_value(field: &str, value: &str, expected_stderr_substring: &str) {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    let temp_path = temp_dir.path();
    write_fixture(temp_path, "planning", 1);
    let state_json = fs::read_to_string(temp_path.join(".heist/my-slug/state.json"))
        .expect("failed to read fixture back");

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(temp_path)
        .arg("state")
        .arg("set")
        .arg("my-slug")
        .arg(field)
        .arg(value)
        .output()
        .expect("failed to run command");

    assert_eq!(
        output.status.code(),
        Some(2),
        "expected exit code 2, got {:?}",
        output.status.code()
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(expected_stderr_substring),
        "stderr should contain {:?}, got: {:?}",
        expected_stderr_substring,
        stderr
    );

    let content = fs::read_to_string(temp_path.join(".heist/my-slug/state.json"))
        .expect("failed to read state.json");
    assert_eq!(content, state_json, "state.json should be unchanged");
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

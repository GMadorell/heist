use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

#[test]
fn creates_state_json_with_defaults() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    let temp_path = temp_dir.path();

    let mut cmd = Command::cargo_bin("heist-cli").expect("failed to get cargo bin");
    let assert = cmd
        .current_dir(temp_path)
        .arg("state")
        .arg("init")
        .arg("my-slug")
        .assert();

    assert.success();

    let state_file = temp_path.join(".heist/my-slug/state.json");
    assert!(
        state_file.exists(),
        "state file should exist at {:?}",
        state_file
    );

    let content = fs::read_to_string(&state_file).expect("failed to read state.json");
    let parsed_state: serde_json::Value =
        serde_json::from_str(&content).expect("failed to parse state.json");

    assert_eq!(parsed_state["slug"], "my-slug", "slug should be 'my-slug'");
    assert_eq!(
        parsed_state["schema_version"], 1,
        "schema_version should be 1"
    );
    assert_eq!(parsed_state["stage"], "casing", "stage should be 'casing'");
    assert_eq!(
        parsed_state["worktree"],
        serde_json::Value::Null,
        "worktree should be null"
    );
    assert_eq!(
        parsed_state["branch"],
        serde_json::Value::Null,
        "branch should be null"
    );
    assert_eq!(parsed_state["score_step"], 0, "score_step should be 0");
    assert_eq!(
        parsed_state["score_steps_total"], 0,
        "score_steps_total should be 0"
    );
    assert_eq!(parsed_state["fence_rounds"], 0, "fence_rounds should be 0");
}

#[test]
fn rejects_existing_slug_directory() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    let temp_path = temp_dir.path();

    fs::create_dir_all(temp_path.join(".heist/my-slug"))
        .expect("failed to create existing slug directory");

    let mut cmd = Command::cargo_bin("heist-cli").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(temp_path)
        .arg("state")
        .arg("init")
        .arg("my-slug")
        .output()
        .expect("failed to run command");

    assert_eq!(output.status.code(), Some(2), "expected exit code 2");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("my-slug"),
        "stderr should contain 'my-slug', got: {}",
        stderr
    );
}

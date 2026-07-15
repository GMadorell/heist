use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn write_fixture(temp_path: &std::path::Path, schema_version: u32) {
    fs::create_dir_all(temp_path.join(".heist/my-slug")).expect("failed to create state directory");

    let state_json = format!(
        r#"{{
  "schema_version": {},
  "slug": "my-slug",
  "stage": "forging",
  "worktree": null,
  "branch": null,
  "score_step": 0,
  "score_steps_total": 0,
  "fence_rounds": 0,
  "created": "2026-07-13",
  "updated": "2026-07-13"
}}"#,
        schema_version
    );
    fs::write(temp_path.join(".heist/my-slug/state.json"), state_json)
        .expect("failed to write state.json");
}

#[test]
fn prints_requested_field() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    let temp_path = temp_dir.path();
    write_fixture(temp_path, 1);

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(temp_path)
        .arg("state")
        .arg("get")
        .arg("my-slug")
        .arg("stage")
        .output()
        .expect("failed to run command");

    assert!(
        output.status.success(),
        "expected success, got {:?}",
        output.status
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout, "forging\n",
        "stdout should be exactly 'forging\\n', got: {:?}",
        stdout
    );
}

#[test]
fn prints_null_for_unset_optional_field() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    let temp_path = temp_dir.path();
    write_fixture(temp_path, 1);

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(temp_path)
        .arg("state")
        .arg("get")
        .arg("my-slug")
        .arg("worktree")
        .output()
        .expect("failed to run command");

    assert!(
        output.status.success(),
        "expected success, got {:?}",
        output.status
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout, "null\n",
        "stdout should be exactly 'null\\n', got: {:?}",
        stdout
    );
}

#[test]
fn missing_state_file_exits_precondition() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    let temp_path = temp_dir.path();

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(temp_path)
        .arg("state")
        .arg("get")
        .arg("my-slug")
        .arg("stage")
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
        stderr.contains("my-slug"),
        "stderr should contain 'my-slug', got: {:?}",
        stderr
    );
}

#[test]
fn rejects_unknown_field() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    let temp_path = temp_dir.path();
    write_fixture(temp_path, 1);

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(temp_path)
        .arg("state")
        .arg("get")
        .arg("my-slug")
        .arg("bogus_field")
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
    write_fixture(temp_path, 99);

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(temp_path)
        .arg("state")
        .arg("get")
        .arg("my-slug")
        .arg("stage")
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

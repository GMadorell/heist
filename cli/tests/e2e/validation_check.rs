use assert_cmd::Command;
use std::fs;
use std::path::Path;
use std::process::Command as StdCommand;
use tempfile::TempDir;

fn run_git(dir: &Path, args: &[&str]) {
    let status = StdCommand::new("git")
        .arg("-c")
        .arg("commit.gpgsign=false")
        .args(args)
        .current_dir(dir)
        .status()
        .expect("failed to run git");
    assert!(status.success(), "git {:?} failed", args);
}

/// Set up a fixture repo with root validation.md.
/// Returns the repo root path.
fn setup_fixture_with_root_validation() -> TempDir {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    let repo_root = temp_dir.path();

    run_git(repo_root, &["init", "-q", "-b", "main"]);
    run_git(repo_root, &["config", "user.email", "test@example.com"]);
    run_git(repo_root, &["config", "user.name", "Test"]);

    let root_validation = r#"# Validation

## Build
root build command

## Lint
root lint config

## Test
root test runner

## Docs
root docs instruction

## PR conventions
root pr convention

## Notes
root notes"#;

    fs::write(repo_root.join("validation.md"), root_validation)
        .expect("failed to write root validation.md");

    fs::create_dir_all(repo_root.join("some/nested"))
        .expect("failed to create some/nested directory");

    fs::write(repo_root.join("some/nested/path.rs"), "// stub")
        .expect("failed to write some/nested/path.rs");

    run_git(repo_root, &["add", "."]);
    run_git(repo_root, &["commit", "-q", "-m", "init"]);

    temp_dir
}

/// Set up a fixture repo with NO validation.md anywhere.
/// Returns the repo root path.
fn setup_fixture_no_validation() -> TempDir {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    let repo_root = temp_dir.path();

    run_git(repo_root, &["init", "-q", "-b", "main"]);
    run_git(repo_root, &["config", "user.email", "test@example.com"]);
    run_git(repo_root, &["config", "user.name", "Test"]);

    fs::create_dir_all(repo_root.join("some/nested"))
        .expect("failed to create some/nested directory");

    fs::write(repo_root.join("some/nested/anything.rs"), "// stub")
        .expect("failed to write some/nested/anything.rs");

    fs::write(repo_root.join("README.md"), "# Test").expect("failed to write README.md");

    run_git(repo_root, &["add", "."]);
    run_git(repo_root, &["commit", "-q", "-m", "init"]);

    temp_dir
}

#[test]
fn reports_ok_when_present() {
    let temp_dir = setup_fixture_with_root_validation();
    let repo_root = temp_dir.path();

    let mut cmd = Command::cargo_bin("heist-cli").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(repo_root)
        .arg("validation")
        .arg("check")
        .arg("some/nested/path.rs")
        .output()
        .expect("failed to run validation check");

    assert_eq!(
        output.status.code(),
        Some(0),
        "command should exit with 0, got exit code {:?}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        "ok",
        "stdout should be 'ok', got: {}",
        stdout
    );
}

#[test]
fn reports_missing_when_absent() {
    let temp_dir = setup_fixture_no_validation();
    let repo_root = temp_dir.path();

    let mut cmd = Command::cargo_bin("heist-cli").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(repo_root)
        .arg("validation")
        .arg("check")
        .arg("anything.rs")
        .output()
        .expect("failed to run validation check");

    assert_eq!(
        output.status.code(),
        Some(2),
        "command should exit with 2, got exit code {:?}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        "missing",
        "stdout should be 'missing', got: {}",
        stdout
    );
}

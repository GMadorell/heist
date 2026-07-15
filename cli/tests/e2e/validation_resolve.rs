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

/// Set up a fixture repo with root validation.md, cli/validation.md, and plugin/validation.md.
/// Returns the repo root path.
fn setup_fixture() -> TempDir {
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

    fs::create_dir_all(repo_root.join("cli")).expect("failed to create cli directory");

    let cli_validation = r#"## Build
cli build command

## Lint
cli lint config

## Test
cli test runner"#;

    fs::write(repo_root.join("cli/validation.md"), cli_validation)
        .expect("failed to write cli/validation.md");

    fs::create_dir_all(repo_root.join("plugin")).expect("failed to create plugin directory");

    let plugin_validation = r#"## Build
plugin build command

## Lint
plugin lint config

## Test
plugin test runner"#;

    fs::write(repo_root.join("plugin/validation.md"), plugin_validation)
        .expect("failed to write plugin/validation.md");

    fs::create_dir_all(repo_root.join("cli/src")).expect("failed to create cli/src directory");

    fs::write(repo_root.join("cli/src/main.rs"), "// stub")
        .expect("failed to write cli/src/main.rs");

    fs::create_dir_all(repo_root.join("plugin/skills/heist"))
        .expect("failed to create plugin/skills/heist directory");

    fs::write(
        repo_root.join("plugin/skills/heist/pipeline.md"),
        "# pipeline",
    )
    .expect("failed to write plugin/skills/heist/pipeline.md");

    fs::create_dir_all(repo_root.join("cli/tests")).expect("failed to create cli/tests directory");

    fs::write(
        repo_root.join("cli/tests/validation_resolve.rs"),
        "// test file",
    )
    .expect("failed to write cli/tests/validation_resolve.rs");

    run_git(repo_root, &["add", "."]);
    run_git(repo_root, &["commit", "-q", "-m", "init"]);

    temp_dir
}

#[test]
fn single_path_merges_root_and_leaf() {
    let temp_dir = setup_fixture();
    let repo_root = temp_dir.path();

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(repo_root)
        .arg("validation")
        .arg("resolve")
        .arg("cli/src/main.rs")
        .output()
        .expect("failed to run validation resolve");

    assert!(
        output.status.success(),
        "command should succeed, got exit code {:?}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    let expected_golden = r#"## Build
cli build command

## Lint
cli lint config

## Test
cli test runner

## Docs
root docs instruction

## PR conventions
root pr convention

## Notes
root notes
"#;

    assert_eq!(
        stdout.to_string(),
        expected_golden,
        "stdout should match the expected golden string"
    );
}

#[test]
fn multi_path_returns_distinct_scopes() {
    let temp_dir = setup_fixture();
    let repo_root = temp_dir.path();

    // plugin/skills/heist/pipeline.md resolves to the plugin scope,
    // cli/src/main.rs resolves to the cli scope: two distinct labeled blocks expected.
    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(repo_root)
        .arg("validation")
        .arg("resolve")
        .arg("plugin/skills/heist/pipeline.md")
        .arg("cli/src/main.rs")
        .output()
        .expect("failed to run validation resolve");

    assert!(
        output.status.success(),
        "command should succeed, got exit code {:?}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("## Build"), "should contain Build section");
    assert!(
        stdout.contains("plugin build command"),
        "should contain plugin build command"
    );
    assert!(
        stdout.contains("cli build command"),
        "should contain cli build command"
    );

    let lines: Vec<&str> = stdout.lines().collect();
    let build_count = lines
        .iter()
        .filter(|line| line.contains("## Build"))
        .count();
    assert_eq!(
        build_count, 2,
        "should have exactly 2 Build sections (one per scope), got: {}",
        stdout
    );
}

#[test]
fn multi_path_dedupes_same_scope() {
    let temp_dir = setup_fixture();
    let repo_root = temp_dir.path();

    // Both paths resolve to the same cli scope, so the block should be deduped.
    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(repo_root)
        .arg("validation")
        .arg("resolve")
        .arg("cli/src/main.rs")
        .arg("cli/tests/validation_resolve.rs")
        .output()
        .expect("failed to run validation resolve");

    assert!(
        output.status.success(),
        "command should succeed, got exit code {:?}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    let build_count = stdout
        .lines()
        .filter(|line| line.contains("## Build"))
        .count();
    assert_eq!(
        build_count, 1,
        "should have exactly 1 Build section (deduped same scope), got: {}",
        stdout
    );
}

#[test]
fn resolves_using_cwd_not_repo_root_relative_path() {
    let temp_dir = setup_fixture();
    let repo_root = temp_dir.path();

    // Run from cli/ directory with a relative path that only exists relative to cwd
    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(repo_root.join("cli"))
        .arg("validation")
        .arg("resolve")
        .arg("src/main.rs")
        .output()
        .expect("failed to run validation resolve");

    assert!(
        output.status.success(),
        "command should succeed, got exit code {:?}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("cli build command"),
        "stdout should contain 'cli build command', got: {}",
        stdout
    );
}

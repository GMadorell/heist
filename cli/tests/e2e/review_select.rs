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

fn commit_file(dir: &Path, name: &str, content: &str) {
    fs::write(dir.join(name), content).expect("failed to write file");
    run_git(dir, &["add", "."]);
    run_git(dir, &["commit", "-q", "-m", name]);
}

/// Sets up a main repo pushed to a bare origin, with `state init my-slug`
/// and `worktree add my-slug` already run, leaving branch `heist/my-slug`
/// checked out in the worktree at `<main_repo>/.worktrees/my-slug`.
fn setup_repo_with_worktree() -> (TempDir, TempDir) {
    let main_temp = TempDir::new().expect("failed to create main temp dir");
    let main_repo = main_temp.path();

    let bare_temp = TempDir::new().expect("failed to create bare temp dir");
    let bare_repo = bare_temp.path();

    run_git(bare_repo, &["init", "-q", "--bare"]);

    run_git(main_repo, &["init", "-q", "-b", "main"]);
    run_git(main_repo, &["config", "user.email", "test@example.com"]);
    run_git(main_repo, &["config", "user.name", "Test"]);
    run_git(
        main_repo,
        &[
            "remote",
            "add",
            "origin",
            bare_repo.to_string_lossy().as_ref(),
        ],
    );
    fs::write(main_repo.join("README.md"), "hello").expect("failed to write README");
    run_git(main_repo, &["add", "."]);
    run_git(main_repo, &["commit", "-q", "-m", "init"]);
    run_git(main_repo, &["push", "-u", "origin", "main"]);

    let mut init_cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    init_cmd.current_dir(main_repo);
    init_cmd.arg("state").arg("init").arg("my-slug");
    init_cmd.assert().success();

    let mut add_cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    add_cmd.current_dir(main_repo);
    add_cmd.arg("worktree").arg("add").arg("my-slug");
    add_cmd.assert().success();

    (main_temp, bare_temp)
}

#[test]
fn markdown_only_change_selects_intent_quality_simplicity() {
    let (main_temp, _bare_temp) = setup_repo_with_worktree();
    let main_repo = main_temp.path();
    let worktree_path = main_repo.join(".worktrees/my-slug");

    commit_file(&worktree_path, "NOTES.md", "some notes");

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(main_repo)
        .arg("review")
        .arg("select")
        .arg("my-slug")
        .output()
        .expect("failed to run review select");

    assert!(
        output.status.success(),
        "command should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "intent\nquality\nsimplicity\n");
}

#[test]
fn rust_change_selects_all_lanes() {
    let (main_temp, _bare_temp) = setup_repo_with_worktree();
    let main_repo = main_temp.path();
    let worktree_path = main_repo.join(".worktrees/my-slug");

    fs::create_dir_all(worktree_path.join("src")).expect("failed to create src dir");
    commit_file(&worktree_path, "src/lib.rs", "fn main() {}");

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(main_repo)
        .arg("review")
        .arg("select")
        .arg("my-slug")
        .output()
        .expect("failed to run review select");

    assert!(
        output.status.success(),
        "command should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "intent\ncoverage\nquality\nsimplicity\nrust\n");
}

#[test]
fn unresolvable_origin_default_exits_precondition_with_clear_message() {
    let (main_temp, _bare_temp) = setup_repo_with_worktree();
    let main_repo = main_temp.path();

    // Simulate a repo where `origin/<default>` can no longer be resolved
    // (e.g. remote removed, or `refs/remotes/origin/HEAD` unset and the
    // fallback branch name doesn't exist on the remote either).
    run_git(main_repo, &["remote", "remove", "origin"]);

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(main_repo)
        .arg("review")
        .arg("select")
        .arg("my-slug")
        .output()
        .expect("failed to run review select");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("origin's default branch doesn't resolve"),
        "stderr should explain the precondition failure, got: {}",
        stderr
    );
}

#[test]
fn missing_state_exits_precondition() {
    let main_temp = TempDir::new().expect("failed to create main temp dir");
    let main_repo = main_temp.path();
    run_git(main_repo, &["init", "-q", "-b", "main"]);

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(main_repo)
        .arg("review")
        .arg("select")
        .arg("ghost")
        .output()
        .expect("failed to run review select");

    assert_eq!(output.status.code(), Some(2));
}

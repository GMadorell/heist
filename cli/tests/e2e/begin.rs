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

/// Sets up a main repo pushed to a bare origin (no `state init` run — unlike
/// worktree_add.rs's fixture, `begin` owns state creation itself).
/// Returns (main_temp, bare_temp) — both must stay alive for the repo paths to remain valid.
fn setup_repo() -> (TempDir, TempDir) {
    let main_temp = TempDir::new().expect("failed to create main temp dir");
    let main_repo = main_temp.path();
    let bare_temp = TempDir::new().expect("failed to create bare temp dir");
    let bare_repo = bare_temp.path();

    run_git(bare_repo, &["init", "-q", "--bare"]);
    run_git(main_repo, &["init", "-q", "-b", "main"]);
    run_git(main_repo, &["config", "user.email", "test@example.com"]);
    run_git(main_repo, &["config", "user.name", "Test"]);
    let bare_repo_str = bare_repo.to_string_lossy();
    run_git(main_repo, &["remote", "add", "origin", &bare_repo_str]);
    fs::write(main_repo.join("README.md"), "hello").expect("failed to write README");
    run_git(main_repo, &["add", "."]);
    run_git(main_repo, &["commit", "-q", "-m", "init"]);
    run_git(main_repo, &["push", "-u", "origin", "main"]);

    (main_temp, bare_temp)
}

#[test]
fn creates_state_worktree_and_prints_worktree_path() {
    let (main_temp, _bare_temp) = setup_repo();
    let main_repo = main_temp.path();

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(main_repo)
        .arg("begin")
        .arg("my-slug")
        .arg("--mode")
        .arg("heavy")
        .output()
        .expect("failed to run heist begin");

    assert!(
        output.status.success(),
        "begin should succeed, got exit code {:?}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let worktree_path = main_repo.join(".worktrees/my-slug");
    let canonicalized = worktree_path
        .canonicalize()
        .expect("failed to canonicalize worktree path");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.to_string(), format!("{}\n", canonicalized.display()));

    let state_file = main_repo.join(".heist/my-slug/state.json");
    let content = fs::read_to_string(&state_file).expect("failed to read state.json");
    let state: serde_json::Value =
        serde_json::from_str(&content).expect("failed to parse state.json");
    assert_eq!(state["mode"], "heavy");
    assert_eq!(state["stage"], "planning");
    assert!(state["worktree"].as_str().is_some());
    assert!(state["branch"].as_str().is_some());

    let list_output = StdCommand::new("git")
        .args(["worktree", "list"])
        .current_dir(main_repo)
        .output()
        .expect("failed to run git worktree list");
    let list_str = String::from_utf8_lossy(&list_output.stdout);
    assert!(list_str.contains(".worktrees/my-slug"));
    assert!(list_str.contains("heist/my-slug"));
}

#[test]
fn rejects_malformed_slug_with_precondition_exit_code() {
    let (main_temp, _bare_temp) = setup_repo();
    let main_repo = main_temp.path();

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(main_repo)
        .arg("begin")
        .arg("Not A Slug")
        .arg("--mode")
        .arg("heavy")
        .output()
        .expect("failed to run heist begin");

    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn rejects_pre_existing_state_dir_with_dedicated_collision_message() {
    let (main_temp, _bare_temp) = setup_repo();
    let main_repo = main_temp.path();
    fs::create_dir_all(main_repo.join(".heist/my-slug")).expect("failed to create stray state dir");

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(main_repo)
        .arg("begin")
        .arg("my-slug")
        .arg("--mode")
        .arg("heavy")
        .output()
        .expect("failed to run heist begin");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("my-slug"),
        "stderr should name the slug, got: {}",
        stderr
    );
}

#[test]
fn rejects_pre_existing_worktree_without_state_instead_of_adopting_it() {
    let (main_temp, _bare_temp) = setup_repo();
    let main_repo = main_temp.path();
    run_git(
        main_repo,
        &[
            "worktree",
            "add",
            "-b",
            "heist/my-slug",
            ".worktrees/my-slug",
        ],
    );

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(main_repo)
        .arg("begin")
        .arg("my-slug")
        .arg("--mode")
        .arg("heavy")
        .output()
        .expect("failed to run heist begin");

    assert_eq!(output.status.code(), Some(2));
    assert!(!main_repo.join(".heist/my-slug").exists());
}

#[test]
fn rejects_pre_existing_branch_without_state() {
    let (main_temp, _bare_temp) = setup_repo();
    let main_repo = main_temp.path();
    run_git(main_repo, &["branch", "heist/my-slug"]);

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(main_repo)
        .arg("begin")
        .arg("my-slug")
        .arg("--mode")
        .arg("heavy")
        .output()
        .expect("failed to run heist begin");

    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn rolls_back_state_dir_on_worktree_add_failure_and_allows_clean_retry() {
    let (main_temp, _bare_temp) = setup_repo();
    let main_repo = main_temp.path();

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(main_repo)
        .arg("begin")
        .arg("my-slug")
        .arg("--mode")
        .arg("heavy")
        .arg("--base")
        .arg("does-not-exist")
        .output()
        .expect("failed to run heist begin");

    assert_eq!(
        output.status.code(),
        Some(3),
        "bad --base ref should surface as a git error"
    );
    assert!(
        !main_repo.join(".heist/my-slug").exists(),
        "failed begin should roll back the state directory it created"
    );

    let mut retry_cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let retry_output = retry_cmd
        .current_dir(main_repo)
        .arg("begin")
        .arg("my-slug")
        .arg("--mode")
        .arg("heavy")
        .output()
        .expect("failed to run heist begin retry");

    assert!(
        retry_output.status.success(),
        "retry with the same slug should succeed cleanly after rollback, stderr: {}",
        String::from_utf8_lossy(&retry_output.stderr)
    );
}

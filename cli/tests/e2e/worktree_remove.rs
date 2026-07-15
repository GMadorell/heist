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

/// Sets up a main repo pushed to a bare origin, with `state init my-slug` and
/// `worktree add my-slug` already run. Returns (main_temp, bare_temp, worktree_path)
/// — both temp dirs must stay alive for the repo paths to remain valid.
fn setup_repo_with_worktree() -> (TempDir, TempDir, std::path::PathBuf) {
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

    let mut init_cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    init_cmd.current_dir(main_repo);
    init_cmd.arg("state").arg("init").arg("my-slug");
    init_cmd.assert().success();

    let mut add_cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = add_cmd
        .current_dir(main_repo)
        .arg("worktree")
        .arg("add")
        .arg("my-slug")
        .output()
        .expect("failed to run worktree add");
    assert!(
        output.status.success(),
        "worktree add should succeed, got exit code {:?}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let worktree_path = main_repo.join(".worktrees/my-slug");
    assert!(worktree_path.exists(), ".worktrees/my-slug should exist");

    (main_temp, bare_temp, worktree_path)
}

#[test]
fn removes_merged_worktree_and_branch() {
    let (main_temp, _bare_temp, worktree_path) = setup_repo_with_worktree();
    let main_repo = main_temp.path();
    let state_file = main_repo.join(".heist/my-slug/state.json");

    fs::write(worktree_path.join("feature.txt"), "feature work")
        .expect("failed to write feature.txt");
    run_git(&worktree_path, &["add", "."]);
    run_git(&worktree_path, &["commit", "-q", "-m", "add feature"]);
    run_git(&worktree_path, &["push", "-u", "origin", "heist/my-slug"]);

    run_git(main_repo, &["checkout", "main"]);
    run_git(main_repo, &["merge", "--ff-only", "heist/my-slug"]);
    run_git(main_repo, &["push", "origin", "main"]);

    let mut remove_cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = remove_cmd
        .current_dir(main_repo)
        .arg("worktree")
        .arg("remove")
        .arg("my-slug")
        .output()
        .expect("failed to run worktree remove");

    assert!(
        output.status.success(),
        "worktree remove should succeed, got exit code {:?}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let list_output = StdCommand::new("git")
        .args(["worktree", "list"])
        .current_dir(main_repo)
        .output()
        .expect("failed to run git worktree list");
    let list_str = String::from_utf8_lossy(&list_output.stdout);
    assert!(
        !list_str.contains(".worktrees/my-slug"),
        "worktree list should not contain .worktrees/my-slug after removal"
    );

    // Only the local branch is checked here — remote branch deletion is out of
    // scope (see succeeds_when_branch_was_never_pushed_to_origin below), so `-a`
    // would be the wrong assertion.
    let branch_output = StdCommand::new("git")
        .args(["branch"])
        .current_dir(main_repo)
        .output()
        .expect("failed to run git branch");
    let branch_str = String::from_utf8_lossy(&branch_output.stdout);
    assert!(
        !branch_str.contains("heist/my-slug"),
        "local branch list should not contain heist/my-slug after removal"
    );

    assert!(
        state_file.exists(),
        ".heist/my-slug/state.json should still exist after worktree removal"
    );

    let state_content = fs::read_to_string(&state_file).expect("failed to read state.json");
    let state_json: serde_json::Value =
        serde_json::from_str(&state_content).expect("failed to parse state.json");
    assert_eq!(
        state_json["stage"].as_str(),
        Some("done"),
        "stage should be 'done' after worktree removal"
    );
}

#[test]
fn succeeds_when_branch_was_never_pushed_to_origin() {
    // Reproduces the common case (and GitHub's "auto-delete head branches on
    // merge" setting): heist/<slug> is merged locally into main but was never
    // pushed to origin, so origin has no matching ref. worktree remove must
    // not attempt (or fail on) any remote branch deletion — that's out of
    // scope per blueprint.md/score.md step 23, which only calls for
    // `git worktree remove` + `git branch -d`.
    let (main_temp, _bare_temp, worktree_path) = setup_repo_with_worktree();
    let main_repo = main_temp.path();
    let state_file = main_repo.join(".heist/my-slug/state.json");

    fs::write(worktree_path.join("feature.txt"), "feature work")
        .expect("failed to write feature.txt");
    run_git(&worktree_path, &["add", "."]);
    run_git(&worktree_path, &["commit", "-q", "-m", "add feature"]);

    // heist/my-slug is never pushed to origin here (unlike the happy-path test).
    run_git(main_repo, &["checkout", "main"]);
    run_git(main_repo, &["merge", "--ff-only", "heist/my-slug"]);
    run_git(main_repo, &["push", "origin", "main"]);

    let mut remove_cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = remove_cmd
        .current_dir(main_repo)
        .arg("worktree")
        .arg("remove")
        .arg("my-slug")
        .output()
        .expect("failed to run worktree remove");

    assert!(
        output.status.success(),
        "worktree remove should succeed even though heist/my-slug was never pushed \
         to origin, got exit code {:?}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        !worktree_path.exists(),
        ".worktrees/my-slug should be removed"
    );

    let state_content = fs::read_to_string(&state_file).expect("failed to read state.json");
    let state_json: serde_json::Value =
        serde_json::from_str(&state_content).expect("failed to parse state.json");
    assert_eq!(
        state_json["stage"].as_str(),
        Some("done"),
        "stage should be 'done' after worktree removal"
    );
}

#[test]
fn refuses_unmerged_branch() {
    let (main_temp, _bare_temp, worktree_path) = setup_repo_with_worktree();
    let main_repo = main_temp.path();

    fs::write(worktree_path.join("feature.txt"), "feature work")
        .expect("failed to write feature.txt");
    run_git(&worktree_path, &["add", "."]);
    run_git(&worktree_path, &["commit", "-q", "-m", "add feature"]);
    run_git(&worktree_path, &["push", "-u", "origin", "heist/my-slug"]);

    // Key difference from the happy path: heist/my-slug is never merged into main.

    let mut remove_cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = remove_cmd
        .current_dir(main_repo)
        .arg("worktree")
        .arg("remove")
        .arg("my-slug")
        .output()
        .expect("failed to run worktree remove");

    assert_eq!(
        output.status.code(),
        Some(2),
        "worktree remove should exit with code 2, got {:?}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr_str = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr_str.contains("heist/my-slug"),
        "stderr should contain branch name 'heist/my-slug', got: {}",
        stderr_str
    );
    assert!(
        stderr_str.contains("not merged"),
        "stderr should contain 'not merged', got: {}",
        stderr_str
    );

    assert!(
        worktree_path.exists(),
        ".worktrees/my-slug should still exist after refusing to remove unmerged branch"
    );

    let branch_output = StdCommand::new("git")
        .args(["branch", "-a"])
        .current_dir(main_repo)
        .output()
        .expect("failed to run git branch -a");
    let branch_str = String::from_utf8_lossy(&branch_output.stdout);
    assert!(
        branch_str.contains("heist/my-slug"),
        "branch heist/my-slug should still exist after refusing to remove unmerged branch"
    );
}

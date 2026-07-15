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

/// Sets up a main repo pushed to a bare origin, with `state init my-slug` already run.
/// Returns (main_temp, bare_temp) — both must stay alive for the repo paths to remain valid.
fn setup_repo_with_state() -> (TempDir, TempDir) {
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

    (main_temp, bare_temp)
}

#[test]
fn creates_worktree_symlink_and_updates_state() {
    let (main_temp, _bare_temp) = setup_repo_with_state();
    let main_repo = main_temp.path();
    let state_file = main_repo.join(".heist/my-slug/state.json");

    let initial_content = fs::read_to_string(&state_file).expect("failed to read state.json");
    let initial_state: serde_json::Value =
        serde_json::from_str(&initial_content).expect("failed to parse state.json");
    let initial_stage = initial_state["stage"]
        .as_str()
        .expect("stage should be string");

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(main_repo)
        .arg("worktree")
        .arg("add")
        .arg("my-slug")
        .output()
        .expect("failed to run worktree add");

    assert!(
        output.status.success(),
        "command should succeed, got exit code {:?}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let worktree_path = main_repo.join(".worktrees/my-slug");
    let canonicalized_path = worktree_path
        .canonicalize()
        .expect("failed to canonicalize worktree path");
    let expected_output = format!("{}\n", canonicalized_path.display());
    assert_eq!(
        stdout.to_string(),
        expected_output,
        "stdout should be worktree path followed by newline"
    );

    assert!(worktree_path.exists(), ".worktrees/my-slug should exist");

    let list_output = StdCommand::new("git")
        .args(["worktree", "list"])
        .current_dir(main_repo)
        .output()
        .expect("failed to run git worktree list");
    let list_str = String::from_utf8_lossy(&list_output.stdout);
    assert!(
        list_str.contains(".worktrees/my-slug"),
        "worktree list should contain .worktrees/my-slug"
    );
    assert!(
        list_str.contains("heist/my-slug"),
        "worktree list should contain heist/my-slug branch"
    );

    let symlink_path = worktree_path.join(".heist/my-slug");
    assert!(
        symlink_path.exists(),
        ".worktrees/my-slug/.heist/my-slug should exist"
    );

    let symlink_target = fs::read_link(&symlink_path).expect("failed to read symlink");
    let expected_target = main_repo
        .join(".heist/my-slug")
        .canonicalize()
        .expect("failed to canonicalize expected target");
    let actual_target = symlink_target
        .canonicalize()
        .expect("failed to canonicalize actual target");
    assert_eq!(
        actual_target, expected_target,
        "symlink should point to main repo's .heist/my-slug"
    );

    let updated_content =
        fs::read_to_string(&state_file).expect("failed to read updated state.json");
    let updated_state: serde_json::Value =
        serde_json::from_str(&updated_content).expect("failed to parse updated state.json");

    assert!(
        updated_state["worktree"].as_str().is_some(),
        "worktree field should not be null"
    );
    assert!(
        updated_state["branch"].as_str().is_some(),
        "branch field should not be null"
    );

    let get_date_output = StdCommand::new("date")
        .args(["-u", "+%Y-%m-%d"])
        .output()
        .expect("failed to get date");
    let today = String::from_utf8(get_date_output.stdout)
        .expect("invalid utf8")
        .trim()
        .to_string();

    let updated_date = updated_state["updated"]
        .as_str()
        .expect("updated should be string");
    assert_eq!(updated_date, today, "updated field should be today's date");

    let updated_stage = updated_state["stage"]
        .as_str()
        .expect("stage should be string");
    assert_eq!(updated_stage, initial_stage, "stage should not change");
}

#[test]
fn is_idempotent_on_reentry() {
    let (main_temp, _bare_temp) = setup_repo_with_state();
    let main_repo = main_temp.path();

    let mut cmd1 = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output1 = cmd1
        .current_dir(main_repo)
        .arg("worktree")
        .arg("add")
        .arg("my-slug")
        .output()
        .expect("failed to run worktree add");

    assert!(
        output1.status.success(),
        "first worktree add should succeed, got exit code {:?}, stderr: {}",
        output1.status.code(),
        String::from_utf8_lossy(&output1.stderr)
    );

    let stdout1 = String::from_utf8_lossy(&output1.stdout);
    let worktree_path = main_repo.join(".worktrees/my-slug");
    let canonicalized_path = worktree_path
        .canonicalize()
        .expect("failed to canonicalize worktree path");
    let expected_output = format!("{}\n", canonicalized_path.display());

    assert_eq!(
        stdout1.to_string(),
        expected_output,
        "first call stdout should be worktree path followed by newline"
    );

    let symlink_path = worktree_path.join(".heist/my-slug");
    assert!(
        symlink_path.exists(),
        ".worktrees/my-slug/.heist/my-slug should exist after first call"
    );

    let expected_target = main_repo
        .join(".heist/my-slug")
        .canonicalize()
        .expect("failed to canonicalize expected target");

    // Second call must be idempotent: same output, symlink untouched.
    let mut cmd2 = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output2 = cmd2
        .current_dir(main_repo)
        .arg("worktree")
        .arg("add")
        .arg("my-slug")
        .output()
        .expect("failed to run worktree add again");

    assert!(
        output2.status.success(),
        "second worktree add should succeed (idempotent), got exit code {:?}, stderr: {}",
        output2.status.code(),
        String::from_utf8_lossy(&output2.stderr)
    );

    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    assert_eq!(
        stdout2.to_string(),
        expected_output,
        "second call stdout should be same worktree path followed by newline"
    );

    let symlink_target2 =
        fs::read_link(&symlink_path).expect("failed to read symlink after second call");
    let actual_target2 = symlink_target2
        .canonicalize()
        .expect("failed to canonicalize actual target after second call");
    assert_eq!(
        actual_target2, expected_target,
        "symlink should still point to main repo's .heist/my-slug after second call"
    );

    // A missing symlink (e.g. from a fresh checkout) must be recreated, not treated as an error.
    fs::remove_file(&symlink_path).expect("failed to delete symlink");
    assert!(!symlink_path.exists(), "symlink should be deleted");

    let mut cmd3 = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output3 = cmd3
        .current_dir(main_repo)
        .arg("worktree")
        .arg("add")
        .arg("my-slug")
        .output()
        .expect("failed to run worktree add after symlink deletion");

    assert!(
        output3.status.success(),
        "worktree add after symlink deletion should succeed, got exit code {:?}, stderr: {}",
        output3.status.code(),
        String::from_utf8_lossy(&output3.stderr)
    );

    assert!(
        symlink_path.exists(),
        ".worktrees/my-slug/.heist/my-slug should be recreated"
    );

    let symlink_target3 =
        fs::read_link(&symlink_path).expect("failed to read symlink after recreation");
    let actual_target3 = symlink_target3
        .canonicalize()
        .expect("failed to canonicalize actual target after recreation");
    assert_eq!(
        actual_target3, expected_target,
        "symlink should point to main repo's .heist/my-slug after recreation"
    );
}

#[test]
fn branch_conflict_exits_git_error_code() {
    let (main_temp, _bare_temp) = setup_repo_with_state();
    let main_repo = main_temp.path();

    // Pre-create a branch named heist/my-slug (not as a worktree, just a branch)
    // so `worktree add` collides with it instead of creating cleanly.
    run_git(main_repo, &["branch", "heist/my-slug"]);

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(main_repo)
        .arg("worktree")
        .arg("add")
        .arg("my-slug")
        .output()
        .expect("failed to run worktree add");

    assert_eq!(
        output.status.code(),
        Some(3),
        "should exit with code 3 (GIT), got {:?}",
        output.status.code()
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("already-exists"),
        "stderr should contain 'already-exists', got: {}",
        stderr
    );
}

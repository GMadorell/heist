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

/// Sets up a main repo pushed to a bare origin, with `state init <slug>` and
/// `worktree add <slug>` already run. Returns (main_temp, bare_temp, worktree_path).
fn setup_repo_with_worktree(slug: &str) -> (TempDir, TempDir, std::path::PathBuf) {
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
    init_cmd
        .current_dir(main_repo)
        .arg("state")
        .arg("init")
        .arg(slug);
    init_cmd.assert().success();

    let mut add_cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = add_cmd
        .current_dir(main_repo)
        .arg("worktree")
        .arg("add")
        .arg(slug)
        .output()
        .expect("failed to run worktree add");
    assert!(
        output.status.success(),
        "worktree add should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let worktree_path = main_repo.join(".worktrees").join(slug);
    (main_temp, bare_temp, worktree_path)
}

#[test]
fn removes_merged_heist_owned_worktree() {
    let (main_temp, _bare_temp, worktree_path) = setup_repo_with_worktree("my-slug");
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

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(main_repo)
        .arg("worktree")
        .arg("cleanup")
        .output()
        .expect("failed to run worktree cleanup");

    assert!(
        output.status.success(),
        "cleanup should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "removed my-slug");

    assert!(
        !worktree_path.exists(),
        ".worktrees/my-slug should be removed"
    );

    let branch_output = StdCommand::new("git")
        .args(["branch"])
        .current_dir(main_repo)
        .output()
        .expect("failed to run git branch");
    assert!(!String::from_utf8_lossy(&branch_output.stdout).contains("heist/my-slug"));

    let state_content = fs::read_to_string(&state_file).expect("failed to read state.json");
    let state_json: serde_json::Value =
        serde_json::from_str(&state_content).expect("failed to parse state.json");
    assert_eq!(state_json["stage"].as_str(), Some("done"));
}

#[test]
fn skips_unmerged_heist_owned_worktree() {
    let (main_temp, _bare_temp, worktree_path) = setup_repo_with_worktree("my-slug");
    let main_repo = main_temp.path();

    fs::write(worktree_path.join("feature.txt"), "feature work")
        .expect("failed to write feature.txt");
    run_git(&worktree_path, &["add", "."]);
    run_git(&worktree_path, &["commit", "-q", "-m", "add feature"]);
    // heist/my-slug is never merged into main.

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(main_repo)
        .arg("worktree")
        .arg("cleanup")
        .output()
        .expect("failed to run worktree cleanup");

    assert!(
        output.status.success(),
        "cleanup should succeed even when skipping, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "skipped my-slug (unmerged)");
    assert!(
        worktree_path.exists(),
        ".worktrees/my-slug should still exist"
    );
}

#[test]
fn dry_run_mutates_nothing() {
    let (main_temp, _bare_temp, worktree_path) = setup_repo_with_worktree("my-slug");
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

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(main_repo)
        .arg("worktree")
        .arg("cleanup")
        .arg("--dry-run")
        .output()
        .expect("failed to run worktree cleanup --dry-run");

    assert!(
        output.status.success(),
        "dry-run should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "would remove my-slug");

    assert!(
        worktree_path.exists(),
        ".worktrees/my-slug should still exist after dry-run"
    );
    let branch_output = StdCommand::new("git")
        .args(["branch"])
        .current_dir(main_repo)
        .output()
        .expect("failed to run git branch");
    assert!(String::from_utf8_lossy(&branch_output.stdout).contains("heist/my-slug"));

    let state_content = fs::read_to_string(&state_file).expect("failed to read state.json");
    let state_json: serde_json::Value =
        serde_json::from_str(&state_content).expect("failed to parse state.json");
    assert_ne!(state_json["stage"].as_str(), Some("done"));
}

#[test]
fn leaves_non_heist_worktree_untouched() {
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

    // A worktree under .worktrees/ but on a plain (non heist/<slug>) branch.
    let scratch_path = main_repo.join(".worktrees").join("scratch");
    run_git(
        main_repo,
        &[
            "worktree",
            "add",
            scratch_path.to_string_lossy().as_ref(),
            "-b",
            "scratch-branch",
        ],
    );

    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .current_dir(main_repo)
        .arg("worktree")
        .arg("cleanup")
        .output()
        .expect("failed to run worktree cleanup");

    assert!(
        output.status.success(),
        "cleanup should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "");
    assert!(
        scratch_path.exists(),
        "non-heist worktree should be untouched"
    );
}

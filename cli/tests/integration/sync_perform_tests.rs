use heist_cli::adapters::real_git::RealGit;
use heist_cli::app::base::BaseResolution;
use heist_cli::domain::value::NonBlankValue;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn run_git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-c")
        .arg("commit.gpgsign=false")
        .args(args)
        .current_dir(dir)
        .status()
        .expect("failed to run git");
    assert!(status.success(), "git {:?} failed", args);
}

fn init_repo_with_commit(dir: &Path) {
    run_git(dir, &["init", "-q", "-b", "main"]);
    run_git(dir, &["config", "user.email", "test@example.com"]);
    run_git(dir, &["config", "user.name", "Test"]);
    fs::write(dir.join("README.md"), "hello").expect("failed to write file");
    run_git(dir, &["add", "."]);
    run_git(dir, &["commit", "-q", "-m", "init"]);
}

fn commit_file(dir: &Path, name: &str, content: &str) {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create parent directories");
    }
    fs::write(path, content).expect("failed to write file");
    run_git(dir, &["add", "."]);
    run_git(dir, &["commit", "-q", "-m", name]);
}

fn build_stacked_squash_scenario() -> (TempDir, TempDir) {
    let origin_dir = TempDir::new().expect("failed to create temp directory");
    let repo_dir = TempDir::new().expect("failed to create temp directory");
    init_repo_with_commit(origin_dir.path());

    run_git(repo_dir.path(), &["init", "-q", "-b", "main"]);
    run_git(
        repo_dir.path(),
        &[
            "remote",
            "add",
            "origin",
            origin_dir.path().to_string_lossy().as_ref(),
        ],
    );
    run_git(repo_dir.path(), &["fetch", "-q", "origin"]);
    run_git(
        repo_dir.path(),
        &["checkout", "-q", "-b", "main", "origin/main"],
    );

    run_git(repo_dir.path(), &["checkout", "-q", "-b", "heist/piece-01"]);
    commit_file(repo_dir.path(), "a.txt", "v1");
    commit_file(repo_dir.path(), "a.txt", "v2");

    run_git(repo_dir.path(), &["checkout", "-q", "-b", "heist/piece-02"]);
    commit_file(repo_dir.path(), "c.txt", "c");

    fs::write(origin_dir.path().join("a.txt"), "v2").expect("failed to write a.txt");
    run_git(origin_dir.path(), &["add", "."]);
    run_git(
        origin_dir.path(),
        &["commit", "-q", "-m", "squash piece-01: orig -> v2 directly"],
    );

    run_git(repo_dir.path(), &["fetch", "-q", "origin"]);
    run_git(repo_dir.path(), &["checkout", "-q", "heist/piece-02"]);

    (origin_dir, repo_dir)
}

#[test]
fn perform_merges_live_base_cleanly_on_multi_commit_squashed_scenario_where_rebase_would_conflict()
{
    let (_origin_dir, repo_dir) = build_stacked_squash_scenario();

    let resolution = BaseResolution::Live {
        base_ref: NonBlankValue::parse("base", "heist/piece-01").unwrap(),
    };

    let result = heist_cli::app::sync::perform(repo_dir.path(), &RealGit, "main", &resolution);

    assert!(result.is_ok());

    assert!(repo_dir.path().join("a.txt").exists());
    assert!(repo_dir.path().join("c.txt").exists());
    assert_eq!(
        fs::read_to_string(repo_dir.path().join("a.txt")).expect("failed to read a.txt"),
        "v2"
    );

    let status_output = std::process::Command::new("git")
        .arg("-c")
        .arg("commit.gpgsign=false")
        .args(["status", "--porcelain"])
        .current_dir(repo_dir.path())
        .output()
        .expect("failed to run git status");
    assert!(
        status_output.stdout.is_empty(),
        "expected clean working directory, got: {}",
        String::from_utf8_lossy(&status_output.stdout)
    );
}

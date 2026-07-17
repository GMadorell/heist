use heist_cli::adapters::real_git::RealGit;
use heist_cli::ports::git::{GitError, GitRepository, MergeCheck};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn run_git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        // Disable commit signing for these throwaway test repos: if the
        // ambient global git config has commit.gpgsign=true, parallel
        // test threads all invoking gpg-agent concurrently can serialize
        // and occasionally time out, making the test suite flaky.
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

#[test]
fn detects_main_branch_via_git() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    let repo_root = temp_dir.path();

    init_repo_with_commit(repo_root);

    assert_eq!(RealGit.default_branch(repo_root), "main");
}

#[test]
fn reports_branch_merged_when_equal_to_origin_main() {
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
    run_git(repo_dir.path(), &["branch", "feature"]);

    assert_eq!(
        RealGit
            .is_branch_merged(repo_dir.path(), "feature", "main")
            .expect("merge check should succeed"),
        MergeCheck::Merged
    );
}

#[test]
fn reports_branch_merged_when_ancestor_of_origin_main() {
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
    run_git(repo_dir.path(), &["branch", "feature"]);

    // Advance origin/main past `feature` so `feature` becomes a strict
    // ancestor rather than equal to it.
    commit_file(origin_dir.path(), "more.txt", "more");
    run_git(repo_dir.path(), &["fetch", "-q", "origin"]);

    assert_eq!(
        RealGit
            .is_branch_merged(repo_dir.path(), "feature", "main")
            .expect("merge check should succeed"),
        MergeCheck::Merged
    );
}

#[test]
fn reports_branch_unmerged_when_not_ancestor_of_origin_main() {
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
    run_git(repo_dir.path(), &["checkout", "-q", "-b", "feature"]);
    commit_file(repo_dir.path(), "feature.txt", "unmerged");

    assert!(matches!(
        RealGit
            .is_branch_merged(repo_dir.path(), "feature", "main")
            .expect("merge check should succeed"),
        MergeCheck::NotMerged { .. }
    ));
}

#[test]
fn is_branch_merged_errors_on_bad_ref() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    init_repo_with_commit(temp_dir.path());

    let result = RealGit.is_branch_merged(temp_dir.path(), "no-such-branch", "main");
    assert!(result.is_err());
}

#[test]
fn delete_branch_succeeds_when_merged() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    init_repo_with_commit(temp_dir.path());
    run_git(temp_dir.path(), &["branch", "feature"]);

    RealGit
        .delete_branch(temp_dir.path(), "feature")
        .expect("delete should succeed");
}

#[test]
fn delete_branch_fails_when_unmerged() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    init_repo_with_commit(temp_dir.path());
    run_git(temp_dir.path(), &["checkout", "-q", "-b", "feature"]);
    commit_file(temp_dir.path(), "feature.txt", "unmerged");
    run_git(temp_dir.path(), &["checkout", "-q", "main"]);

    let result = RealGit.delete_branch(temp_dir.path(), "feature");
    assert!(result.is_err());
}

#[test]
fn worktree_exists_reflects_added_worktrees() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    init_repo_with_commit(temp_dir.path());
    let worktree_path = temp_dir.path().join("worktrees").join("foo");

    assert!(!RealGit.worktree_exists(temp_dir.path(), "foo"));

    RealGit
        .add_worktree(temp_dir.path(), &worktree_path, "heist/foo", "main")
        .expect("add should succeed");

    assert!(RealGit.worktree_exists(temp_dir.path(), "foo"));
    assert!(!RealGit.worktree_exists(temp_dir.path(), "bar"));
}

#[test]
fn add_worktree_fails_when_path_already_exists() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    init_repo_with_commit(temp_dir.path());
    let worktree_path = temp_dir.path().join("worktrees").join("foo");
    fs::create_dir_all(&worktree_path).expect("failed to create directory");
    fs::write(worktree_path.join("occupied"), "x").expect("failed to write file");

    let result = RealGit.add_worktree(temp_dir.path(), &worktree_path, "heist/foo", "main");

    match result {
        Err(GitError::WorktreeAdd { subtype, .. }) => assert_eq!(subtype, "already-exists"),
        other => panic!("expected WorktreeAdd already-exists error, got {other:?}"),
    }
}

#[test]
fn remove_worktree_removes_added_worktree() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    init_repo_with_commit(temp_dir.path());
    let worktree_path = temp_dir.path().join("worktrees").join("foo");
    RealGit
        .add_worktree(temp_dir.path(), &worktree_path, "heist/foo", "main")
        .expect("add should succeed");

    RealGit
        .remove_worktree(temp_dir.path(), &worktree_path)
        .expect("remove should succeed");

    assert!(!RealGit.worktree_exists(temp_dir.path(), "foo"));
}

#[test]
fn remove_worktree_fails_for_nonexistent_path() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    init_repo_with_commit(temp_dir.path());
    let missing_path = temp_dir.path().join("worktrees").join("missing");

    let result = RealGit.remove_worktree(temp_dir.path(), &missing_path);
    assert!(result.is_err());
}

#[test]
fn list_worktrees_reports_path_and_branch() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    init_repo_with_commit(temp_dir.path());
    let worktree_path = temp_dir.path().join("worktrees").join("foo");
    RealGit
        .add_worktree(temp_dir.path(), &worktree_path, "heist/foo", "main")
        .expect("add should succeed");

    let infos = RealGit
        .list_worktrees(temp_dir.path())
        .expect("list should succeed");

    assert_eq!(infos.len(), 1);
    assert_eq!(
        infos[0].path,
        worktree_path
            .canonicalize()
            .expect("worktree path should exist")
    );
    assert_eq!(infos[0].branch.as_deref(), Some("heist/foo"));
}

#[test]
fn changed_paths_lists_files_changed_since_merge_base() {
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
    run_git(repo_dir.path(), &["checkout", "-q", "-b", "feature"]);
    commit_file(repo_dir.path(), "src/lib.rs", "fn main() {}");
    commit_file(repo_dir.path(), "README.md", "hello\nmore");

    let paths = RealGit
        .changed_paths(repo_dir.path(), "main", "feature")
        .expect("changed_paths should succeed");

    assert_eq!(
        paths,
        vec![PathBuf::from("README.md"), PathBuf::from("src/lib.rs")]
    );
}

#[test]
fn changed_paths_errors_on_unresolvable_base() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    init_repo_with_commit(temp_dir.path());

    let result = RealGit.changed_paths(temp_dir.path(), "no-such-remote-branch", "HEAD");
    assert!(result.is_err());
}

#[test]
fn changed_paths_includes_deleted_files_via_old_file_path() {
    let origin_dir = TempDir::new().expect("failed to create temp directory");
    let repo_dir = TempDir::new().expect("failed to create temp directory");
    init_repo_with_commit(origin_dir.path());
    commit_file(origin_dir.path(), "doomed.rs", "fn gone() {}");

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
    run_git(repo_dir.path(), &["checkout", "-q", "-b", "feature"]);
    run_git(repo_dir.path(), &["rm", "-q", "doomed.rs"]);
    run_git(repo_dir.path(), &["commit", "-q", "-m", "remove doomed.rs"]);

    let paths = RealGit
        .changed_paths(repo_dir.path(), "main", "feature")
        .expect("changed_paths should succeed");

    assert_eq!(paths, vec![PathBuf::from("doomed.rs")]);
}

#[test]
fn resolve_ref_errors_on_missing_ref() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    init_repo_with_commit(temp_dir.path());

    let result = RealGit.resolve_ref(temp_dir.path(), "no-such-ref");
    assert!(result.is_err());
}

#[test]
fn resolve_ref_succeeds_for_existing_branch() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    init_repo_with_commit(temp_dir.path());
    run_git(temp_dir.path(), &["branch", "feature"]);

    let result = RealGit.resolve_ref(temp_dir.path(), "feature");
    assert!(result.is_ok());
}

#[test]
fn is_ancestor_false_when_not_reachable() {
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
    run_git(repo_dir.path(), &["checkout", "-q", "-b", "feature"]);
    commit_file(repo_dir.path(), "feature.txt", "unmerged");

    assert!(!RealGit
        .is_ancestor(repo_dir.path(), "feature", "main")
        .expect("is_ancestor should succeed"));
}

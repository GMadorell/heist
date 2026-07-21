use heist_cli::adapters::real_git::RealGit;
use heist_cli::domain::value::{BranchValue, RefValue, SlugValue};
use heist_cli::ports::git::{GitError, GitRepository, MergeCheck};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn branch(s: &str) -> BranchValue {
    BranchValue::try_from_raw("branch", s).expect("valid branch")
}

fn ref_value(s: &str) -> RefValue {
    RefValue::try_from_raw(s).expect("valid ref")
}

fn slug(s: &str) -> SlugValue {
    SlugValue::parse(s).expect("valid slug")
}

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
fn current_branch_reports_checked_out_branch() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    let repo_root = temp_dir.path();
    init_repo_with_commit(repo_root);
    run_git(repo_root, &["checkout", "-q", "-b", "heist/piece-01"]);

    let branch = RealGit
        .current_branch(repo_root)
        .expect("current_branch should succeed");

    assert_eq!(branch, Some("heist/piece-01".to_string()));
}

#[test]
fn current_branch_reports_none_when_head_detached() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    let repo_root = temp_dir.path();
    init_repo_with_commit(repo_root);
    // Detach HEAD at the current commit.
    run_git(repo_root, &["checkout", "-q", "--detach"]);

    let branch = RealGit
        .current_branch(repo_root)
        .expect("current_branch should succeed");

    assert_eq!(branch, None);
}

#[test]
fn resolve_ref_errors_with_ref_resolve_on_missing_ref() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    let repo_root = temp_dir.path();
    init_repo_with_commit(repo_root);

    let result = RealGit.resolve_ref(repo_root, &ref_value("does-not-exist"));

    match result {
        Err(GitError::RefResolve { ref_spec, .. }) => {
            assert_eq!(ref_spec, "does-not-exist");
        }
        other => panic!("expected RefResolve error, got {:?}", other),
    }
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
            .is_branch_merged(repo_dir.path(), &branch("feature"), "main")
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
            .is_branch_merged(repo_dir.path(), &branch("feature"), "main")
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
            .is_branch_merged(repo_dir.path(), &branch("feature"), "main")
            .expect("merge check should succeed"),
        MergeCheck::NotMerged { .. }
    ));
}

#[test]
fn is_branch_merged_errors_on_bad_ref() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    init_repo_with_commit(temp_dir.path());

    let result = RealGit.is_branch_merged(temp_dir.path(), &branch("no-such-branch"), "main");
    assert!(result.is_err());
}

#[test]
fn delete_branch_succeeds_when_merged() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    init_repo_with_commit(temp_dir.path());
    run_git(temp_dir.path(), &["branch", "feature"]);

    RealGit
        .delete_branch(temp_dir.path(), &branch("feature"))
        .expect("delete should succeed");
}

#[test]
fn delete_branch_fails_when_unmerged() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    init_repo_with_commit(temp_dir.path());
    run_git(temp_dir.path(), &["checkout", "-q", "-b", "feature"]);
    commit_file(temp_dir.path(), "feature.txt", "unmerged");
    run_git(temp_dir.path(), &["checkout", "-q", "main"]);

    let result = RealGit.delete_branch(temp_dir.path(), &branch("feature"));
    assert!(result.is_err());
}

#[test]
fn worktree_exists_reflects_added_worktrees() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    init_repo_with_commit(temp_dir.path());
    let worktree_path = temp_dir.path().join("worktrees").join("foo");

    assert!(!RealGit
        .worktree_exists(temp_dir.path(), &slug("foo"))
        .unwrap());

    RealGit
        .add_worktree(
            temp_dir.path(),
            &worktree_path,
            &branch("heist/foo"),
            &ref_value("main"),
        )
        .expect("add should succeed");

    assert!(RealGit
        .worktree_exists(temp_dir.path(), &slug("foo"))
        .unwrap());
    assert!(!RealGit
        .worktree_exists(temp_dir.path(), &slug("bar"))
        .unwrap());
}

#[test]
fn add_worktree_fails_when_path_already_exists() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    init_repo_with_commit(temp_dir.path());
    let worktree_path = temp_dir.path().join("worktrees").join("foo");
    fs::create_dir_all(&worktree_path).expect("failed to create directory");
    fs::write(worktree_path.join("occupied"), "x").expect("failed to write file");

    let result = RealGit.add_worktree(
        temp_dir.path(),
        &worktree_path,
        &branch("heist/foo"),
        &ref_value("main"),
    );

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
        .add_worktree(
            temp_dir.path(),
            &worktree_path,
            &branch("heist/foo"),
            &ref_value("main"),
        )
        .expect("add should succeed");

    RealGit
        .remove_worktree(temp_dir.path(), &worktree_path)
        .expect("remove should succeed");

    assert!(!RealGit
        .worktree_exists(temp_dir.path(), &slug("foo"))
        .unwrap());
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
        .add_worktree(
            temp_dir.path(),
            &worktree_path,
            &branch("heist/foo"),
            &ref_value("main"),
        )
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
        .changed_paths(repo_dir.path(), "main", &ref_value("feature"))
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

    let result =
        RealGit.changed_paths(temp_dir.path(), "no-such-remote-branch", &ref_value("HEAD"));
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
        .changed_paths(repo_dir.path(), "main", &ref_value("feature"))
        .expect("changed_paths should succeed");

    assert_eq!(paths, vec![PathBuf::from("doomed.rs")]);
}

#[test]
fn resolve_ref_errors_on_missing_ref() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    init_repo_with_commit(temp_dir.path());

    let result = RealGit.resolve_ref(temp_dir.path(), &ref_value("no-such-ref"));
    assert!(result.is_err());
}

#[test]
fn resolve_ref_succeeds_for_existing_branch() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    init_repo_with_commit(temp_dir.path());
    run_git(temp_dir.path(), &["branch", "feature"]);

    let result = RealGit.resolve_ref(temp_dir.path(), &ref_value("feature"));
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
        .is_ancestor(repo_dir.path(), &ref_value("feature"), &ref_value("main"))
        .expect("is_ancestor should succeed"));
}

/// Builds the load-bearing stacked-squash scenario shared by
/// `rebase_conflicts_on_multi_commit_squashed_base` and
/// `merge_succeeds_on_multi_commit_squashed_base_where_rebase_would_conflict`.
///
/// `heist/piece-01` makes *two sequential* commits that each rewrite the same
/// line of `a.txt` (`orig` -> `v1` -> `v2`). `heist/piece-02` forks from
/// `heist/piece-01` and adds its own unrelated `c.txt`. `origin/main` then
/// lands piece-01's net change as a single squash commit (`orig` -> `v2`
/// directly, in one commit, not two).
///
/// This must stay a *two-commit* base, not a one-commit one: a single-commit
/// base's diff is byte-identical to the squash's diff, so git's "patch
/// contents already upstream" empty-commit skip silently (and correctly)
/// drops it during rebase, and the rebase falsely succeeds either way. With
/// two sequential commits touching the same line, replaying the *first* of
/// them lands on a tree that already reflects the *second* commit's result;
/// the patch's expected context (`orig`) no longer exists (the line already
/// reads `v2`), so `git rebase` conflicts even though the net diff is
/// identical. A three-way `git merge` compares final tree states directly
/// (`v2` locally vs. `v2` upstream) and sees no divergence at all.
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
fn rebase_conflicts_on_multi_commit_squashed_base() {
    let (_origin_dir, repo_dir) = build_stacked_squash_scenario();

    let result = RealGit.rebase(repo_dir.path(), &ref_value("origin/main"));
    assert!(result.is_err());
}

#[test]
fn merge_succeeds_on_multi_commit_squashed_base_where_rebase_would_conflict() {
    let (_origin_dir, repo_dir) = build_stacked_squash_scenario();

    let result = RealGit.merge(repo_dir.path(), &ref_value("origin/main"));
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

#[test]
fn branch_exists_true_for_existing_branch_false_otherwise() {
    let temp_dir = TempDir::new().expect("failed to create temp directory");
    let repo_root = temp_dir.path();
    init_repo_with_commit(repo_root);
    run_git(repo_root, &["branch", "heist/foo"]);

    assert!(RealGit
        .branch_exists(repo_root, &branch("heist/foo"))
        .unwrap());
    assert!(!RealGit
        .branch_exists(repo_root, &branch("heist/does-not-exist"))
        .unwrap());
}

/// Builds a scenario where a direct three-way `git merge` (not a rebase)
/// conflicts: both `origin/main` and the local branch touch the same line
/// of the same file in incompatible ways, so there is no clean auto-merge.
fn build_merge_conflict_scenario() -> (TempDir, TempDir) {
    let origin_dir = TempDir::new().expect("failed to create temp directory");
    let repo_dir = TempDir::new().expect("failed to create temp directory");
    init_repo_with_commit(origin_dir.path());
    commit_file(origin_dir.path(), "a.txt", "orig");

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
    commit_file(repo_dir.path(), "a.txt", "feature-value");

    commit_file(origin_dir.path(), "a.txt", "main-value");
    run_git(repo_dir.path(), &["fetch", "-q", "origin"]);
    run_git(repo_dir.path(), &["checkout", "-q", "feature"]);

    (origin_dir, repo_dir)
}

#[test]
fn rebase_resumes_in_progress_rebase_instead_of_starting_fresh() {
    let (_origin_dir, repo_dir) = build_stacked_squash_scenario();

    let first = RealGit.rebase(repo_dir.path(), &ref_value("origin/main"));
    assert!(first.is_err(), "expected the rebase to conflict first");

    // Resolve the conflict as a human/Cleaner would: take the upstream
    // content (which already reflects the net change) and stage it.
    fs::write(repo_dir.path().join("a.txt"), "v2").expect("failed to write a.txt");
    run_git(repo_dir.path(), &["add", "a.txt"]);

    // Re-running `rebase` mid-conflict must *resume* (`git rebase
    // --continue`), not attempt a fresh `git rebase origin/main`, which
    // would fail with "rebase-merge directory already exists".
    let second = RealGit.rebase(repo_dir.path(), &ref_value("origin/main"));
    assert!(
        second.is_ok(),
        "expected resumed rebase to succeed, got: {:?}",
        second.err()
    );

    assert!(!repo_dir.path().join(".git").join("rebase-merge").exists());
    assert!(!repo_dir.path().join(".git").join("rebase-apply").exists());
}

#[test]
fn rebase_resume_with_unresolved_conflicts_fails_with_bounded_diagnostic() {
    let (_origin_dir, repo_dir) = build_stacked_squash_scenario();

    let first = RealGit.rebase(repo_dir.path(), &ref_value("origin/main"));
    assert!(first.is_err(), "expected the rebase to conflict first");

    // Re-run without resolving anything: this must fail loudly rather than
    // loop forever or silently start a new rebase.
    let second = RealGit.rebase(repo_dir.path(), &ref_value("origin/main"));
    assert!(
        second.is_err(),
        "expected resume with unresolved conflicts to fail"
    );
}

#[test]
fn merge_resumes_in_progress_merge_instead_of_starting_fresh() {
    let (_origin_dir, repo_dir) = build_merge_conflict_scenario();

    let first = RealGit.merge(repo_dir.path(), &ref_value("origin/main"));
    assert!(first.is_err(), "expected the merge to conflict first");
    assert!(repo_dir.path().join(".git").join("MERGE_HEAD").exists());

    // Resolve the conflict and stage it.
    fs::write(repo_dir.path().join("a.txt"), "resolved").expect("failed to write a.txt");
    run_git(repo_dir.path(), &["add", "a.txt"]);

    // Re-running `merge` mid-conflict must *resume* (`git commit
    // --no-edit`), not attempt a fresh `git merge origin/main`, which
    // would fail with "fatal: MERGE_HEAD exists".
    let second = RealGit.merge(repo_dir.path(), &ref_value("origin/main"));
    assert!(
        second.is_ok(),
        "expected resumed merge to succeed, got: {:?}",
        second.err()
    );

    assert!(!repo_dir.path().join(".git").join("MERGE_HEAD").exists());
    assert_eq!(
        fs::read_to_string(repo_dir.path().join("a.txt")).expect("failed to read a.txt"),
        "resolved"
    );
}

#[test]
fn merge_resume_with_unresolved_conflicts_fails_with_bounded_diagnostic() {
    let (_origin_dir, repo_dir) = build_merge_conflict_scenario();

    let first = RealGit.merge(repo_dir.path(), &ref_value("origin/main"));
    assert!(first.is_err(), "expected the merge to conflict first");

    // Re-run without resolving anything: this must fail loudly rather than
    // loop forever or silently start a new merge.
    let second = RealGit.merge(repo_dir.path(), &ref_value("origin/main"));
    assert!(
        second.is_err(),
        "expected resume with unresolved conflicts to fail"
    );
}

use crate::domain::error::{FieldError, StateError};
use crate::domain::state::Stage;
use crate::domain::value::{NonBlankValue, SlugValue};
use crate::domain::worktree;
use crate::domain::worktree::HeistWorktree;
use crate::ports::clock::Clock;
use crate::ports::git::{GitError, GitRepository, MergeCheck};
use crate::ports::state_repository::StateRepository;
use crate::ports::worktree_fs::WorktreeFs;
use std::path::Path;

pub enum AddError {
    NoState,
    Naming(FieldError),
    Fs(std::io::Error),
    Git(GitError),
    Load(StateError),
    Save(StateError),
    BaseImmutable {
        existing: Option<String>,
        requested: String,
    },
}

#[allow(clippy::too_many_arguments)]
pub fn add(
    repo_root: &Path,
    state_repo: &dyn StateRepository,
    git: &dyn GitRepository,
    fs: &dyn WorktreeFs,
    clock: &dyn Clock,
    slug: &str,
    base: Option<&str>,
) -> Result<NonBlankValue, AddError> {
    if !state_repo.exists(slug) {
        return Err(AddError::NoState);
    }

    if let Some(b) = base {
        git.resolve_ref(repo_root, b).map_err(AddError::Git)?;
    }

    let main_branch = git.default_branch(repo_root);
    fs.ensure_worktrees_ignored(repo_root)
        .map_err(AddError::Fs)?;

    let worktree_path = worktree::worktree_path(repo_root, slug);
    let branch = worktree::branch_name(slug).map_err(AddError::Naming)?;

    let start_point = base
        .map(str::to_string)
        .unwrap_or_else(|| format!("origin/{}", main_branch));

    let worktree_exists = git.worktree_exists(repo_root, slug);

    // A `--base` only takes effect when the worktree (and its branch) is
    // created. Passing one for an existing worktree that was built from a
    // different start point would silently record a base the branch never
    // forked from, which a later `sync` would then merge in. Refuse instead.
    if worktree_exists {
        if let Some(requested) = base {
            let existing = state_repo
                .load(slug)
                .map_err(AddError::Load)?
                .base
                .map(|b| b.as_ref().to_string());
            if existing.as_deref() != Some(requested) {
                return Err(AddError::BaseImmutable {
                    existing,
                    requested: requested.to_string(),
                });
            }
        }
    } else {
        git.add_worktree(repo_root, &worktree_path, branch.as_ref(), &start_point)
            .map_err(AddError::Git)?;
    }

    fs.link_heist_dir(repo_root, &worktree_path, slug)
        .map_err(AddError::Fs)?;

    let worktree_absolute = fs.canonicalize(&worktree_path).map_err(AddError::Fs)?;
    let worktree_value = NonBlankValue::parse("worktree", &worktree_absolute.to_string_lossy())
        .map_err(AddError::Naming)?;

    let mut state = state_repo.load(slug).map_err(AddError::Load)?;
    state.worktree = Some(worktree_value.clone());
    state.branch = Some(branch);
    if let Some(b) = base {
        state.base = Some(NonBlankValue::parse("base", b).map_err(AddError::Naming)?);
    }
    state.updated = clock.today();
    state_repo.save(slug, &state).map_err(AddError::Save)?;

    Ok(worktree_value)
}

pub enum RemoveError {
    NoState,
    Naming(FieldError),
    Git(GitError),
    NotMerged {
        branch: String,
        main_branch: String,
        verification_error: Option<String>,
    },
    Load(StateError),
    Save(StateError),
}

pub fn remove(
    repo_root: &Path,
    state_repo: &dyn StateRepository,
    git: &dyn GitRepository,
    clock: &dyn Clock,
    slug: &str,
) -> Result<(), RemoveError> {
    if !state_repo.exists(slug) {
        return Err(RemoveError::NoState);
    }

    let main_branch = git.default_branch(repo_root);
    let branch = worktree::branch_name(slug).map_err(RemoveError::Naming)?;

    match git.is_branch_merged(repo_root, branch.as_ref(), &main_branch) {
        Ok(MergeCheck::Merged) => {}
        Ok(MergeCheck::NotMerged { verification_error }) => {
            return Err(RemoveError::NotMerged {
                branch: branch.to_string(),
                main_branch,
                verification_error,
            });
        }
        Err(e) => return Err(RemoveError::Git(e)),
    }

    let worktree_path = worktree::worktree_path(repo_root, slug);
    git.remove_worktree(repo_root, &worktree_path)
        .map_err(RemoveError::Git)?;
    git.delete_branch(repo_root, branch.as_ref())
        .map_err(RemoveError::Git)?;

    // Remote branch deletion is intentionally out of scope: rely
    // on GH auto-delete after merge

    let mut state = state_repo.load(slug).map_err(RemoveError::Load)?;
    state.stage = Stage::Done;
    state.updated = clock.today();
    state_repo.save(slug, &state).map_err(RemoveError::Save)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CleanupOutcome {
    Removed(SlugValue),
    Skipped {
        slug: SlugValue,
        verification_error: Option<String>,
    },
    WouldRemove(SlugValue),
    Failed {
        slug: SlugValue,
        reason: String,
    },
}

impl CleanupOutcome {
    fn slug(&self) -> &SlugValue {
        match self {
            CleanupOutcome::Removed(s) => s,
            CleanupOutcome::Skipped { slug: s, .. } => s,
            CleanupOutcome::WouldRemove(s) => s,
            CleanupOutcome::Failed { slug: s, .. } => s,
        }
    }
}

#[derive(Debug)]
pub enum CleanupError {
    Fs(std::io::Error),
    Git(GitError),
}

#[allow(clippy::too_many_arguments)]
pub fn cleanup(
    repo_root: &Path,
    state_repo: &dyn StateRepository,
    git: &dyn GitRepository,
    fs: &dyn WorktreeFs,
    clock: &dyn Clock,
    dry_run: bool,
) -> Result<Vec<CleanupOutcome>, CleanupError> {
    let canonical_repo_root = fs.canonicalize(repo_root).map_err(CleanupError::Fs)?;
    let main_branch = git.default_branch(repo_root);

    git.remote_default_resolves(repo_root, &main_branch)
        .map_err(CleanupError::Git)?;

    let infos = git.list_worktrees(repo_root).map_err(CleanupError::Git)?;

    let mut outcomes = Vec::new();
    for info in infos {
        let Some(hw) =
            HeistWorktree::try_from_parts(&info.path, info.branch.as_deref(), &canonical_repo_root)
        else {
            continue;
        };

        match git.is_branch_merged(repo_root, hw.branch.as_ref(), &main_branch) {
            Ok(MergeCheck::Merged) => {}
            Ok(MergeCheck::NotMerged { verification_error }) => {
                outcomes.push(CleanupOutcome::Skipped {
                    slug: hw.slug,
                    verification_error,
                });
                continue;
            }
            Err(e) => {
                outcomes.push(CleanupOutcome::Failed {
                    slug: hw.slug,
                    reason: e.to_string(),
                });
                continue;
            }
        }

        if dry_run {
            outcomes.push(CleanupOutcome::WouldRemove(hw.slug));
            continue;
        }

        if let Err(e) = git.remove_worktree(repo_root, &hw.path) {
            outcomes.push(CleanupOutcome::Failed {
                slug: hw.slug,
                reason: e.to_string(),
            });
            continue;
        }

        if let Err(e) = git.delete_branch(repo_root, hw.branch.as_ref()) {
            outcomes.push(CleanupOutcome::Failed {
                slug: hw.slug,
                reason: format!(
                    "worktree removed but branch {} not deleted: {}",
                    hw.branch, e
                ),
            });
            continue;
        }

        if state_repo.exists(hw.slug.as_ref()) {
            match state_repo.load(hw.slug.as_ref()) {
                Ok(mut state) => {
                    state.stage = Stage::Done;
                    state.updated = clock.today();
                    if let Err(e) = state_repo.save(hw.slug.as_ref(), &state) {
                        outcomes.push(CleanupOutcome::Failed {
                            slug: hw.slug,
                            reason: e.to_string(),
                        });
                        continue;
                    }
                }
                Err(e) => {
                    outcomes.push(CleanupOutcome::Failed {
                        slug: hw.slug,
                        reason: e.to_string(),
                    });
                    continue;
                }
            }
        }

        outcomes.push(CleanupOutcome::Removed(hw.slug));
    }

    outcomes.sort_by(|a, b| a.slug().as_ref().cmp(b.slug().as_ref()));
    Ok(outcomes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::testing::{FakeGit, FakeWorktreeFs, FixedClock, InMemoryStateRepository};
    use crate::domain::state::State;
    use crate::domain::value::DateValue;
    use std::path::Path;

    fn fixed_clock() -> FixedClock {
        FixedClock(DateValue::parse("today", "2026-01-01").expect("valid date"))
    }

    #[test]
    fn cleanup_returns_empty_outcomes_when_no_worktrees() {
        let repo = InMemoryStateRepository::new();
        let git = FakeGit::new().with_default_branch("main");

        let outcomes = cleanup(
            Path::new("."),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            false,
        )
        .expect("cleanup should succeed");

        assert!(outcomes.is_empty());
    }

    #[test]
    fn cleanup_aborts_when_origin_default_unresolvable() {
        let repo = InMemoryStateRepository::new();
        let git = FakeGit::new()
            .with_default_branch("main")
            .failing_remote_default_resolve(GitError::MergeCheck {
                message: "cannot find remote ref origin/main".into(),
            });

        let result = cleanup(
            Path::new("."),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            false,
        );

        assert!(matches!(result, Err(CleanupError::Git(_))));
    }

    #[test]
    fn cleanup_ignores_non_heist_owned_worktree() {
        let repo = InMemoryStateRepository::new();
        let git = FakeGit::new()
            .with_default_branch("main")
            .with_worktree_info("/repo/.worktrees/scratch", Some("some-other-branch"));

        let outcomes = cleanup(
            Path::new("/repo"),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            false,
        )
        .expect("cleanup should succeed");

        assert!(outcomes.is_empty());
    }

    #[test]
    fn cleanup_skips_unmerged_heist_owned_worktree() {
        let repo = InMemoryStateRepository::new();
        // No merged branch configured, so heist/foo is treated as unmerged.
        let git = FakeGit::new()
            .with_default_branch("main")
            .with_worktree_info("/repo/.worktrees/foo", Some("heist/foo"));

        let outcomes = cleanup(
            Path::new("/repo"),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            false,
        )
        .expect("cleanup should succeed");

        assert_eq!(
            outcomes,
            vec![CleanupOutcome::Skipped {
                slug: SlugValue::parse("foo").expect("valid slug"),
                verification_error: None,
            }]
        );
        assert!(git.removed_worktree_paths().is_empty());
        assert!(git.deleted_branch_names().is_empty());
    }

    #[test]
    fn cleanup_skips_with_verification_error_when_github_check_is_inconclusive() {
        let repo = InMemoryStateRepository::new();
        let git = FakeGit::new()
            .with_default_branch("main")
            .with_worktree_info("/repo/.worktrees/foo", Some("heist/foo"))
            .failing_verification_for("heist/foo", "gh: command not found");

        let outcomes = cleanup(
            Path::new("/repo"),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            false,
        )
        .expect("cleanup should succeed");

        assert_eq!(
            outcomes,
            vec![CleanupOutcome::Skipped {
                slug: SlugValue::parse("foo").expect("valid slug"),
                verification_error: Some("gh: command not found".to_string()),
            }]
        );
        assert!(git.removed_worktree_paths().is_empty());
    }

    #[test]
    fn cleanup_reports_failed_when_one_items_merge_check_errors_but_others_proceed() {
        // The top-level `origin/<default>` probe succeeds (only "main" is
        // configured to fail), so this exercises the per-item merge-check
        // error arm distinctly from the top-level abort path.
        let repo = InMemoryStateRepository::new();
        let git = FakeGit::new()
            .with_default_branch("main")
            .with_merged_branch("heist/bar")
            .with_worktree_info("/repo/.worktrees/foo", Some("heist/foo"))
            .with_worktree_info("/repo/.worktrees/bar", Some("heist/bar"))
            .failing_merge_check_for(
                "heist/foo",
                GitError::MergeCheck {
                    message: "bad ref".into(),
                },
            );

        let outcomes = cleanup(
            Path::new("/repo"),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            false,
        )
        .expect("cleanup should surface per-item failures, not error out");

        assert_eq!(
            outcomes,
            vec![
                CleanupOutcome::Removed(SlugValue::parse("bar").expect("valid slug")),
                CleanupOutcome::Failed {
                    slug: SlugValue::parse("foo").expect("valid slug"),
                    reason: "failed to check merged branches: bad ref".to_string(),
                },
            ]
        );
        assert_eq!(
            git.removed_worktree_paths(),
            vec![std::path::PathBuf::from("/repo/.worktrees/bar")]
        );
    }

    #[test]
    fn cleanup_removes_merged_heist_owned_worktree() {
        let repo = InMemoryStateRepository::new();
        let git = FakeGit::new()
            .with_default_branch("main")
            .with_merged_branch("heist/foo")
            .with_worktree_info("/repo/.worktrees/foo", Some("heist/foo"));

        let outcomes = cleanup(
            Path::new("/repo"),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            false,
        )
        .expect("cleanup should succeed");

        assert_eq!(
            outcomes,
            vec![CleanupOutcome::Removed(
                SlugValue::parse("foo").expect("valid slug")
            )]
        );
        assert_eq!(
            git.removed_worktree_paths(),
            vec![std::path::PathBuf::from("/repo/.worktrees/foo")]
        );
        assert_eq!(git.deleted_branch_names(), vec!["heist/foo".to_string()]);
    }

    #[test]
    fn cleanup_dry_run_previews_without_mutating() {
        let repo = InMemoryStateRepository::new();
        let git = FakeGit::new()
            .with_default_branch("main")
            .with_merged_branch("heist/foo")
            .with_worktree_info("/repo/.worktrees/foo", Some("heist/foo"));

        let outcomes = cleanup(
            Path::new("/repo"),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            true,
        )
        .expect("cleanup should succeed");

        assert_eq!(
            outcomes,
            vec![CleanupOutcome::WouldRemove(
                SlugValue::parse("foo").expect("valid slug")
            )]
        );
        assert!(git.removed_worktree_paths().is_empty());
        assert!(git.deleted_branch_names().is_empty());
    }

    #[test]
    fn cleanup_marks_existing_state_done() {
        let repo = InMemoryStateRepository::new().with_state(
            "foo",
            State::new(
                "foo",
                DateValue::parse("today", "2025-01-01").expect("valid date"),
            )
            .expect("valid slug"),
        );
        let git = FakeGit::new()
            .with_default_branch("main")
            .with_merged_branch("heist/foo")
            .with_worktree_info("/repo/.worktrees/foo", Some("heist/foo"));

        let outcomes = cleanup(
            Path::new("/repo"),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            false,
        )
        .expect("cleanup should succeed");

        assert_eq!(
            outcomes,
            vec![CleanupOutcome::Removed(
                SlugValue::parse("foo").expect("valid slug")
            )]
        );
        let state = repo.get("foo").expect("state should still exist");
        assert_eq!(state.stage, Stage::Done);
        assert_eq!(
            state.updated,
            DateValue::parse("today", "2026-01-01").expect("valid date")
        );
    }

    #[test]
    fn cleanup_removes_orphan_worktree_without_state() {
        let repo = InMemoryStateRepository::new(); // no state for "foo"
        let git = FakeGit::new()
            .with_default_branch("main")
            .with_merged_branch("heist/foo")
            .with_worktree_info("/repo/.worktrees/foo", Some("heist/foo"));

        let outcomes = cleanup(
            Path::new("/repo"),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            false,
        )
        .expect("cleanup should succeed even without a state entry");

        assert_eq!(
            outcomes,
            vec![CleanupOutcome::Removed(
                SlugValue::parse("foo").expect("valid slug")
            )]
        );
        assert_eq!(repo.get("foo"), None);
    }

    #[test]
    fn cleanup_reports_failed_when_remove_worktree_fails() {
        let repo = InMemoryStateRepository::new().with_state(
            "foo",
            State::new(
                "foo",
                DateValue::parse("today", "2025-01-01").expect("valid date"),
            )
            .expect("valid slug"),
        );
        let git = FakeGit::new()
            .with_default_branch("main")
            .with_merged_branch("heist/foo")
            .with_worktree_info("/repo/.worktrees/foo", Some("heist/foo"))
            .failing_remove(GitError::WorktreeRemove {
                message: "worktree is dirty".into(),
            });

        let outcomes = cleanup(
            Path::new("/repo"),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            false,
        )
        .expect("cleanup should surface per-item failures, not error out");

        match &outcomes[..] {
            [CleanupOutcome::Failed { slug, reason }] => {
                assert_eq!(slug.as_ref(), "foo");
                assert!(reason.contains("worktree is dirty"));
            }
            other => panic!("expected a single Failed outcome, got {:?}", other),
        }
        assert!(git.deleted_branch_names().is_empty());
        let state = repo.get("foo").expect("state should still exist");
        assert_eq!(state.stage, Stage::Casing);
    }

    #[test]
    fn cleanup_reports_orphaned_branch_when_delete_fails() {
        let repo = InMemoryStateRepository::new().with_state(
            "foo",
            State::new(
                "foo",
                DateValue::parse("today", "2025-01-01").expect("valid date"),
            )
            .expect("valid slug"),
        );
        let git = FakeGit::new()
            .with_default_branch("main")
            .with_merged_branch("heist/foo")
            .with_worktree_info("/repo/.worktrees/foo", Some("heist/foo"))
            .failing_delete(GitError::BranchDelete {
                message: "not fully merged".into(),
            });

        let outcomes = cleanup(
            Path::new("/repo"),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            false,
        )
        .expect("cleanup should surface per-item failures, not error out");

        match &outcomes[..] {
            [CleanupOutcome::Failed { slug, reason }] => {
                assert_eq!(slug.as_ref(), "foo");
                assert!(reason.contains("worktree removed but branch heist/foo not deleted"));
                assert!(reason.contains("not fully merged"));
            }
            other => panic!("expected a single Failed outcome, got {:?}", other),
        }
        // The worktree removal did happen before the branch-delete failure.
        assert_eq!(
            git.removed_worktree_paths(),
            vec![std::path::PathBuf::from("/repo/.worktrees/foo")]
        );
        let state = repo.get("foo").expect("state should still exist");
        assert_eq!(state.stage, Stage::Casing);
    }

    #[test]
    fn cleanup_sorts_outcomes_by_slug() {
        let repo = InMemoryStateRepository::new();
        let git = FakeGit::new()
            .with_default_branch("main")
            .with_merged_branch("heist/zeta")
            .with_merged_branch("heist/alpha")
            .with_worktree_info("/repo/.worktrees/zeta", Some("heist/zeta"))
            .with_worktree_info("/repo/.worktrees/alpha", Some("heist/alpha"));

        let outcomes = cleanup(
            Path::new("/repo"),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            false,
        )
        .expect("cleanup should succeed");

        assert_eq!(
            outcomes,
            vec![
                CleanupOutcome::Removed(SlugValue::parse("alpha").expect("valid slug")),
                CleanupOutcome::Removed(SlugValue::parse("zeta").expect("valid slug")),
            ]
        );
    }

    #[test]
    fn add_with_base_validates_ref_before_creating_worktree_or_state() {
        let repo = InMemoryStateRepository::new().with_state(
            "foo",
            State::new(
                "foo",
                DateValue::parse("today", "2025-01-01").expect("valid date"),
            )
            .expect("valid slug"),
        );
        let git = FakeGit::new()
            .with_default_branch("main")
            .failing_resolve_ref_for(
                "heist/piece-01",
                GitError::MergeCheck {
                    message: "bad ref".into(),
                },
            );

        let result = add(
            Path::new("."),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            "foo",
            Some("heist/piece-01"),
        );

        assert!(matches!(result, Err(AddError::Git(_))));
        assert!(repo
            .get("foo")
            .expect("foo state should exist")
            .worktree
            .is_none());
    }

    #[test]
    fn add_with_base_uses_verbatim_ref_as_start_point_and_persists_base_field() {
        let repo = InMemoryStateRepository::new().with_state(
            "foo",
            State::new(
                "foo",
                DateValue::parse("today", "2025-01-01").expect("valid date"),
            )
            .expect("valid slug"),
        );
        let git = FakeGit::new().with_default_branch("main");

        let result = add(
            Path::new("."),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            "foo",
            Some("heist/piece-01"),
        );

        assert!(result.is_ok());
        assert_eq!(
            git.add_worktree_start_points(),
            vec!["heist/piece-01".to_string()]
        );
        let saved_state = repo.get("foo").expect("foo state should exist");
        assert_eq!(
            saved_state.base,
            Some(NonBlankValue::parse("base", "heist/piece-01").expect("valid base"))
        );
    }

    #[test]
    fn add_without_base_preserves_previously_persisted_base() {
        let mut state = State::new(
            "foo",
            DateValue::parse("today", "2025-01-01").expect("valid date"),
        )
        .expect("valid slug");
        state.base = Some(NonBlankValue::parse("base", "heist/piece-01").expect("valid base"));
        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new().with_default_branch("main");

        let result = add(
            Path::new("."),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            "foo",
            None,
        );

        assert!(result.is_ok());
        let saved_state = repo.get("foo").expect("foo state should exist");
        assert_eq!(
            saved_state.base,
            Some(NonBlankValue::parse("base", "heist/piece-01").expect("valid base")),
            "re-running `heist worktree add` without --base must not null an already-persisted base"
        );
    }

    #[test]
    fn add_with_differing_base_on_existing_worktree_is_refused_and_state_unchanged() {
        let mut state = State::new(
            "foo",
            DateValue::parse("today", "2025-01-01").expect("valid date"),
        )
        .expect("valid slug");
        state.base = Some(NonBlankValue::parse("base", "heist/piece-01").expect("valid base"));
        let repo = InMemoryStateRepository::new().with_state("foo", state);
        // Worktree already exists, created from heist/piece-01.
        let git = FakeGit::new()
            .with_default_branch("main")
            .with_existing_worktree("foo");

        let result = add(
            Path::new("."),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            "foo",
            Some("heist/piece-02"),
        );

        assert!(matches!(result, Err(AddError::BaseImmutable { .. })));
        let saved_state = repo.get("foo").expect("foo state should exist");
        assert_eq!(
            saved_state.base,
            Some(NonBlankValue::parse("base", "heist/piece-01").expect("valid base")),
            "a refused re-add must not rewrite the persisted base"
        );
    }

    #[test]
    fn add_with_same_base_on_existing_worktree_is_idempotent() {
        let mut state = State::new(
            "foo",
            DateValue::parse("today", "2025-01-01").expect("valid date"),
        )
        .expect("valid slug");
        state.base = Some(NonBlankValue::parse("base", "heist/piece-01").expect("valid base"));
        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new()
            .with_default_branch("main")
            .with_existing_worktree("foo");

        let result = add(
            Path::new("."),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            "foo",
            Some("heist/piece-01"),
        );

        assert!(result.is_ok());
    }

    #[test]
    fn add_without_base_uses_origin_default_start_point() {
        let repo = InMemoryStateRepository::new().with_state(
            "foo",
            State::new(
                "foo",
                DateValue::parse("today", "2025-01-01").expect("valid date"),
            )
            .expect("valid slug"),
        );
        let git = FakeGit::new().with_default_branch("main");

        let result = add(
            Path::new("."),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            "foo",
            None,
        );

        assert!(result.is_ok());
        assert_eq!(
            git.add_worktree_start_points(),
            vec!["origin/main".to_string()]
        );
    }
}

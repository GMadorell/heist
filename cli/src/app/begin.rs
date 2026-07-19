use crate::app;
use crate::domain::error::FieldError;
use crate::domain::value::{NonBlankValue, SlugValue};
use crate::ports::clock::Clock;
use crate::ports::git::GitRepository;
use crate::ports::state_repository::StateRepository;
use crate::ports::worktree_fs::WorktreeFs;
use std::path::Path;

pub enum CollisionArtifact {
    State,
    Worktree,
    Branch,
}

impl CollisionArtifact {
    pub fn describe(&self, slug: &str) -> String {
        match self {
            CollisionArtifact::State => format!(".heist/{}/", slug),
            CollisionArtifact::Worktree => format!(".worktrees/{}", slug),
            CollisionArtifact::Branch => format!("branch heist/{}", slug),
        }
    }
}

pub enum BeginError {
    InvalidSlug(FieldError),
    Collision(CollisionArtifact),
    Mode {
        error: app::state::SetError,
        rollback_errors: Vec<String>,
    },
    Worktree {
        error: app::worktree::AddError,
        rollback_errors: Vec<String>,
    },
    Stage {
        error: app::state::SetError,
        rollback_errors: Vec<String>,
    },
}

fn rollback(
    repo_root: &Path,
    state_repo: &dyn StateRepository,
    git: &dyn GitRepository,
    slug: &str,
    branch: &str,
) -> Vec<String> {
    let mut errors = Vec::new();

    if git.worktree_exists(repo_root, slug) {
        let worktree_path = crate::domain::worktree::worktree_path(repo_root, slug);
        if let Err(e) = git.remove_worktree(repo_root, &worktree_path) {
            errors.push(format!("failed to remove worktree: {}", e));
        }
    }
    if git.branch_exists(repo_root, branch) {
        if let Err(e) = git.delete_branch(repo_root, branch) {
            errors.push(format!("failed to delete branch: {}", e));
        }
    }
    if let Err(e) = state_repo.remove(slug) {
        errors.push(format!("failed to remove state directory: {}", e));
    }
    errors
}

#[allow(clippy::too_many_arguments)]
pub fn begin(
    repo_root: &Path,
    state_repo: &dyn StateRepository,
    git: &dyn GitRepository,
    fs: &dyn WorktreeFs,
    clock: &dyn Clock,
    slug: &str,
    mode: &str,
    base: Option<&str>,
) -> Result<NonBlankValue, BeginError> {
    SlugValue::parse(slug).map_err(BeginError::InvalidSlug)?;

    if state_repo.exists(slug) {
        return Err(BeginError::Collision(CollisionArtifact::State));
    }
    if git.worktree_exists(repo_root, slug) {
        return Err(BeginError::Collision(CollisionArtifact::Worktree));
    }
    let branch = crate::domain::worktree::branch_name(slug).map_err(BeginError::InvalidSlug)?;
    if git.branch_exists(repo_root, branch.as_ref()) {
        return Err(BeginError::Collision(CollisionArtifact::Branch));
    }

    match app::state::init(state_repo, clock, slug) {
        Ok(()) => {}
        Err(app::state::InitError::InvalidSlug(e)) => return Err(BeginError::InvalidSlug(e)),
        Err(app::state::InitError::Init(_)) => {
            return Err(BeginError::Collision(CollisionArtifact::State));
        }
    }

    if let Err(error) = app::state::set(state_repo, clock, slug, "mode", mode) {
        let rollback_errors = rollback(repo_root, state_repo, git, slug, branch.as_ref());
        return Err(BeginError::Mode {
            error,
            rollback_errors,
        });
    }

    let worktree_value = match app::worktree::add(repo_root, state_repo, git, fs, clock, slug, base)
    {
        Ok(v) => v,
        Err(error) => {
            let rollback_errors = rollback(repo_root, state_repo, git, slug, branch.as_ref());
            return Err(BeginError::Worktree {
                error,
                rollback_errors,
            });
        }
    };

    if let Err(error) = app::state::set(state_repo, clock, slug, "stage", "planning") {
        let rollback_errors = rollback(repo_root, state_repo, git, slug, branch.as_ref());
        return Err(BeginError::Stage {
            error,
            rollback_errors,
        });
    }

    Ok(worktree_value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::testing::{FakeGit, FakeWorktreeFs, FixedClock, InMemoryStateRepository};
    use crate::domain::value::DateValue;
    use std::path::Path;

    fn fixed_clock() -> FixedClock {
        FixedClock(DateValue::parse("today", "2026-01-01").expect("valid date"))
    }

    #[test]
    fn begin_happy_path_composes_init_mode_worktree_and_stage() {
        let repo = InMemoryStateRepository::new();
        let git = FakeGit::new().with_default_branch("main");

        let result = begin(
            Path::new("."),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            "my-slug",
            "medium",
            None,
        );

        assert!(result.is_ok(), "begin should succeed");
        let state = repo.get("my-slug").expect("state should exist");
        assert_eq!(state.mode, crate::domain::state::Mode::Medium);
        assert_eq!(state.stage, crate::domain::state::Stage::Planning);
        assert!(state.worktree.is_some());
        assert!(state.branch.is_some());
    }

    #[test]
    fn begin_rejects_malformed_slug_before_any_mutation() {
        let repo = InMemoryStateRepository::new();
        let git = FakeGit::new().with_default_branch("main");

        let result = begin(
            Path::new("."),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            "Not A Slug",
            "heavy",
            None,
        );

        assert!(matches!(result, Err(BeginError::InvalidSlug(_))));
        assert!(!repo.exists("Not A Slug"));
    }

    #[test]
    fn begin_rejects_precheck_collision_for_existing_state_worktree_or_branch() {
        let repo_with_state = InMemoryStateRepository::new().with_state(
            "foo",
            crate::domain::state::State::new(
                "foo",
                DateValue::parse("today", "2026-01-01").expect("valid date"),
            )
            .expect("valid slug"),
        );
        let git = FakeGit::new().with_default_branch("main");
        let result = begin(
            Path::new("."),
            &repo_with_state,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            "foo",
            "heavy",
            None,
        );
        assert!(matches!(
            result,
            Err(BeginError::Collision(CollisionArtifact::State))
        ));

        let repo_no_state = InMemoryStateRepository::new();
        let git_worktree_collision = FakeGit::new()
            .with_default_branch("main")
            .with_existing_worktree("bar");
        let result = begin(
            Path::new("."),
            &repo_no_state,
            &git_worktree_collision,
            &FakeWorktreeFs,
            &fixed_clock(),
            "bar",
            "heavy",
            None,
        );
        assert!(matches!(
            result,
            Err(BeginError::Collision(CollisionArtifact::Worktree))
        ));

        let git_branch_collision = FakeGit::new()
            .with_default_branch("main")
            .with_branch("heist/baz");
        let result = begin(
            Path::new("."),
            &repo_no_state,
            &git_branch_collision,
            &FakeWorktreeFs,
            &fixed_clock(),
            "baz",
            "heavy",
            None,
        );
        assert!(matches!(
            result,
            Err(BeginError::Collision(CollisionArtifact::Branch))
        ));
    }

    #[test]
    fn begin_rolls_back_state_dir_when_mode_set_fails_before_worktree_creation() {
        let repo = InMemoryStateRepository::new();
        let git = FakeGit::new().with_default_branch("main");

        let result = begin(
            Path::new("."),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            "foo",
            "bogus-mode",
            None,
        );

        assert!(matches!(result, Err(BeginError::Mode { .. })));
        assert!(!repo.exists("foo"), "state dir should be rolled back");
        assert!(
            git.removed_worktree_paths().is_empty(),
            "nothing was created yet, rollback must not attempt worktree removal"
        );
        assert!(git.deleted_branch_names().is_empty());
    }

    #[test]
    fn begin_rolls_back_worktree_and_branch_when_a_later_step_fails() {
        use crate::ports::worktree_fs::WorktreeFs;
        use std::path::PathBuf;

        struct FailingLinkFs;
        impl WorktreeFs for FailingLinkFs {
            fn ensure_worktrees_ignored(&self, _repo_root: &Path) -> std::io::Result<bool> {
                Ok(false)
            }
            fn link_heist_dir(
                &self,
                _repo_root: &Path,
                _worktree_path: &Path,
                _slug: &str,
            ) -> std::io::Result<()> {
                Err(std::io::Error::other("link failed"))
            }
            fn canonicalize(&self, path: &Path) -> std::io::Result<PathBuf> {
                Ok(path.to_path_buf())
            }
        }

        let repo = InMemoryStateRepository::new();
        let git = FakeGit::new().with_default_branch("main");

        let result = begin(
            Path::new("."),
            &repo,
            &git,
            &FailingLinkFs,
            &fixed_clock(),
            "foo",
            "heavy",
            None,
        );

        assert!(matches!(result, Err(BeginError::Worktree { .. })));
        assert_eq!(git.removed_worktree_paths().len(), 1);
        assert_eq!(git.deleted_branch_names(), vec!["heist/foo".to_string()]);
        assert!(!repo.exists("foo"), "state dir should be rolled back");
    }
}

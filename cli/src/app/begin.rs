use crate::app;
use crate::domain::error::{StateError, ValueError};
use crate::domain::state::Mode;
use crate::domain::value::{BranchValue, NonBlankValue, RefValue, SlugValue};
use crate::domain::worktree::{branch_name, worktree_path};
use crate::ports::clock::Clock;
use crate::ports::git::{GitError, GitRepository};
use crate::ports::heist_dir_repository::HeistDirRepository;
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

pub enum RollbackFailure {
    WorktreeProbe(GitError),
    WorktreeRemove(GitError),
    BranchProbe(GitError),
    BranchDelete(GitError),
    HeistDirRemove(StateError),
}

pub enum BeginError {
    InvalidSlug(ValueError),
    Collision(CollisionArtifact),
    /// Inconclusive probe (not a confirmed absence), so never safe to proceed.
    Probe(GitError),
    Init(StateError),
    State {
        error: app::state::SetError,
        rollback_errors: Vec<RollbackFailure>,
    },
    Worktree {
        error: app::worktree::AddError,
        rollback_errors: Vec<RollbackFailure>,
    },
}

#[allow(clippy::too_many_arguments)]
pub fn begin(
    repo_root: &Path,
    heist_dir_repo: &dyn HeistDirRepository,
    state_repo: &dyn StateRepository,
    git: &dyn GitRepository,
    fs: &dyn WorktreeFs,
    clock: &dyn Clock,
    slug: &str,
    mode: Mode,
    base: Option<&RefValue>,
) -> Result<NonBlankValue, BeginError> {
    let branch = check_preconditions(repo_root, state_repo, git, slug)?;

    match app::state::init(heist_dir_repo, state_repo, clock, slug) {
        Ok(()) => {}
        Err(app::state::InitError::InvalidSlug(e)) => return Err(BeginError::InvalidSlug(e)),
        // Only a lost pre-check/init race maps to Collision; any other
        // StateError is a genuine init failure, not a collision.
        Err(app::state::InitError::Init(StateError::AlreadyExists)) => {
            return Err(BeginError::Collision(CollisionArtifact::State));
        }
        Err(app::state::InitError::Init(e)) => return Err(BeginError::Init(e)),
    }

    set_field_or_rollback(
        state_repo,
        clock,
        repo_root,
        heist_dir_repo,
        git,
        slug,
        &branch,
        "mode",
        mode.as_str(),
    )?;

    let worktree_value = match app::worktree::add(repo_root, state_repo, git, fs, clock, slug, base)
    {
        Ok(v) => v,
        Err(error) => {
            let rollback_errors = rollback(repo_root, heist_dir_repo, git, slug, &branch);
            return Err(BeginError::Worktree {
                error,
                rollback_errors,
            });
        }
    };

    set_field_or_rollback(
        state_repo,
        clock,
        repo_root,
        heist_dir_repo,
        git,
        slug,
        &branch,
        "stage",
        "planning",
    )?;

    Ok(worktree_value)
}

fn check_preconditions(
    repo_root: &Path,
    state_repo: &dyn StateRepository,
    git: &dyn GitRepository,
    slug: &str,
) -> Result<BranchValue, BeginError> {
    SlugValue::parse(slug).map_err(BeginError::InvalidSlug)?;

    if state_repo.exists(slug) {
        return Err(BeginError::Collision(CollisionArtifact::State));
    }
    if git
        .worktree_exists(repo_root, slug)
        .map_err(BeginError::Probe)?
    {
        return Err(BeginError::Collision(CollisionArtifact::Worktree));
    }
    let branch = branch_name(slug).map_err(BeginError::InvalidSlug)?;
    if git
        .branch_exists(repo_root, &branch)
        .map_err(BeginError::Probe)?
    {
        return Err(BeginError::Collision(CollisionArtifact::Branch));
    }
    Ok(branch)
}

#[allow(clippy::too_many_arguments)]
fn set_field_or_rollback(
    state_repo: &dyn StateRepository,
    clock: &dyn Clock,
    repo_root: &Path,
    heist_dir_repo: &dyn HeistDirRepository,
    git: &dyn GitRepository,
    slug: &str,
    branch: &BranchValue,
    field: &str,
    value: &str,
) -> Result<(), BeginError> {
    if let Err(error) = app::state::set(state_repo, clock, slug, field, value) {
        let rollback_errors = rollback(repo_root, heist_dir_repo, git, slug, branch);
        return Err(BeginError::State {
            error,
            rollback_errors,
        });
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn rollback(
    repo_root: &Path,
    heist_dir_repo: &dyn HeistDirRepository,
    git: &dyn GitRepository,
    slug: &str,
    branch: &BranchValue,
) -> Vec<RollbackFailure> {
    let mut errors = Vec::new();

    match git.worktree_exists(repo_root, slug) {
        Ok(true) => {
            let path = worktree_path(repo_root, slug);
            if let Err(e) = git.remove_worktree(repo_root, &path) {
                errors.push(RollbackFailure::WorktreeRemove(e));
            }
        }
        Ok(false) => {}
        Err(e) => errors.push(RollbackFailure::WorktreeProbe(e)),
    }
    match git.branch_exists(repo_root, &branch) {
        Ok(true) => {
            if let Err(e) = git.delete_branch(repo_root, &branch) {
                errors.push(RollbackFailure::BranchDelete(e));
            }
        }
        Ok(false) => {}
        Err(e) => errors.push(RollbackFailure::BranchProbe(e)),
    }
    if let Err(e) = heist_dir_repo.remove(slug) {
        errors.push(RollbackFailure::HeistDirRemove(e));
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::testing::{
        FakeGit, FakeWorktreeFs, FixedClock, InMemoryHeistDirRepository, InMemoryStateRepository,
    };
    use crate::domain::state::{Mode, Stage, State};
    use crate::domain::value::DateValue;
    use crate::ports::worktree_fs::WorktreeFs;
    use std::path::{Path, PathBuf};

    fn fixed_clock() -> FixedClock {
        FixedClock(DateValue::parse("today", "2026-01-01").expect("valid date"))
    }

    #[test]
    fn begin_happy_path_composes_init_mode_worktree_and_stage() {
        let heist_dir_repo = InMemoryHeistDirRepository::new();
        let repo = InMemoryStateRepository::new();
        let git = FakeGit::new().with_default_branch("main");

        let result = begin(
            Path::new("."),
            &heist_dir_repo,
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            "my-slug",
            crate::domain::state::Mode::Medium,
            None,
        );

        assert!(result.is_ok(), "begin should succeed");
        let state = repo.get("my-slug").expect("state should exist");
        assert_eq!(state.mode, Mode::Medium);
        assert_eq!(state.stage, Stage::Planning);
        assert!(state.worktree.is_some());
        assert!(state.branch.is_some());
    }

    #[test]
    fn begin_rejects_malformed_slug_before_any_mutation() {
        let heist_dir_repo = InMemoryHeistDirRepository::new();
        let repo = InMemoryStateRepository::new();
        let git = FakeGit::new().with_default_branch("main");

        let result = begin(
            Path::new("."),
            &heist_dir_repo,
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            "Not A Slug",
            crate::domain::state::Mode::Heavy,
            None,
        );

        assert!(matches!(result, Err(BeginError::InvalidSlug(_))));
        assert!(!repo.exists("Not A Slug"));
        assert!(!heist_dir_repo.exists("Not A Slug"));
    }

    #[test]
    fn begin_rejects_precheck_collision_for_existing_state_worktree_or_branch() {
        let repo_with_state = InMemoryStateRepository::new().with_state(
            "foo",
            State::new(
                "foo",
                DateValue::parse("today", "2026-01-01").expect("valid date"),
            )
            .expect("valid slug"),
        );
        let heist_dir_repo = InMemoryHeistDirRepository::new();
        let git = FakeGit::new().with_default_branch("main");
        let result = begin(
            Path::new("."),
            &heist_dir_repo,
            &repo_with_state,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            "foo",
            crate::domain::state::Mode::Heavy,
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
            &heist_dir_repo,
            &repo_no_state,
            &git_worktree_collision,
            &FakeWorktreeFs,
            &fixed_clock(),
            "bar",
            crate::domain::state::Mode::Heavy,
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
            &heist_dir_repo,
            &repo_no_state,
            &git_branch_collision,
            &FakeWorktreeFs,
            &fixed_clock(),
            "baz",
            crate::domain::state::Mode::Heavy,
            None,
        );
        assert!(matches!(
            result,
            Err(BeginError::Collision(CollisionArtifact::Branch))
        ));
    }

    #[test]
    fn begin_rolls_back_state_dir_when_stage_set_fails_after_worktree_creation() {
        struct FailAfterModeStateRepository {
            inner: InMemoryStateRepository,
            set_calls: std::cell::Cell<u32>,
        }

        impl StateRepository for FailAfterModeStateRepository {
            fn exists(&self, slug: &str) -> bool {
                self.inner.exists(slug)
            }
            fn load(&self, slug: &str) -> Result<State, StateError> {
                self.inner.load(slug)
            }
            fn save(&self, slug: &str, state: &State) -> Result<(), StateError> {
                let call = self.set_calls.get();
                self.set_calls.set(call + 1);
                // Saves happen in order: 0 = init's own save, 1 = the "mode"
                // set, 2 = worktree add's own state save, 3 = the trailing
                // "stage" set. Only fail the fourth so the S -- fails --> RB
                // edge of the flowchart is actually exercised (not the
                // earlier steps).
                if call == 3 {
                    return Err(StateError::Unreadable(std::io::Error::other(
                        "simulated stage-save failure",
                    )));
                }
                self.inner.save(slug, state)
            }
            fn list_slugs(&self) -> Result<Vec<SlugValue>, StateError> {
                self.inner.list_slugs()
            }
        }

        let heist_dir_repo = InMemoryHeistDirRepository::new();
        let repo = FailAfterModeStateRepository {
            inner: InMemoryStateRepository::new(),
            set_calls: std::cell::Cell::new(0),
        };
        let git = FakeGit::new().with_default_branch("main");

        let result = begin(
            Path::new("."),
            &heist_dir_repo,
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            "foo",
            crate::domain::state::Mode::Heavy,
            None,
        );

        assert!(matches!(result, Err(BeginError::State { .. })));
        assert!(
            !heist_dir_repo.exists("foo"),
            "heist dir should be rolled back"
        );
        assert_eq!(
            git.removed_worktree_paths().len(),
            1,
            "worktree created before the stage-set failure must be rolled back"
        );
        assert_eq!(git.deleted_branch_names(), vec!["heist/foo".to_string()]);
    }

    #[test]
    fn begin_rolls_back_worktree_and_branch_when_a_later_step_fails() {
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

        let heist_dir_repo = InMemoryHeistDirRepository::new();
        let repo = InMemoryStateRepository::new();
        let git = FakeGit::new().with_default_branch("main");

        let result = begin(
            Path::new("."),
            &heist_dir_repo,
            &repo,
            &git,
            &FailingLinkFs,
            &fixed_clock(),
            "foo",
            crate::domain::state::Mode::Heavy,
            None,
        );

        assert!(matches!(result, Err(BeginError::Worktree { .. })));
        assert_eq!(git.removed_worktree_paths().len(), 1);
        assert_eq!(git.deleted_branch_names(), vec!["heist/foo".to_string()]);
        assert!(
            !heist_dir_repo.exists("foo"),
            "heist dir should be rolled back"
        );
    }
}

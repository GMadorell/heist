use crate::app;
use crate::domain::error::FieldError;
use crate::domain::value::{NonBlankValue, SlugValue};
use crate::ports::clock::Clock;
use crate::ports::git::GitRepository;
use crate::ports::state_repository::StateRepository;
use crate::ports::worktree_fs::WorktreeFs;
use std::path::Path;

pub enum BeginError {
    InvalidSlug(FieldError),
    Mode(app::state::SetError),
    Worktree(app::worktree::AddError),
    Stage(app::state::SetError),
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

    match app::state::init(state_repo, clock, slug) {
        Ok(()) => {}
        Err(app::state::InitError::InvalidSlug(e)) => return Err(BeginError::InvalidSlug(e)),
        Err(app::state::InitError::Init(e)) => {
            return Err(BeginError::Mode(app::state::SetError::Load(e)))
        }
    }

    app::state::set(state_repo, clock, slug, "mode", mode).map_err(BeginError::Mode)?;
    let worktree_value = app::worktree::add(repo_root, state_repo, git, fs, clock, slug, base)
        .map_err(BeginError::Worktree)?;
    app::state::set(state_repo, clock, slug, "stage", "planning").map_err(BeginError::Stage)?;
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
}

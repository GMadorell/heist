pub mod exit_code;
mod present;

use crate::adapters::file_state_repository::FileStateRepository;
use crate::adapters::filesystem_worktree::FilesystemWorktree;
use crate::adapters::real_git::RealGit;
use crate::adapters::system_clock::SystemClock;
use crate::adapters::validation_fs::ValidationFs;
use crate::app;
use crate::ports::state_repository::StateRepository;
use clap::{Parser, Subcommand};
use exit_code::ExitCode;
use std::path::Path;

#[derive(Parser)]
#[command(name = "heist")]
#[command(
    about = "Deterministic bookkeeping for the Heist pipeline: state, worktrees, validation.md lookup",
    long_about = "Deterministic, token-free half of the Heist pipeline: state tracking, worktree \
setup/teardown, and validation.md lookup. All commands read/write `.heist/<slug>/state.json` \
relative to the current directory unless noted.\n\n\
Exit codes: 0 success, 1 internal error, 2 precondition failed, 3 git command failed, 4 invalid path argument."
)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage heist state.json (init, get, set, incr, schema)
    State {
        #[command(subcommand)]
        command: StateCommands,
    },
    /// Create or remove the git worktree/branch for a heist
    Worktree {
        #[command(subcommand)]
        command: WorktreeCommands,
    },
    /// Look up and merge validation.md files
    Validation {
        #[command(subcommand)]
        command: ValidationCommands,
    },
    /// Select the reviewer lanes for the current diff (conditional on file types touched)
    Review {
        #[command(subcommand)]
        command: ReviewCommands,
    },
    /// Print a short summary (stage, next_step, worktree) for picking a heist back up
    Resume {
        /// Heist slug (directory name under .heist/)
        slug: String,
    },
    /// Print one line per heist under .heist/, sorted by slug
    List,
}

#[derive(Subcommand)]
enum StateCommands {
    /// Create .heist/<slug>/state.json with defaults; fails if it already exists
    Init {
        /// Heist slug (directory name under .heist/)
        slug: String,
    },
    /// Print one field's value (or `null`)
    Get {
        /// Heist slug (directory name under .heist/)
        slug: String,
        /// State field name, e.g. stage, worktree, branch, score_wave
        field: String,
    },
    /// Update one field and bump `updated` to today; validates the value
    Set {
        /// Heist slug (directory name under .heist/)
        slug: String,
        /// State field name, e.g. stage, worktree, branch, score_wave
        field: String,
        /// New value for the field
        value: String,
    },
    /// Add 1 to a numeric field and bump `updated` to today
    Incr {
        /// Heist slug (directory name under .heist/)
        slug: String,
        /// Numeric state field name, e.g. score_wave
        field: String,
    },
    /// Print the field list and an example state.json (no slug required)
    Schema,
}

#[derive(Subcommand)]
enum WorktreeCommands {
    /// Create .worktrees/<slug> on branch heist/<slug>; requires `state init` first
    Add {
        /// Heist slug (directory name under .heist/)
        slug: String,
    },
    /// Remove the worktree and branch, then set stage: done; refuses if unmerged
    Remove {
        /// Heist slug (directory name under .heist/)
        slug: String,
    },
    /// Remove every heist-owned worktree whose branch is already merged
    Cleanup {
        /// Preview without removing anything
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
enum ValidationCommands {
    /// Merge the nearest validation.md with the root one for each path, deduped
    Resolve {
        /// One or more file/directory paths to resolve validation.md for
        paths: Vec<std::path::PathBuf>,
    },
    /// Exit 0 (prints `ok`) if a validation.md covers path, exit 2 (prints `missing`) otherwise
    Check {
        /// File/directory path to check
        path: std::path::PathBuf,
    },
}

#[derive(Subcommand)]
enum ReviewCommands {
    /// Print the reviewer lanes to run for the diff since main, one bare lane name per line
    Select {
        /// Heist slug (directory name under .heist/)
        slug: String,
    },
}

pub fn run(cli: Cli) -> ExitCode {
    let state_repo = FileStateRepository;
    let git = RealGit;
    let fs = FilesystemWorktree;
    let clock = SystemClock;
    let validation_src = ValidationFs;
    let repo_root = Path::new(".");

    match cli.command {
        Commands::State { command } => run_state(command, &state_repo, &clock),
        Commands::Worktree { command } => {
            run_worktree(command, repo_root, &state_repo, &git, &fs, &clock)
        }
        Commands::Validation { command } => run_validation(command, &validation_src),
        Commands::Review { command } => run_review(command, repo_root, &state_repo, &git),
        Commands::Resume { slug } => run_resume(&slug, &state_repo),
        Commands::List => run_list(&state_repo),
    }
}

fn run_state(
    command: StateCommands,
    repo: &dyn StateRepository,
    clock: &dyn crate::ports::clock::Clock,
) -> ExitCode {
    match command {
        StateCommands::Init { slug } => match app::state::init(repo, clock, &slug) {
            Ok(()) => ExitCode::Success,
            Err(app::state::InitError::InvalidSlug(e)) => {
                present::error(e);
                ExitCode::Precondition
            }
            Err(app::state::InitError::Init(e)) => {
                present::state_init_failed(&slug, &e);
                ExitCode::from(&e)
            }
        },
        StateCommands::Get { slug, field } => match app::state::get(repo, &slug, &field) {
            Ok(value) => {
                present::line(value);
                ExitCode::Success
            }
            Err(app::state::GetError::Load(e)) => {
                present::state_load_failed(&slug, &e);
                ExitCode::from(&e)
            }
            Err(app::state::GetError::Field(e)) => {
                present::error(e);
                ExitCode::Precondition
            }
        },
        StateCommands::Set { slug, field, value } => {
            match app::state::set(repo, clock, &slug, &field, &value) {
                Ok(()) => ExitCode::Success,
                Err(app::state::SetError::Field(e)) => {
                    present::error(e);
                    ExitCode::Precondition
                }
                Err(app::state::SetError::Load(e)) => {
                    present::state_load_failed(&slug, &e);
                    ExitCode::from(&e)
                }
                Err(app::state::SetError::Save(e)) => {
                    present::state_save_failed(&slug, &e);
                    ExitCode::from(&e)
                }
            }
        }
        StateCommands::Incr { slug, field } => match app::state::incr(repo, clock, &slug, &field) {
            Ok(()) => ExitCode::Success,
            Err(app::state::IncrError::Field(e)) => {
                present::error(e);
                ExitCode::Precondition
            }
            Err(app::state::IncrError::Load(e)) => {
                present::state_load_failed(&slug, &e);
                ExitCode::from(&e)
            }
            Err(app::state::IncrError::Save(e)) => {
                present::state_save_failed(&slug, &e);
                ExitCode::from(&e)
            }
        },
        StateCommands::Schema => match app::state::schema() {
            Ok(output) => {
                present::line(output);
                ExitCode::Success
            }
            Err(app::state::SchemaError::InvalidExample(e)) => {
                present::error(e);
                ExitCode::Internal
            }
            Err(app::state::SchemaError::Serialize(e)) => {
                present::error(e);
                ExitCode::Internal
            }
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn run_worktree(
    command: WorktreeCommands,
    repo_root: &Path,
    state_repo: &dyn StateRepository,
    git: &dyn crate::ports::git::GitRepository,
    fs: &dyn crate::ports::worktree_fs::WorktreeFs,
    clock: &dyn crate::ports::clock::Clock,
) -> ExitCode {
    match command {
        WorktreeCommands::Add { slug } => {
            match app::worktree::add(repo_root, state_repo, git, fs, clock, &slug) {
                Ok(worktree_value) => {
                    present::line(worktree_value);
                    ExitCode::Success
                }
                Err(app::worktree::AddError::NoState) => {
                    present::no_state_for_add(&slug);
                    ExitCode::Precondition
                }
                Err(app::worktree::AddError::Naming(e)) => {
                    present::error(e);
                    ExitCode::Precondition
                }
                Err(app::worktree::AddError::Fs(e)) => {
                    present::error(e);
                    ExitCode::Internal
                }
                Err(app::worktree::AddError::Git(e)) => {
                    present::error(&e);
                    ExitCode::from(&e)
                }
                Err(app::worktree::AddError::Load(e)) => {
                    present::state_load_failed(&slug, &e);
                    ExitCode::from(&e)
                }
                Err(app::worktree::AddError::Save(e)) => {
                    present::state_save_failed(&slug, &e);
                    ExitCode::from(&e)
                }
            }
        }
        WorktreeCommands::Remove { slug } => {
            match app::worktree::remove(repo_root, state_repo, git, clock, &slug) {
                Ok(()) => ExitCode::Success,
                Err(app::worktree::RemoveError::NoState) => {
                    present::no_state_for_remove(&slug);
                    ExitCode::Precondition
                }
                Err(app::worktree::RemoveError::Naming(e)) => {
                    present::error(e);
                    ExitCode::Precondition
                }
                Err(app::worktree::RemoveError::NotMerged {
                    branch,
                    main_branch,
                    verification_error,
                }) => {
                    present::not_merged(&branch, &main_branch, verification_error.as_deref());
                    ExitCode::Precondition
                }
                Err(app::worktree::RemoveError::Git(e)) => {
                    present::error(&e);
                    ExitCode::from(&e)
                }
                Err(app::worktree::RemoveError::Load(e)) => {
                    present::state_load_failed(&slug, &e);
                    ExitCode::from(&e)
                }
                Err(app::worktree::RemoveError::Save(e)) => {
                    present::state_save_failed(&slug, &e);
                    ExitCode::from(&e)
                }
            }
        }
        WorktreeCommands::Cleanup { dry_run } => {
            match app::worktree::cleanup(repo_root, state_repo, git, fs, clock, dry_run) {
                Ok(outcomes) => {
                    let mut any_failed = false;
                    for outcome in &outcomes {
                        if let app::worktree::CleanupOutcome::Failed { .. } = outcome {
                            any_failed = true;
                        }
                        present::cleanup_outcome(outcome);
                    }
                    if any_failed {
                        ExitCode::Git
                    } else {
                        ExitCode::Success
                    }
                }
                Err(app::worktree::CleanupError::Fs(e)) => {
                    present::error(e);
                    ExitCode::Internal
                }
                Err(app::worktree::CleanupError::Git(e)) => {
                    present::error(&e);
                    ExitCode::from(&e)
                }
            }
        }
    }
}

fn run_validation(
    command: ValidationCommands,
    src: &dyn crate::ports::validation_source::ValidationSource,
) -> ExitCode {
    match command {
        ValidationCommands::Resolve { paths } => {
            if paths.is_empty() {
                present::error("at least one path is required");
                return ExitCode::Precondition;
            }

            match app::validation::resolve(src, &paths) {
                Ok(output) => {
                    present::validation_output(&output);
                    ExitCode::Success
                }
                Err(e) => {
                    present::validation_resolve_failed(&e);
                    ExitCode::from(&e)
                }
            }
        }
        ValidationCommands::Check { path } => match app::validation::check(src, &path) {
            Ok(true) => {
                present::validation_ok();
                ExitCode::Success
            }
            Ok(false) => {
                present::validation_missing();
                ExitCode::Precondition
            }
            Err(e) => {
                present::validation_check_failed(&e);
                ExitCode::from(&e)
            }
        },
    }
}

fn run_review(
    command: ReviewCommands,
    repo_root: &Path,
    state_repo: &dyn StateRepository,
    git: &dyn crate::ports::git::GitRepository,
) -> ExitCode {
    match command {
        ReviewCommands::Select { slug } => {
            match app::review::select(repo_root, state_repo, git, &slug) {
                Ok(lanes) => {
                    present::lane_list(&lanes);
                    ExitCode::Success
                }
                Err(app::review::SelectError::NoState) => {
                    present::no_state_for_review(&slug);
                    ExitCode::Precondition
                }
                Err(app::review::SelectError::NoBranch) => {
                    present::no_branch_for_review(&slug);
                    ExitCode::Precondition
                }
                Err(app::review::SelectError::Load(e)) => {
                    present::state_load_failed(&slug, &e);
                    ExitCode::from(&e)
                }
                Err(app::review::SelectError::NoRemoteDefault(e)) => {
                    present::no_remote_default_for_review(&slug, &e);
                    ExitCode::Precondition
                }
                Err(app::review::SelectError::Git(e)) => {
                    present::error(&e);
                    ExitCode::from(&e)
                }
            }
        }
    }
}

fn run_resume(slug: &str, repo: &dyn StateRepository) -> ExitCode {
    match app::resume::resume(repo, slug) {
        Ok(state) => {
            present::resume_summary(&state);
            ExitCode::Success
        }
        Err(e) => {
            present::state_load_failed(slug, &e);
            ExitCode::from(&e)
        }
    }
}

fn run_list(repo: &dyn StateRepository) -> ExitCode {
    match app::list::list(repo) {
        Ok(rows) => {
            present::list_summary(&rows);
            ExitCode::Success
        }
        Err(app::list::ListError::ListSlugs(e)) => {
            present::error(&e);
            ExitCode::from(&e)
        }
        Err(app::list::ListError::Load { slug, error }) => {
            present::state_load_failed(slug.as_ref(), &error);
            ExitCode::from(&error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::testing::{FakeGit, FakeWorktreeFs, FixedClock, InMemoryStateRepository};
    use crate::domain::state::{Stage, State};
    use crate::domain::value::{DateValue, ScoreWave};
    use crate::ports::git::GitError;
    use tempfile::TempDir;

    fn fixed_date() -> DateValue {
        DateValue::parse("today", "2026-01-01").expect("valid date")
    }

    fn fixed_clock() -> FixedClock {
        FixedClock(fixed_date())
    }

    #[test]
    fn state_init_succeeds_for_new_slug() {
        let repo = InMemoryStateRepository::new();
        let code = run_state(
            StateCommands::Init { slug: "foo".into() },
            &repo,
            &fixed_clock(),
        );
        assert_eq!(code, ExitCode::Success);
        assert_eq!(
            repo.get("foo").expect("state should exist").stage,
            Stage::Casing
        );
    }

    #[test]
    fn state_init_rejects_existing_slug() {
        let repo = InMemoryStateRepository::new()
            .with_state("foo", State::new("foo", fixed_date()).expect("valid slug"));
        let code = run_state(
            StateCommands::Init { slug: "foo".into() },
            &repo,
            &fixed_clock(),
        );
        assert_eq!(code, ExitCode::Precondition);
    }

    #[test]
    fn state_set_on_missing_slug_is_precondition() {
        let repo = InMemoryStateRepository::new();
        let code = run_state(
            StateCommands::Set {
                slug: "ghost".into(),
                field: "stage".into(),
                value: "done".into(),
            },
            &repo,
            &fixed_clock(),
        );
        assert_eq!(code, ExitCode::Precondition);
    }

    #[test]
    fn state_set_persists_valid_field() {
        let repo = InMemoryStateRepository::new()
            .with_state("foo", State::new("foo", fixed_date()).expect("valid slug"));
        let code = run_state(
            StateCommands::Set {
                slug: "foo".into(),
                field: "score_wave".into(),
                value: "4".into(),
            },
            &repo,
            &fixed_clock(),
        );
        assert_eq!(code, ExitCode::Success);
        assert_eq!(
            repo.get("foo").expect("state should exist").score_wave,
            ScoreWave::new(4)
        );
    }

    #[test]
    fn state_set_invalid_numeric_is_precondition_and_leaves_state() {
        let repo = InMemoryStateRepository::new()
            .with_state("foo", State::new("foo", fixed_date()).expect("valid slug"));
        let code = run_state(
            StateCommands::Set {
                slug: "foo".into(),
                field: "score_wave".into(),
                value: "not-a-number".into(),
            },
            &repo,
            &fixed_clock(),
        );
        assert_eq!(code, ExitCode::Precondition);
        assert_eq!(
            repo.get("foo").expect("state should exist").score_wave,
            ScoreWave::new(0)
        );
    }

    #[test]
    fn worktree_add_refuses_when_state_missing() {
        let temp_dir = TempDir::new().expect("failed to create temp directory");
        let repo = InMemoryStateRepository::new();
        let git = FakeGit::new();

        let code = run_worktree(
            WorktreeCommands::Add { slug: "foo".into() },
            temp_dir.path(),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
        );

        assert_eq!(code, ExitCode::Precondition);
    }

    #[test]
    fn worktree_add_fails_when_origin_unreachable() {
        let temp_dir = TempDir::new().expect("failed to create temp directory");
        let repo = InMemoryStateRepository::new()
            .with_state("foo", State::new("foo", fixed_date()).expect("valid slug"));
        let git = FakeGit::new().failing_add(GitError::WorktreeAdd {
            subtype: "origin-unreachable".into(),
            message: "cannot find remote ref".into(),
        });

        let code = run_worktree(
            WorktreeCommands::Add { slug: "foo".into() },
            temp_dir.path(),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
        );

        assert_eq!(code, ExitCode::Git);
        // State untouched: worktree/branch never populated.
        assert_eq!(repo.get("foo").expect("state should exist").worktree, None);
    }

    #[test]
    fn worktree_remove_refuses_when_state_missing() {
        let repo = InMemoryStateRepository::new();
        let git = FakeGit::new().with_default_branch("main");

        let code = run_worktree(
            WorktreeCommands::Remove { slug: "foo".into() },
            Path::new("."),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
        );

        assert_eq!(code, ExitCode::Precondition);
    }

    #[test]
    fn worktree_remove_refuses_when_branch_not_merged() {
        let repo = InMemoryStateRepository::new()
            .with_state("foo", State::new("foo", fixed_date()).expect("valid slug"));
        // No merged branch configured, so heist/foo is treated as unmerged.
        let git = FakeGit::new().with_default_branch("main");

        let code = run_worktree(
            WorktreeCommands::Remove { slug: "foo".into() },
            Path::new("."),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
        );

        assert_eq!(code, ExitCode::Precondition);
        // State must NOT advance to Done when the precondition fails.
        assert_eq!(
            repo.get("foo").expect("state should exist").stage,
            Stage::Casing
        );
    }

    #[test]
    fn worktree_remove_surfaces_worktree_removal_failure() {
        let repo = InMemoryStateRepository::new()
            .with_state("foo", State::new("foo", fixed_date()).expect("valid slug"));
        let git = FakeGit::new()
            .with_merged_branch("heist/foo")
            .failing_remove(GitError::WorktreeRemove {
                message: "worktree is dirty".into(),
            });

        let code = run_worktree(
            WorktreeCommands::Remove { slug: "foo".into() },
            Path::new("."),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
        );

        assert_eq!(code, ExitCode::Git);
        // Failing mid-teardown must not strand the state at Done.
        assert_eq!(
            repo.get("foo").expect("state should exist").stage,
            Stage::Casing
        );
    }

    #[test]
    fn worktree_remove_surfaces_branch_deletion_failure() {
        let repo = InMemoryStateRepository::new()
            .with_state("foo", State::new("foo", fixed_date()).expect("valid slug"));
        let git = FakeGit::new()
            .with_merged_branch("heist/foo")
            .failing_delete(GitError::BranchDelete {
                message: "not fully merged".into(),
            });

        let code = run_worktree(
            WorktreeCommands::Remove { slug: "foo".into() },
            Path::new("."),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
        );

        assert_eq!(code, ExitCode::Git);
        assert_eq!(
            repo.get("foo").expect("state should exist").stage,
            Stage::Casing
        );
    }

    #[test]
    fn worktree_remove_marks_done_when_merged() {
        let repo = InMemoryStateRepository::new()
            .with_state("foo", State::new("foo", fixed_date()).expect("valid slug"));
        let git = FakeGit::new()
            .with_default_branch("main")
            .with_merged_branch("heist/foo");

        let code = run_worktree(
            WorktreeCommands::Remove { slug: "foo".into() },
            Path::new("."),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
        );

        assert_eq!(code, ExitCode::Success);
        assert_eq!(
            repo.get("foo").expect("state should exist").stage,
            Stage::Done
        );
    }

    #[test]
    fn worktree_cleanup_returns_success_when_nothing_failed() {
        let repo = InMemoryStateRepository::new();
        let git = FakeGit::new().with_default_branch("main");

        let code = run_worktree(
            WorktreeCommands::Cleanup { dry_run: false },
            Path::new("."),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
        );

        assert_eq!(code, ExitCode::Success);
    }

    #[test]
    fn worktree_cleanup_returns_git_exit_code_on_item_failure() {
        let repo = InMemoryStateRepository::new();
        let git = FakeGit::new()
            .with_default_branch("main")
            .with_merged_branch("heist/foo")
            .with_worktree_info("/foo-repo/.worktrees/foo", Some("heist/foo"))
            .failing_remove(GitError::WorktreeRemove {
                message: "worktree is dirty".into(),
            });

        let code = run_worktree(
            WorktreeCommands::Cleanup { dry_run: false },
            Path::new("/foo-repo"),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
        );

        assert_eq!(code, ExitCode::Git);
    }

    #[test]
    fn worktree_cleanup_returns_git_exit_code_when_origin_unresolvable() {
        let repo = InMemoryStateRepository::new();
        let git = FakeGit::new()
            .with_default_branch("main")
            .failing_remote_default_resolve(GitError::MergeCheck {
                message: "cannot find remote ref origin/main".into(),
            });

        let code = run_worktree(
            WorktreeCommands::Cleanup { dry_run: false },
            Path::new("."),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
        );

        assert_eq!(code, ExitCode::Git);
    }
}

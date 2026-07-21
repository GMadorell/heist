pub mod exit_code;
mod present;

use crate::adapters::file_heist_dir_repository::FileHeistDirRepository;
use crate::adapters::file_score_repository::FileScoreRepository;
use crate::adapters::file_state_repository::FileStateRepository;
use crate::adapters::filesystem_worktree::FilesystemWorktree;
use crate::adapters::real_git::RealGit;
use crate::adapters::system_clock::SystemClock;
use crate::adapters::validation_fs::ValidationFs;
use crate::app;
use crate::ports::clock::Clock;
use crate::ports::git::GitRepository;
use crate::ports::heist_dir_repository::HeistDirRepository;
use crate::ports::score_repository::ScoreRepository;
use crate::ports::state_repository::StateRepository;
use crate::ports::worktree_fs::WorktreeFs;
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
Exit codes: 0 success, 1 internal error, 2 precondition failed, 3 git command failed, 4 invalid path argument, 5 <abandoned-base halt: base PR closed unmerged, human decision required>."
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
    /// Print a short summary (stage, next, worktree) for picking a heist back up
    Resume {
        /// Heist slug (directory name under .heist/)
        slug: String,
    },
    /// Print one line per heist under .heist/, sorted by slug
    List,
    /// Resolve a heist's base: prints `resolution:` (null|live|expired|abandoned), `merge_ref:`, and `pr_base:`
    Base {
        /// Heist slug (directory name under .heist/)
        slug: String,
    },
    /// Rebase or merge onto the recorded base, per `heist base`'s resolution; the only place this heist ever runs `git rebase`/`git merge`
    Sync {
        /// Heist slug (directory name under .heist/)
        slug: String,
    },
    /// Check required tools (git, gh, crit) are on PATH; exit 2 if any is missing
    Doctor,
    /// Atomically init state, set mode, create the worktree, and advance to planning; rolls back on failure
    Begin {
        /// Heist slug (directory name under .heist/)
        slug: String,
        /// Pipeline mode: heavy, medium, or light
        #[arg(long)]
        mode: String,
        /// Git ref to use as start point instead of origin/<default>
        #[arg(long)]
        base: Option<String>,
    },
    /// Parse/check/dispatch a heist's score.md (the Forger's work order)
    Score {
        #[command(subcommand)]
        command: ScoreCommands,
    },
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
        /// Git ref to use as start point instead of origin/<default>
        #[arg(long)]
        base: Option<String>,
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

#[derive(Subcommand)]
enum ScoreCommands {
    /// Parse + cross-check score.md; prints ok/steps:/waves: on success, findings to stderr and exit 2 otherwise
    Check {
        /// Heist slug (directory name under .heist/)
        slug: String,
    },
    /// Like Check, then persists score_steps_total/score_waves_total into state.json and bumps updated
    Record {
        /// Heist slug (directory name under .heist/)
        slug: String,
    },
    /// Print one wave's steps verbatim: steps: K header then --- step N --- delimited raw blocks
    Wave {
        /// Heist slug (directory name under .heist/)
        slug: String,
        /// Wave number to print
        n: u32,
    },
}

pub fn run(cli: Cli) -> ExitCode {
    let heist_dir_repo = FileHeistDirRepository;
    let state_repo = FileStateRepository;
    let score_repo = FileScoreRepository;
    let git = RealGit;
    let fs = FilesystemWorktree;
    let clock = SystemClock;
    let validation_src = ValidationFs;
    let tool_probe = crate::adapters::real_tool_probe::RealToolProbe;
    let repo_root = Path::new(".");

    match cli.command {
        Commands::State { command } => run_state(command, &heist_dir_repo, &state_repo, &clock),
        Commands::Worktree { command } => {
            run_worktree(command, repo_root, &state_repo, &git, &fs, &clock)
        }
        Commands::Validation { command } => run_validation(command, &validation_src),
        Commands::Review { command } => run_review(command, repo_root, &state_repo, &git),
        Commands::Resume { slug } => run_resume(&slug, &state_repo),
        Commands::List => run_list(&state_repo),
        Commands::Base { slug } => run_base(&slug, repo_root, &state_repo, &git),
        Commands::Sync { slug } => run_sync(&slug, &state_repo, &git),
        Commands::Doctor => run_doctor(&tool_probe),
        Commands::Begin { slug, mode, base } => run_begin(
            &slug,
            &mode,
            base.as_deref(),
            repo_root,
            &heist_dir_repo,
            &state_repo,
            &git,
            &fs,
            &clock,
        ),
        Commands::Score { command } => run_score(command, &state_repo, &score_repo, &clock),
    }
}

fn run_state(
    command: StateCommands,
    heist_dir_repo: &dyn HeistDirRepository,
    repo: &dyn StateRepository,
    clock: &dyn Clock,
) -> ExitCode {
    match command {
        StateCommands::Init { slug } => {
            match app::state::init(heist_dir_repo, repo, clock, &slug) {
                Ok(()) => ExitCode::Success,
                Err(app::state::InitError::InvalidSlug(e)) => {
                    present::error(e);
                    ExitCode::Precondition
                }
                Err(app::state::InitError::Init(e)) => {
                    present::state_init_failed(&slug, &e);
                    ExitCode::from(&e)
                }
            }
        }
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
                Err(e) => set_error_exit(&slug, e),
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
    git: &dyn GitRepository,
    fs: &dyn WorktreeFs,
    clock: &dyn Clock,
) -> ExitCode {
    match command {
        WorktreeCommands::Add { slug, base } => {
            match app::worktree::add(
                repo_root,
                state_repo,
                git,
                fs,
                clock,
                &slug,
                base.as_deref(),
            ) {
                Ok(worktree_value) => {
                    present::line(worktree_value);
                    ExitCode::Success
                }
                Err(e) => add_error_exit(&slug, e),
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
    git: &dyn GitRepository,
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

fn run_doctor(probe: &dyn crate::ports::tool_probe::ToolProbe) -> ExitCode {
    let results = app::doctor::doctor(probe);
    present::doctor(&results);
    if results.iter().all(|status| status.available) {
        ExitCode::Success
    } else {
        ExitCode::Precondition
    }
}

fn run_base(
    slug: &str,
    repo_root: &Path,
    state_repo: &dyn StateRepository,
    git: &dyn GitRepository,
) -> ExitCode {
    let main_branch = git.default_branch(repo_root);

    match app::base::resolve(repo_root, state_repo, git, slug) {
        Ok(app::base::BaseResolution::Null) => {
            present::base_resolution("null", &format!("origin/{}", main_branch), &main_branch);
            ExitCode::Success
        }
        Ok(app::base::BaseResolution::Live { base_ref }) => {
            present::base_resolution("live", base_ref.as_ref(), base_ref.as_ref());
            ExitCode::Success
        }
        Ok(app::base::BaseResolution::Expired { base_ref }) => {
            present::base_resolution_expired(
                &format!("origin/{}", main_branch),
                &main_branch,
                base_ref.as_ref(),
            );
            ExitCode::Success
        }
        Ok(app::base::BaseResolution::Abandoned { base_ref }) => {
            present::abandoned_base(base_ref.as_ref());
            ExitCode::Precondition
        }
        Err(app::base::ResolveError::NoState) => {
            present::no_state_for_review(slug);
            ExitCode::Precondition
        }
        Err(app::base::ResolveError::Load(e)) => {
            present::state_load_failed(slug, &e);
            ExitCode::from(&e)
        }
        Err(app::base::ResolveError::RefMissingWithOpenPr { base_ref }) => {
            present::base_resolve_failed(&base_ref, "ref does not exist but PR is still open");
            ExitCode::Precondition
        }
        Err(app::base::ResolveError::RefMissingNoPr { base_ref }) => {
            present::base_resolve_failed(
                &base_ref,
                "ref does not exist and no PR was found; the base branch may have been deleted, re-check state.json's base field",
            );
            ExitCode::Precondition
        }
        Err(app::base::ResolveError::Ambiguous { base_ref }) => {
            present::base_resolve_failed(&base_ref, "cannot determine PR state");
            ExitCode::Precondition
        }
        Err(app::base::ResolveError::VerificationFailed { base_ref, message }) => {
            present::base_verification_failed(&base_ref, &message);
            ExitCode::Git
        }
    }
}

fn run_sync(slug: &str, state_repo: &dyn StateRepository, git: &dyn GitRepository) -> ExitCode {
    match app::sync::sync(state_repo, git, slug) {
        Ok(action) => {
            present::sync_action(&action);
            ExitCode::Success
        }
        Err(app::sync::SyncError::FetchFailed(e)) => {
            present::sync_fetch_failed(&e);
            ExitCode::from(&e)
        }
        Err(app::sync::SyncError::Abandoned { base_ref }) => {
            present::abandoned_base_sync_refused(&base_ref);
            ExitCode::AbandonedBase
        }
        Err(app::sync::SyncError::NotSetUp) => {
            present::sync_not_set_up(slug);
            ExitCode::Precondition
        }
        Err(app::sync::SyncError::WrongCheckout { expected, actual }) => {
            present::sync_wrong_checkout(slug, &expected, &actual);
            ExitCode::Precondition
        }
        Err(app::sync::SyncError::Git(e)) => {
            present::error(&e);
            ExitCode::from(&e)
        }
        Err(app::sync::SyncError::Resolve(app::base::ResolveError::NoState)) => {
            present::no_state_for_review(slug);
            ExitCode::Precondition
        }
        Err(app::sync::SyncError::Resolve(app::base::ResolveError::Load(e))) => {
            present::state_load_failed(slug, &e);
            ExitCode::from(&e)
        }
        Err(app::sync::SyncError::Resolve(app::base::ResolveError::RefMissingWithOpenPr {
            base_ref,
        })) => {
            present::base_resolve_failed(&base_ref, "ref does not exist but PR is still open");
            ExitCode::Precondition
        }
        Err(app::sync::SyncError::Resolve(app::base::ResolveError::RefMissingNoPr {
            base_ref,
        })) => {
            present::base_resolve_failed(
                &base_ref,
                "ref does not exist and no PR was found; the base branch may have been deleted, re-check state.json's base field",
            );
            ExitCode::Precondition
        }
        Err(app::sync::SyncError::Resolve(app::base::ResolveError::Ambiguous { base_ref })) => {
            present::base_resolve_failed(&base_ref, "cannot determine PR state");
            ExitCode::Precondition
        }
        Err(app::sync::SyncError::Resolve(app::base::ResolveError::VerificationFailed {
            base_ref,
            message,
        })) => {
            present::base_verification_failed(&base_ref, &message);
            ExitCode::Git
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_begin(
    slug: &str,
    mode: &str,
    base: Option<&str>,
    repo_root: &Path,
    heist_dir_repo: &dyn HeistDirRepository,
    state_repo: &dyn StateRepository,
    git: &dyn GitRepository,
    fs: &dyn WorktreeFs,
    clock: &dyn Clock,
) -> ExitCode {
    match app::begin::begin(
        repo_root,
        heist_dir_repo,
        state_repo,
        git,
        fs,
        clock,
        slug,
        mode,
        base,
    ) {
        Ok(worktree_value) => {
            present::line(worktree_value);
            ExitCode::Success
        }
        Err(app::begin::BeginError::InvalidSlug(e)) => {
            present::error(e);
            ExitCode::Precondition
        }
        Err(app::begin::BeginError::Collision(artifact)) => {
            present::slug_collision(slug, &artifact.describe(slug));
            ExitCode::Precondition
        }
        Err(app::begin::BeginError::Probe(e)) => {
            let exit_code = ExitCode::from(&e);
            present::error(e);
            exit_code
        }
        Err(app::begin::BeginError::Init(e)) => {
            present::state_init_failed(slug, &e);
            ExitCode::from(&e)
        }
        Err(app::begin::BeginError::State {
            error,
            rollback_errors,
        }) => {
            present::rollback_diagnostics(&rollback_errors);
            set_error_exit(slug, error)
        }
        Err(app::begin::BeginError::Worktree {
            error,
            rollback_errors,
        }) => {
            present::rollback_diagnostics(&rollback_errors);
            add_error_exit(slug, error)
        }
    }
}

/// Shared `SetError` -> presented message / exit code mapping, used by both
/// the standalone `state set` command and `begin`'s mode/stage-set steps.
fn set_error_exit(slug: &str, error: app::state::SetError) -> ExitCode {
    match error {
        app::state::SetError::Field(e) => {
            present::error(e);
            ExitCode::Precondition
        }
        app::state::SetError::Load(e) => {
            present::state_load_failed(slug, &e);
            ExitCode::from(&e)
        }
        app::state::SetError::Save(e) => {
            present::state_save_failed(slug, &e);
            ExitCode::from(&e)
        }
    }
}

/// Shared `AddError` -> presented message / exit code mapping, used by both
/// the standalone `worktree add` command and `begin`'s worktree-add step.
fn add_error_exit(slug: &str, error: app::worktree::AddError) -> ExitCode {
    match error {
        app::worktree::AddError::NoState => {
            present::no_state_for_add(slug);
            ExitCode::Precondition
        }
        app::worktree::AddError::Naming(e) => {
            present::error(e);
            ExitCode::Precondition
        }
        app::worktree::AddError::Fs(e) => {
            present::error(e);
            ExitCode::Internal
        }
        app::worktree::AddError::Git(e) => {
            present::error(&e);
            ExitCode::from(&e)
        }
        app::worktree::AddError::Load(e) => {
            present::state_load_failed(slug, &e);
            ExitCode::from(&e)
        }
        app::worktree::AddError::Save(e) => {
            present::state_save_failed(slug, &e);
            ExitCode::from(&e)
        }
        app::worktree::AddError::BaseImmutable {
            existing,
            requested,
        } => {
            present::base_immutable(slug, existing.as_deref(), &requested);
            ExitCode::Precondition
        }
    }
}

fn run_score(
    command: ScoreCommands,
    state_repo: &dyn StateRepository,
    score_repo: &dyn ScoreRepository,
    clock: &dyn crate::ports::clock::Clock,
) -> ExitCode {
    match command {
        ScoreCommands::Check { slug } => match app::score::check(state_repo, score_repo, &slug) {
            Ok(outcome) => {
                present::score_check_ok(outcome.steps, outcome.waves);
                ExitCode::Success
            }
            Err(app::score::CheckError::NoState) => {
                present::no_state_for_score(&slug);
                ExitCode::Precondition
            }
            Err(app::score::CheckError::NoScore) => {
                present::no_score_for_slug(&slug);
                ExitCode::Precondition
            }
            Err(app::score::CheckError::Io(e)) => {
                present::score_io_failed(&slug, &e);
                ExitCode::Internal
            }
            Err(app::score::CheckError::Findings(findings)) => {
                present::score_findings(&findings);
                ExitCode::Precondition
            }
        },
        ScoreCommands::Record { slug } => match app::score::record(state_repo, score_repo, clock, &slug) {
            Ok(outcome) => {
                present::score_record_ok(outcome.steps, outcome.waves);
                ExitCode::Success
            }
            Err(app::score::RecordError::NoState) => {
                present::no_state_for_score(&slug);
                ExitCode::Precondition
            }
            Err(app::score::RecordError::NoScore) => {
                present::no_score_for_slug(&slug);
                ExitCode::Precondition
            }
            Err(app::score::RecordError::Io(e)) => {
                present::score_io_failed(&slug, &e);
                ExitCode::Internal
            }
            Err(app::score::RecordError::Findings(findings)) => {
                present::score_findings(&findings);
                ExitCode::Precondition
            }
            Err(app::score::RecordError::Save(e)) => {
                present::state_save_failed(&slug, &e);
                ExitCode::from(&e)
            }
        },
        ScoreCommands::Wave { slug, n } => match app::score::wave(state_repo, score_repo, &slug, n) {
            Ok(blocks) => {
                present::score_wave_blocks(&blocks);
                ExitCode::Success
            }
            Err(app::score::WaveError::NoState) => {
                present::no_state_for_score(&slug);
                ExitCode::Precondition
            }
            Err(app::score::WaveError::NoScore) => {
                present::no_score_for_slug(&slug);
                ExitCode::Precondition
            }
            Err(app::score::WaveError::Io(e)) => {
                present::score_io_failed(&slug, &e);
                ExitCode::Internal
            }
            Err(app::score::WaveError::Findings(findings)) => {
                present::score_findings(&findings);
                ExitCode::Precondition
            }
            Err(app::score::WaveError::NoSuchWave(n)) => {
                present::score_no_such_wave(&slug, n);
                ExitCode::Precondition
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::testing::{
        FakeGit, FakeWorktreeFs, FixedClock, InMemoryHeistDirRepository, InMemoryStateRepository,
    };
    use crate::domain::state::{Stage, State};
    use crate::domain::value::{DateValue, NonBlankValue, ScoreWave};
    use crate::ports::git::{GitError, PrState};
    use tempfile::TempDir;

    fn fixed_date() -> DateValue {
        DateValue::parse("today", "2026-01-01").expect("valid date")
    }

    fn fixed_clock() -> FixedClock {
        FixedClock(fixed_date())
    }

    #[test]
    fn state_init_succeeds_for_new_slug() {
        let heist_dir_repo = InMemoryHeistDirRepository::new();
        let repo = InMemoryStateRepository::new();
        let code = run_state(
            StateCommands::Init { slug: "foo".into() },
            &heist_dir_repo,
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
        let heist_dir_repo = InMemoryHeistDirRepository::new().with_dir("foo");
        let repo = InMemoryStateRepository::new()
            .with_state("foo", State::new("foo", fixed_date()).expect("valid slug"));
        let code = run_state(
            StateCommands::Init { slug: "foo".into() },
            &heist_dir_repo,
            &repo,
            &fixed_clock(),
        );
        assert_eq!(code, ExitCode::Precondition);
    }

    #[test]
    fn state_set_on_missing_slug_is_precondition() {
        let heist_dir_repo = InMemoryHeistDirRepository::new();
        let repo = InMemoryStateRepository::new();
        let code = run_state(
            StateCommands::Set {
                slug: "ghost".into(),
                field: "stage".into(),
                value: "done".into(),
            },
            &heist_dir_repo,
            &repo,
            &fixed_clock(),
        );
        assert_eq!(code, ExitCode::Precondition);
    }

    #[test]
    fn state_set_persists_valid_field() {
        let heist_dir_repo = InMemoryHeistDirRepository::new();
        let repo = InMemoryStateRepository::new()
            .with_state("foo", State::new("foo", fixed_date()).expect("valid slug"));
        let code = run_state(
            StateCommands::Set {
                slug: "foo".into(),
                field: "score_wave".into(),
                value: "4".into(),
            },
            &heist_dir_repo,
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
        let heist_dir_repo = InMemoryHeistDirRepository::new();
        let repo = InMemoryStateRepository::new()
            .with_state("foo", State::new("foo", fixed_date()).expect("valid slug"));
        let code = run_state(
            StateCommands::Set {
                slug: "foo".into(),
                field: "score_wave".into(),
                value: "not-a-number".into(),
            },
            &heist_dir_repo,
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
            WorktreeCommands::Add {
                slug: "foo".into(),
                base: None,
            },
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
            WorktreeCommands::Add {
                slug: "foo".into(),
                base: None,
            },
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

    #[test]
    fn base_command_reports_abandoned_as_precondition_exit_code() {
        let mut state = State::new("foo", fixed_date()).expect("valid slug");
        state.base = Some(NonBlankValue::parse("base", "heist/piece-01").expect("valid base"));

        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new().with_pr_state("heist/piece-01", PrState::ClosedUnmerged);

        let code = run_base("foo", Path::new("."), &repo, &git);

        assert_eq!(code, ExitCode::Precondition);
    }

    #[test]
    fn sync_command_refuses_abandoned_base_with_abandoned_exit_code() {
        let mut state = State::new("foo", fixed_date()).expect("valid slug");
        state.worktree = Some(NonBlankValue::parse("worktree", "/tmp/wt").expect("valid worktree"));
        state.branch = Some(NonBlankValue::parse("branch", "heist/foo").expect("valid branch"));
        state.base = Some(NonBlankValue::parse("base", "heist/piece-01").expect("valid base"));

        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new()
            .with_current_branch("heist/foo")
            .with_pr_state("heist/piece-01", PrState::ClosedUnmerged);

        let code = run_sync("foo", &repo, &git);

        assert_eq!(code, ExitCode::AbandonedBase);
    }

    #[test]
    fn begin_happy_path_returns_success_and_advances_to_planning() {
        let temp_dir = TempDir::new().expect("failed to create temp directory");
        let heist_dir_repo = InMemoryHeistDirRepository::new();
        let repo = InMemoryStateRepository::new();
        let git = FakeGit::new().with_default_branch("main");

        let code = run_begin(
            "foo",
            "heavy",
            None,
            temp_dir.path(),
            &heist_dir_repo,
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
        );

        assert_eq!(code, ExitCode::Success);
        let state = repo.get("foo").expect("state should exist");
        assert_eq!(state.stage, Stage::Planning);
    }

    #[test]
    fn begin_collision_returns_precondition_exit_code() {
        let temp_dir = TempDir::new().expect("failed to create temp directory");
        let heist_dir_repo = InMemoryHeistDirRepository::new();
        let repo = InMemoryStateRepository::new()
            .with_state("foo", State::new("foo", fixed_date()).expect("valid slug"));
        let git = FakeGit::new().with_default_branch("main");

        let code = run_begin(
            "foo",
            "heavy",
            None,
            temp_dir.path(),
            &heist_dir_repo,
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
        );

        assert_eq!(code, ExitCode::Precondition);
    }

    #[test]
    fn score_check_reports_findings_as_precondition_exit_code() {
        let malformed_score = "\
## Wave 1

### Step 1: add widget
- **Wave**: 1
- **Change**: add widget.
- **Verify**: cargo build
- Depends on: none
";
        let repo = InMemoryStateRepository::new()
            .with_state("foo", State::new("foo", fixed_date()).expect("valid slug"))
            .with_score("foo", malformed_score);

        let code = run_score(
            ScoreCommands::Check { slug: "foo".into() },
            &repo,
            &repo,
            &fixed_clock(),
        );

        assert_eq!(code, ExitCode::Precondition);
    }

    #[test]
    fn score_record_persists_totals_and_returns_success() {
        let valid_score = "\
## Wave 1

### Step 1: add widget
- **Wave**: 1
- **Files**: /tmp/a.rs
- **Change**: add widget.
- **Verify**: cargo build
- Depends on: none
";
        let repo = InMemoryStateRepository::new()
            .with_state("foo", State::new("foo", fixed_date()).expect("valid slug"))
            .with_score("foo", valid_score);

        let code = run_score(
            ScoreCommands::Record { slug: "foo".into() },
            &repo,
            &repo,
            &fixed_clock(),
        );

        assert_eq!(code, ExitCode::Success);
        let saved = repo.get("foo").expect("state should exist");
        assert_eq!(saved.score_steps_total.to_string(), "1");
        assert_eq!(saved.score_waves_total.to_string(), "1");
    }

    #[test]
    fn score_wave_prints_blocks_and_reports_no_such_wave() {
        let valid_score = "\
## Wave 1

### Step 1: add widget
- **Wave**: 1
- **Files**: /tmp/a.rs
- **Change**: add widget.
- **Verify**: cargo build
- Depends on: none
";
        let repo = InMemoryStateRepository::new()
            .with_state("foo", State::new("foo", fixed_date()).expect("valid slug"))
            .with_score("foo", valid_score);

        let ok_code = run_score(
            ScoreCommands::Wave {
                slug: "foo".into(),
                n: 1,
            },
            &repo,
            &repo,
            &fixed_clock(),
        );
        assert_eq!(ok_code, ExitCode::Success);

        let missing_wave_code = run_score(
            ScoreCommands::Wave {
                slug: "foo".into(),
                n: 2,
            },
            &repo,
            &repo,
            &fixed_clock(),
        );
        assert_eq!(missing_wave_code, ExitCode::Precondition);
    }
}

use crate::adapters::file_state_repository::FileStateRepository;
use crate::adapters::filesystem_worktree::FilesystemWorktree;
use crate::adapters::real_git::RealGit;
use crate::adapters::system_clock::SystemClock;
use crate::adapters::validation_fs::ValidationFs;
use crate::domain;
use crate::domain::state::State;
use crate::domain::value::{DateValue, NonBlankValue};
use crate::exitcode::ExitCode;
use crate::ports::clock::Clock;
use crate::ports::git::GitRepository;
use crate::ports::state_repository::StateRepository;
use crate::ports::worktree_fs::WorktreeFs;
use clap::{Parser, Subcommand};
use std::path::Path;

#[derive(Parser)]
#[command(name = "heist-cli")]
#[command(about = "Heist CLI tool", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    State {
        #[command(subcommand)]
        command: StateCommands,
    },
    Worktree {
        #[command(subcommand)]
        command: WorktreeCommands,
    },
    Validation {
        #[command(subcommand)]
        command: ValidationCommands,
    },
    Resume {
        slug: String,
    },
}

#[derive(Subcommand)]
enum StateCommands {
    Init {
        slug: String,
    },
    Get {
        slug: String,
        field: String,
    },
    Set {
        slug: String,
        field: String,
        value: String,
    },
    Schema,
}

#[derive(Subcommand)]
enum WorktreeCommands {
    Add { slug: String },
    Remove { slug: String },
}

#[derive(Subcommand)]
enum ValidationCommands {
    Resolve { paths: Vec<std::path::PathBuf> },
    Check { path: std::path::PathBuf },
}

pub fn run(cli: Cli) -> ExitCode {
    let state_repo = FileStateRepository;
    let git = RealGit;
    let clock = SystemClock;
    let repo_root = Path::new(".");

    match cli.command {
        Commands::State { command } => {
            run_state(command, &state_repo, &clock).unwrap_or_else(|v| v)
        }
        Commands::Worktree { command } => {
            run_worktree(command, repo_root, &state_repo, &git, &clock).unwrap_or_else(|v| v)
        }
        Commands::Validation { command } => run_validation(command),
        Commands::Resume { slug } => run_resume(&slug, &state_repo).unwrap_or_else(|v| v),
    }
}

fn run_state(
    command: StateCommands,
    repo: &dyn StateRepository,
    clock: &dyn Clock,
) -> Result<ExitCode, ExitCode> {
    match command {
        StateCommands::Init { slug } => {
            let state = match State::new(&slug, clock.today()) {
                Ok(state) => state,
                Err(e) => {
                    eprintln!("{}", e);
                    return Ok(ExitCode::Precondition);
                }
            };
            match repo.init(&slug, &state) {
                Ok(()) => Ok(ExitCode::Success),
                Err(e) => {
                    eprintln!("failed to init state for slug {}: {}", slug, e);
                    Err(e.exit_code())
                }
            }
        }
        StateCommands::Get { slug, field } => {
            let state = load_state(repo, &slug)?;
            match state.get_field(&field) {
                Ok(value) => {
                    println!("{}", value);
                    Ok(ExitCode::Success)
                }
                Err(e) => {
                    eprintln!("{}", e);
                    Ok(ExitCode::Precondition)
                }
            }
        }
        StateCommands::Set { slug, field, value } => {
            let mut state = load_state(repo, &slug)?;
            if let Err(e) = state.set_field(&field, &value) {
                eprintln!("{}", e);
                return Ok(ExitCode::Precondition);
            }
            state.updated = clock.today();
            save_state(repo, &slug, &state)?;
            Ok(ExitCode::Success)
        }
        StateCommands::Schema => {
            let field_list = "schema_version: u32\n\
slug: string\n\
stage: string (casing|planning|fence_review|human_review|forging|safehouse|implementing|cleaning|done)\n\
worktree: string|null\n\
branch: string|null\n\
score_step: u32\n\
score_steps_total: u32\n\
fence_rounds: u32\n\
created: string\n\
updated: string";

            let example_date =
                DateValue::parse("created", "2026-01-01").expect("constant date is valid");
            let example =
                State::new("example", example_date).map_err(|e| internal_error(&e))?;
            let json = match serde_json::to_string_pretty(&example) {
                Ok(json) => json,
                Err(e) => {
                    eprintln!("failed to serialize state: {}", e);
                    return Err(ExitCode::Internal);
                }
            };

            println!("{}\n\n{}", field_list, json);
            Ok(ExitCode::Success)
        }
    }
}

fn run_worktree(
    command: WorktreeCommands,
    repo_root: &Path,
    state_repo: &dyn StateRepository,
    git: &dyn GitRepository,
    clock: &dyn Clock,
) -> Result<ExitCode, ExitCode> {
    let fs = FilesystemWorktree;
    match command {
        WorktreeCommands::Add { slug } => {
            if !state_repo.exists(&slug) {
                eprintln!("no state found for slug {}; run `state init` first", slug);
                return Ok(ExitCode::Precondition);
            }

            let main_branch = git.default_branch(repo_root);
            fs.ensure_worktrees_ignored(repo_root)
                .map_err(|e| internal_error(&e))?;

            let worktree_path = domain::worktree::worktree_path(repo_root, &slug);
            let branch = domain::worktree::branch_name(&slug).map_err(|e| internal_error(&e))?;

            if !git.worktree_exists(repo_root, &slug) {
                if let Err(e) = git.add_worktree(
                    repo_root,
                    &worktree_path,
                    branch.as_ref(),
                    &format!("origin/{}", main_branch),
                ) {
                    eprintln!("{}", e);
                    return Err(e.exit_code());
                }
            }

            fs.link_heist_dir(repo_root, &worktree_path, &slug)
                .map_err(|e| internal_error(&e))?;

            let worktree_absolute = fs
                .canonicalize(&worktree_path)
                .map_err(|e| internal_error(&e))?;
            let worktree_value =
                NonBlankValue::parse("worktree", &worktree_absolute.to_string_lossy())
                    .map_err(|e| internal_error(&e))?;

            let mut state = load_state(state_repo, &slug)?;
            state.worktree = Some(worktree_value.clone());
            state.branch = Some(branch);
            state.updated = clock.today();
            save_state(state_repo, &slug, &state)?;

            println!("{}", worktree_value);
            Ok(ExitCode::Success)
        }
        WorktreeCommands::Remove { slug } => {
            if !state_repo.exists(&slug) {
                eprintln!("no state found for slug {}", slug);
                return Ok(ExitCode::Precondition);
            }

            let main_branch = git.default_branch(repo_root);
            let branch = domain::worktree::branch_name(&slug).map_err(|e| internal_error(&e))?;

            match git.is_branch_merged(repo_root, branch.as_ref(), &main_branch) {
                Ok(true) => {}
                Ok(false) => {
                    eprintln!("branch {} is not merged into {}", branch, main_branch);
                    return Ok(ExitCode::Precondition);
                }
                Err(e) => {
                    eprintln!("{}", e);
                    return Err(e.exit_code());
                }
            }

            let worktree_path = domain::worktree::worktree_path(repo_root, &slug);
            if let Err(e) = git.remove_worktree(repo_root, &worktree_path) {
                eprintln!("{}", e);
                return Err(e.exit_code());
            }
            if let Err(e) = git.delete_branch(repo_root, branch.as_ref()) {
                eprintln!("{}", e);
                return Err(e.exit_code());
            }

            // Remote branch deletion is intentionally out of scope: the branch
            // is often never pushed, or GitHub's auto-delete-on-merge already
            // handled it, and failing there would strand state.json short of "done".

            let mut state = load_state(state_repo, &slug)?;
            state.stage = crate::domain::state::Stage::Done;
            state.updated = clock.today();
            save_state(state_repo, &slug, &state)?;

            Ok(ExitCode::Success)
        }
    }
}

fn run_validation(command: ValidationCommands) -> ExitCode {
    let src = ValidationFs;
    match command {
        ValidationCommands::Resolve { paths } => {
            if paths.is_empty() {
                eprintln!("at least one path is required");
                return ExitCode::Precondition;
            }

            let result = if paths.len() == 1 {
                domain::validation::resolve_validation(&src, &paths[0])
            } else {
                domain::validation::resolve_validations(&src, &paths)
            };

            match result {
                Ok(output) => {
                    print!("{}", output);
                    ExitCode::Success
                }
                Err(e) => {
                    eprintln!("failed to resolve validation: {}", e);
                    ExitCode::Internal
                }
            }
        }
        ValidationCommands::Check { path } => {
            match domain::validation::check_validation_exists(&src, &path) {
                Ok(true) => {
                    println!("ok");
                    ExitCode::Success
                }
                Ok(false) => {
                    println!("missing");
                    ExitCode::Precondition
                }
                Err(e) => {
                    eprintln!("failed to check validation: {}", e);
                    ExitCode::Internal
                }
            }
        }
    }
}

fn run_resume(slug: &str, repo: &dyn StateRepository) -> Result<ExitCode, ExitCode> {
    let state = load_state(repo, slug)?;

    let next_step = match state.stage.next_step() {
        Some(stage) => stage.as_str().to_string(),
        None => "none".to_string(),
    };
    let worktree = state
        .worktree
        .as_ref()
        .map(|v| v.to_string())
        .unwrap_or_else(|| "none".to_string());

    println!("slug: {}", state.slug);
    println!("stage: {}", state.stage.as_str());
    println!("next_step: {}", next_step);
    println!("worktree: {}", worktree);

    Ok(ExitCode::Success)
}

fn load_state(repo: &dyn StateRepository, slug: &str) -> Result<State, ExitCode> {
    repo.load(slug).map_err(|e| {
        eprintln!("failed to load state for slug {}: {}", slug, e);
        e.exit_code()
    })
}

fn save_state(repo: &dyn StateRepository, slug: &str, state: &State) -> Result<(), ExitCode> {
    repo.save(slug, state).map_err(|e| {
        eprintln!("failed to save state for slug {}: {}", slug, e);
        e.exit_code()
    })
}

fn internal_error(e: &impl std::fmt::Display) -> ExitCode {
    eprintln!("{}", e);
    ExitCode::Internal
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::testing::{FakeGit, FixedClock, InMemoryStateRepository};
    use crate::domain::state::Stage;
    use crate::domain::value::ScoreStep;
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
        let code =
            run_state(StateCommands::Init { slug: "foo".into() }, &repo, &fixed_clock())
                .unwrap_or_else(|v| v);
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
        let code =
            run_state(StateCommands::Init { slug: "foo".into() }, &repo, &fixed_clock())
                .unwrap_or_else(|v| v);
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
        )
        .unwrap_or_else(|v| v);
        assert_eq!(code, ExitCode::Precondition);
    }

    #[test]
    fn state_set_persists_valid_field() {
        let repo = InMemoryStateRepository::new()
            .with_state("foo", State::new("foo", fixed_date()).expect("valid slug"));
        let code = run_state(
            StateCommands::Set {
                slug: "foo".into(),
                field: "score_step".into(),
                value: "4".into(),
            },
            &repo,
            &fixed_clock(),
        )
        .unwrap_or_else(|v| v);
        assert_eq!(code, ExitCode::Success);
        assert_eq!(
            repo.get("foo").expect("state should exist").score_step,
            ScoreStep::new(4)
        );
    }

    #[test]
    fn state_set_invalid_numeric_is_precondition_and_leaves_state() {
        let repo = InMemoryStateRepository::new()
            .with_state("foo", State::new("foo", fixed_date()).expect("valid slug"));
        let code = run_state(
            StateCommands::Set {
                slug: "foo".into(),
                field: "score_step".into(),
                value: "not-a-number".into(),
            },
            &repo,
            &fixed_clock(),
        )
        .unwrap_or_else(|v| v);
        assert_eq!(code, ExitCode::Precondition);
        assert_eq!(
            repo.get("foo").expect("state should exist").score_step,
            ScoreStep::new(0)
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
            &fixed_clock(),
        )
        .unwrap_or_else(|v| v);

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
            &fixed_clock(),
        )
        .unwrap_or_else(|v| v);

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
            &fixed_clock(),
        )
        .unwrap_or_else(|v| v);

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
            &fixed_clock(),
        )
        .unwrap_or_else(|v| v);

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
            &fixed_clock(),
        )
        .unwrap_or_else(|v| v);

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
            &fixed_clock(),
        )
        .unwrap_or_else(|v| v);

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
            &fixed_clock(),
        )
        .unwrap_or_else(|v| v);

        assert_eq!(code, ExitCode::Success);
        assert_eq!(
            repo.get("foo").expect("state should exist").stage,
            Stage::Done
        );
    }
}

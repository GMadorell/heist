use clap::{Parser, Subcommand};
use std::fs;
use std::path::Path;

mod exitcode;
mod git_repository;
mod state;
mod state_repository;
mod validation;
mod worktree;

use exitcode::ExitCode;
use git_repository::{GitRepository, RealGit};
use state::{today, State};
use state_repository::{FileStateRepository, StateRepository};

#[derive(Parser)]
#[command(name = "heist-cli")]
#[command(about = "Heist CLI tool", long_about = None)]
struct Cli {
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

/// Parse args, wire dependencies, dispatch to a command, exit with its code.
fn main() {
    let cli = Cli::parse();

    // Wire the real dependencies once; commands depend only on the traits.
    let state_repo = FileStateRepository;
    let git = RealGit;
    let repo_root = Path::new(".");

    let code = match cli.command {
        Commands::State { command } => run_state(command, &state_repo).collapse(),
        Commands::Worktree { command } => {
            run_worktree(command, repo_root, &state_repo, &git).collapse()
        }
        Commands::Validation { command } => run_validation(command),
        Commands::Resume { slug } => run_resume(&slug, &state_repo).collapse(),
    };
    code.exit();
}

fn run_state(command: StateCommands, repo: &dyn StateRepository) -> Result<ExitCode, ExitCode> {
    match command {
        StateCommands::Init { slug } => match repo.init(&slug, &State::new(&slug)) {
            Ok(()) => Ok(ExitCode::Success),
            Err(e) => {
                eprintln!("failed to init state for slug {}: {}", slug, e);
                Err(e.exit_code())
            }
        },
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
            state.updated = today();
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

            let example = State::new("example");
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
) -> Result<ExitCode, ExitCode> {
    match command {
        WorktreeCommands::Add { slug } => {
            let main_branch = git.default_branch(repo_root);
            worktree::ensure_worktrees_ignored(repo_root);

            let worktree_path = repo_root.join(".worktrees").join(&slug);
            let branch_name = format!("heist/{}", slug);

            if !git.worktree_exists(repo_root, &slug) {
                if let Err(e) = git.add_worktree(
                    repo_root,
                    &worktree_path,
                    &branch_name,
                    &format!("origin/{}", main_branch),
                ) {
                    eprintln!("{}", e);
                    return Err(e.exit_code());
                }
            }

            create_worktree_symlink(repo_root, &worktree_path, &slug);

            let worktree_absolute = worktree_path
                .canonicalize()
                .expect("failed to canonicalize worktree path");

            let mut state = load_state(state_repo, &slug)?;
            state.worktree = Some(worktree_absolute.to_string_lossy().to_string());
            state.branch = Some(branch_name);
            state.updated = today();
            save_state(state_repo, &slug, &state)?;

            println!("{}", worktree_absolute.display());
            Ok(ExitCode::Success)
        }
        WorktreeCommands::Remove { slug } => {
            let main_branch = git.default_branch(repo_root);
            let branch_name = format!("heist/{}", slug);

            match git.is_branch_merged(repo_root, &branch_name, &main_branch) {
                Ok(true) => {}
                Ok(false) => {
                    eprintln!("branch {} is not merged into {}", branch_name, main_branch);
                    return Ok(ExitCode::Precondition);
                }
                Err(e) => {
                    eprintln!("{}", e);
                    return Err(e.exit_code());
                }
            }

            let worktree_path = repo_root.join(".worktrees").join(&slug);
            if let Err(e) = git.remove_worktree(repo_root, &worktree_path) {
                eprintln!("{}", e);
                return Err(e.exit_code());
            }
            if let Err(e) = git.delete_branch(repo_root, &branch_name) {
                eprintln!("{}", e);
                return Err(e.exit_code());
            }

            // Remote branch deletion is intentionally out of scope: the branch
            // is often never pushed, or GitHub's auto-delete-on-merge already
            // handled it, and failing there would strand state.json short of "done".

            let mut state = load_state(state_repo, &slug)?;
            state.stage = state::Stage::Done;
            state.updated = today();
            save_state(state_repo, &slug, &state)?;

            Ok(ExitCode::Success)
        }
    }
}

fn run_validation(command: ValidationCommands) -> ExitCode {
    match command {
        ValidationCommands::Resolve { paths } => {
            if paths.is_empty() {
                eprintln!("at least one path is required");
                return ExitCode::Precondition;
            }

            let result = if paths.len() == 1 {
                validation::resolve_validation(&paths[0])
            } else {
                validation::resolve_validations(&paths)
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
        ValidationCommands::Check { path } => match validation::check_validation_exists(&path) {
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
        },
    }
}

fn run_resume(slug: &str, repo: &dyn StateRepository) -> Result<ExitCode, ExitCode> {
    let state = load_state(repo, slug)?;

    let next_step = match state.stage.next_step() {
        Some((number, name)) => format!("{} ({})", number, name),
        None => "none".to_string(),
    };
    let worktree = state.worktree.as_deref().unwrap_or("none");

    println!("slug: {}", state.slug);
    println!("stage: {}", state.stage.as_str());
    println!("next_step: {}", next_step);
    println!("worktree: {}", worktree);

    Ok(ExitCode::Success)
}

/// Point `<worktree>/.heist/<slug>` at the main repo's `.heist/<slug>` so state
/// is shared between the worktree and the main checkout.
fn create_worktree_symlink(repo_root: &Path, worktree_path: &Path, slug: &str) {
    let main_heist_canonical = repo_root
        .join(".heist")
        .join(slug)
        .canonicalize()
        .expect("failed to canonicalize main repo .heist/<slug>");

    let worktree_heist_dir = worktree_path.join(".heist");
    if !worktree_heist_dir.exists() {
        fs::create_dir_all(&worktree_heist_dir)
            .expect("failed to create .heist directory in worktree");
    }

    let worktree_heist_slug = worktree_heist_dir.join(slug);
    if worktree_heist_slug.exists() {
        fs::remove_file(&worktree_heist_slug).expect("failed to remove existing symlink");
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs as unix_fs;
        unix_fs::symlink(&main_heist_canonical, &worktree_heist_slug)
            .expect("failed to create symlink");
    }

    #[cfg(not(unix))]
    {
        let _ = main_heist_canonical;
        eprintln!("symlink creation not supported on this platform");
        ExitCode::Internal.exit();
    }
}

/// Load a slug's state, or print the error and yield the matching exit code.
fn load_state(repo: &dyn StateRepository, slug: &str) -> Result<State, ExitCode> {
    repo.load(slug).map_err(|e| {
        eprintln!("failed to load state for slug {}: {}", slug, e);
        e.exit_code()
    })
}

/// Persist a slug's state, or print the error and yield the matching exit code.
fn save_state(repo: &dyn StateRepository, slug: &str, state: &State) -> Result<(), ExitCode> {
    repo.save(slug, state).map_err(|e| {
        eprintln!("failed to save state for slug {}: {}", slug, e);
        e.exit_code()
    })
}

/// Extracts the value out of a `Result<T, T>`, whichever variant it is.
///
/// The stdlib has exactly this (`Result::into_ok_or_err`), but it's been
/// nightly-only since 2021 or so (rust-lang/rust#82223) and was never
/// stabilized, so command handlers that report their own errors via `?` and
/// converge on a single `ExitCode` need a small local stand-in.
trait Collapse<T> {
    fn collapse(self) -> T;
}

impl<T> Collapse<T> for Result<T, T> {
    fn collapse(self) -> T {
        match self {
            Ok(v) | Err(v) => v,
        }
    }
}

#[cfg(test)]
mod tests {
    //! In-process decision-logic tests for the command handlers.
    //!
    //! These exercise `run_state`/`run_worktree`'s branching against the
    //! in-memory `InMemoryStateRepository` and `FakeGit`, with no subprocess and
    //! no real git repo, so preconditions like "branch not merged" or "origin
    //! unreachable" are cheap to arrange and assert.

    use super::*;
    use git_repository::{FakeGit, GitError};
    use state::Stage;
    use state_repository::InMemoryStateRepository;
    use tempfile::TempDir;

    #[test]
    fn state_init_succeeds_for_new_slug() {
        let repo = InMemoryStateRepository::new();
        let code = run_state(StateCommands::Init { slug: "foo".into() }, &repo).collapse();
        assert_eq!(code, ExitCode::Success);
        assert_eq!(
            repo.get("foo").expect("state should exist").stage,
            Stage::Casing
        );
    }

    #[test]
    fn state_init_rejects_existing_slug() {
        let repo = InMemoryStateRepository::new().with_state("foo", State::new("foo"));
        let code = run_state(StateCommands::Init { slug: "foo".into() }, &repo).collapse();
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
        )
        .collapse();
        assert_eq!(code, ExitCode::Precondition);
    }

    #[test]
    fn state_set_persists_valid_field() {
        let repo = InMemoryStateRepository::new().with_state("foo", State::new("foo"));
        let code = run_state(
            StateCommands::Set {
                slug: "foo".into(),
                field: "score_step".into(),
                value: "4".into(),
            },
            &repo,
        )
        .collapse();
        assert_eq!(code, ExitCode::Success);
        assert_eq!(repo.get("foo").expect("state should exist").score_step, 4);
    }

    #[test]
    fn state_set_invalid_numeric_is_precondition_and_leaves_state() {
        let repo = InMemoryStateRepository::new().with_state("foo", State::new("foo"));
        let code = run_state(
            StateCommands::Set {
                slug: "foo".into(),
                field: "score_step".into(),
                value: "not-a-number".into(),
            },
            &repo,
        )
        .collapse();
        assert_eq!(code, ExitCode::Precondition);
        assert_eq!(repo.get("foo").expect("state should exist").score_step, 0);
    }

    #[test]
    fn worktree_remove_refuses_when_branch_not_merged() {
        let repo = InMemoryStateRepository::new().with_state("foo", State::new("foo"));
        // No merged branch configured, so heist/foo is treated as unmerged.
        let git = FakeGit::new().with_default_branch("main");

        let code = run_worktree(
            WorktreeCommands::Remove { slug: "foo".into() },
            Path::new("."),
            &repo,
            &git,
        )
        .collapse();

        assert_eq!(code, ExitCode::Precondition);
        // State must NOT advance to Done when the precondition fails.
        assert_eq!(
            repo.get("foo").expect("state should exist").stage,
            Stage::Casing
        );
    }

    #[test]
    fn worktree_remove_marks_done_when_merged() {
        let repo = InMemoryStateRepository::new().with_state("foo", State::new("foo"));
        let git = FakeGit::new()
            .with_default_branch("main")
            .with_merged_branch("heist/foo");

        let code = run_worktree(
            WorktreeCommands::Remove { slug: "foo".into() },
            Path::new("."),
            &repo,
            &git,
        )
        .collapse();

        assert_eq!(code, ExitCode::Success);
        assert_eq!(
            repo.get("foo").expect("state should exist").stage,
            Stage::Done
        );
    }

    #[test]
    fn worktree_remove_surfaces_worktree_removal_failure() {
        let repo = InMemoryStateRepository::new().with_state("foo", State::new("foo"));
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
        )
        .collapse();

        assert_eq!(code, ExitCode::Git);
        // Failing mid-teardown must not strand the state at Done.
        assert_eq!(
            repo.get("foo").expect("state should exist").stage,
            Stage::Casing
        );
    }

    #[test]
    fn worktree_remove_surfaces_branch_deletion_failure() {
        let repo = InMemoryStateRepository::new().with_state("foo", State::new("foo"));
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
        )
        .collapse();

        assert_eq!(code, ExitCode::Git);
        assert_eq!(
            repo.get("foo").expect("state should exist").stage,
            Stage::Casing
        );
    }

    #[test]
    fn worktree_add_fails_when_origin_unreachable() {
        let temp_dir = TempDir::new().expect("failed to create temp directory");
        let repo = InMemoryStateRepository::new().with_state("foo", State::new("foo"));
        let git = FakeGit::new().failing_add(GitError::WorktreeAdd {
            subtype: "origin-unreachable".into(),
            message: "cannot find remote ref".into(),
        });

        let code = run_worktree(
            WorktreeCommands::Add { slug: "foo".into() },
            temp_dir.path(),
            &repo,
            &git,
        )
        .collapse();

        assert_eq!(code, ExitCode::Git);
        // State untouched: worktree/branch never populated.
        assert_eq!(repo.get("foo").expect("state should exist").worktree, None);
    }
}

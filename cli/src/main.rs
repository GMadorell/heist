use clap::{Parser, Subcommand};
use std::fs;
use std::path::Path;

mod exitcode;
mod state;
mod validation;
mod worktree;

use exitcode::ExitCode;
use state::{state_file_path, today, State};

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

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::State { command } => handle_state(command),
        Commands::Worktree { command } => handle_worktree(command),
        Commands::Validation { command } => handle_validation(command),
        Commands::Resume { slug } => handle_resume(slug),
    }
}

/// Load a slug's state, or print the error and exit per the exit-code contract.
fn load_state_or_exit(slug: &str) -> State {
    match State::load(&state_file_path(slug)) {
        Ok(state) => state,
        Err(e) => {
            eprintln!("failed to load state for slug {}: {}", slug, e);
            e.exit_code().exit();
        }
    }
}

/// Persist a slug's state, or print the error and exit per the exit-code contract.
fn save_state_or_exit(state: &State, slug: &str) {
    if let Err(e) = state.save(&state_file_path(slug)) {
        eprintln!("failed to save state for slug {}: {}", slug, e);
        e.exit_code().exit();
    }
}

fn handle_state(command: StateCommands) {
    match command {
        StateCommands::Init { slug } => {
            let state_file = state_file_path(&slug);
            let state_dir = state_file.parent().expect("state path has a parent");

            if state_dir.exists() {
                eprintln!("state directory already exists for slug: {}", slug);
                ExitCode::Precondition.exit();
            }
            if let Err(e) = fs::create_dir_all(state_dir) {
                eprintln!("failed to create state directory: {}", e);
                ExitCode::Internal.exit();
            }

            save_state_or_exit(&State::new(&slug), &slug);
            ExitCode::Success.exit();
        }
        StateCommands::Get { slug, field } => {
            let state = load_state_or_exit(&slug);
            match state.get_field(&field) {
                Ok(value) => {
                    println!("{}", value);
                    ExitCode::Success.exit();
                }
                Err(e) => {
                    eprintln!("{}", e);
                    ExitCode::Precondition.exit();
                }
            }
        }
        StateCommands::Set { slug, field, value } => {
            let mut state = load_state_or_exit(&slug);
            if let Err(e) = state.set_field(&field, &value) {
                eprintln!("{}", e);
                ExitCode::Precondition.exit();
            }
            state.updated = today();
            save_state_or_exit(&state, &slug);
            ExitCode::Success.exit();
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
                    ExitCode::Internal.exit();
                }
            };

            println!("{}\n\n{}", field_list, json);
            ExitCode::Success.exit();
        }
    }
}

fn handle_worktree(command: WorktreeCommands) {
    let repo_root = Path::new(".");
    match command {
        WorktreeCommands::Add { slug } => {
            let main_branch = worktree::detect_main_branch(repo_root);
            worktree::ensure_worktrees_ignored(repo_root);

            let worktree_path = repo_root.join(".worktrees").join(&slug);

            if !worktree::worktree_exists(repo_root, &slug) {
                // `git worktree add` is a mutating porcelain command; git2's
                // worktree API is more manual and less battle-tested here, so
                // shelling out stays the pragmatic choice.
                let output = std::process::Command::new("git")
                    .args([
                        "worktree",
                        "add",
                        worktree_path.to_string_lossy().as_ref(),
                        "-b",
                        &format!("heist/{}", slug),
                        &format!("origin/{}", main_branch),
                    ])
                    .output()
                    .expect("failed to run git worktree add");

                if !output.status.success() {
                    let git_stderr = String::from_utf8_lossy(&output.stderr);
                    let subtype = if git_stderr.contains("already exists") {
                        "already-exists"
                    } else if git_stderr.contains("cannot find remote ref") {
                        "origin-unreachable"
                    } else if git_stderr.contains("Permission denied") {
                        "permission-denied"
                    } else {
                        "unknown"
                    };
                    eprintln!("{}: {}", subtype, git_stderr.trim());
                    ExitCode::Git.exit();
                }
            }

            create_worktree_symlink(repo_root, &worktree_path, &slug);

            let worktree_absolute = worktree_path
                .canonicalize()
                .expect("failed to canonicalize worktree path");

            let mut state = load_state_or_exit(&slug);
            state.worktree = Some(worktree_absolute.to_string_lossy().to_string());
            state.branch = Some(format!("heist/{}", slug));
            state.updated = today();
            save_state_or_exit(&state, &slug);

            println!("{}", worktree_absolute.display());
            ExitCode::Success.exit();
        }
        WorktreeCommands::Remove { slug } => {
            let main_branch = worktree::detect_main_branch(repo_root);
            let branch_name = format!("heist/{}", slug);

            match worktree::branch_merged_into_main(repo_root, &branch_name, &main_branch) {
                Ok(true) => {}
                Ok(false) => {
                    eprintln!("branch {} is not merged into {}", branch_name, main_branch);
                    ExitCode::Precondition.exit();
                }
                Err(e) => {
                    eprintln!("failed to check merged branches: {}", e);
                    ExitCode::Git.exit();
                }
            }

            // `git worktree remove` / `git branch -d` are mutating porcelain
            // commands; shelling out is more robust than git2's worktree API.
            let worktree_path = repo_root.join(".worktrees").join(&slug);
            let remove_output = std::process::Command::new("git")
                .args([
                    "worktree",
                    "remove",
                    worktree_path.to_string_lossy().as_ref(),
                ])
                .output()
                .expect("failed to run git worktree remove");
            if !remove_output.status.success() {
                let git_stderr = String::from_utf8_lossy(&remove_output.stderr);
                eprintln!("worktree-removal-failed: {}", git_stderr.trim());
                ExitCode::Git.exit();
            }

            let branch_output = std::process::Command::new("git")
                .args(["branch", "-d", &branch_name])
                .output()
                .expect("failed to run git branch -d");
            if !branch_output.status.success() {
                let git_stderr = String::from_utf8_lossy(&branch_output.stderr);
                eprintln!("branch-deletion-failed: {}", git_stderr.trim());
                ExitCode::Git.exit();
            }

            // Remote branch deletion is intentionally out of scope: the branch
            // is often never pushed, or GitHub's auto-delete-on-merge already
            // handled it, and failing there would strand state.json short of "done".

            let mut state = load_state_or_exit(&slug);
            state.stage = state::Stage::Done;
            state.updated = today();
            save_state_or_exit(&state, &slug);

            ExitCode::Success.exit();
        }
    }
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

fn handle_validation(command: ValidationCommands) {
    match command {
        ValidationCommands::Resolve { paths } => {
            if paths.is_empty() {
                eprintln!("at least one path is required");
                ExitCode::Precondition.exit();
            }

            let result = if paths.len() == 1 {
                validation::resolve_validation(&paths[0])
            } else {
                validation::resolve_validations(&paths)
            };

            match result {
                Ok(output) => {
                    print!("{}", output);
                    ExitCode::Success.exit();
                }
                Err(e) => {
                    eprintln!("failed to resolve validation: {}", e);
                    ExitCode::Internal.exit();
                }
            }
        }
        ValidationCommands::Check { path } => match validation::check_validation_exists(&path) {
            Ok(true) => {
                println!("ok");
                ExitCode::Success.exit();
            }
            Ok(false) => {
                println!("missing");
                ExitCode::Precondition.exit();
            }
            Err(e) => {
                eprintln!("failed to check validation: {}", e);
                ExitCode::Internal.exit();
            }
        },
    }
}

fn handle_resume(slug: String) {
    let state = load_state_or_exit(&slug);

    let next_step = match state.stage.next_step() {
        Some((number, name)) => format!("{} ({})", number, name),
        None => "none".to_string(),
    };
    let worktree = state.worktree.as_deref().unwrap_or("none");

    println!("slug: {}", state.slug);
    println!("stage: {}", state.stage.as_str());
    println!("next_step: {}", next_step);
    println!("worktree: {}", worktree);

    ExitCode::Success.exit();
}

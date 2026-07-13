use clap::{Parser, Subcommand};
use std::fs;
use std::path::Path;

mod exitcode;
mod state;
mod worktree;
mod validation;

use state::{State, get_today_date, CURRENT_SCHEMA_VERSION};

// Known fields in state.json
const KNOWN_FIELDS: &[&str] = &[
    "schema_version",
    "slug",
    "stage",
    "worktree",
    "branch",
    "score_step",
    "score_steps_total",
    "fence_rounds",
    "created",
    "updated",
];

fn is_known_field(field: &str) -> bool {
    KNOWN_FIELDS.contains(&field)
}

#[derive(Parser)]
#[command(name = "heist-cli")]
#[command(about = "Heist CLI tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// State management commands
    State {
        #[command(subcommand)]
        command: StateCommands,
    },
    /// Worktree management commands
    Worktree {
        #[command(subcommand)]
        command: WorktreeCommands,
    },
    /// Validation commands
    Validation {
        #[command(subcommand)]
        command: ValidationCommands,
    },
    /// Resume a command
    Resume,
}

#[derive(Subcommand)]
enum StateCommands {
    /// Initialize state
    Init { slug: String },
    /// Get state
    Get { slug: String, field: String },
    /// Set state
    Set { slug: String, field: String, value: String },
    /// Get state schema
    Schema {
        #[arg(long = "write-docs")]
        write_docs: bool,
    },
}

#[derive(Subcommand)]
enum WorktreeCommands {
    /// Add a worktree
    Add { slug: String },
    /// Remove a worktree
    Remove { slug: String },
}

#[derive(Subcommand)]
enum ValidationCommands {
    /// Resolve validation
    Resolve,
    /// Check validation
    Check,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::State { command } => handle_state(command),
        Commands::Worktree { command } => handle_worktree(command),
        Commands::Validation { command } => handle_validation(command),
        Commands::Resume => handle_resume(),
    }
}

fn handle_state(command: StateCommands) {
    match command {
        StateCommands::Init { slug } => {
            // Create .heist/<slug>/ directory if it doesn't exist
            let state_dir = Path::new(".heist").join(&slug);

            // Check if the state directory already exists
            if state_dir.exists() {
                eprintln!("state directory already exists for slug: {}", slug);
                std::process::exit(exitcode::PRECONDITION);
            }

            if let Err(e) = fs::create_dir_all(&state_dir) {
                eprintln!("failed to create state directory: {}", e);
                std::process::exit(exitcode::INTERNAL);
            }

            // Create the state and serialize to JSON
            let state = State::new(&slug);
            let state_json = match serde_json::to_string_pretty(&state) {
                Ok(json) => json,
                Err(e) => {
                    eprintln!("failed to serialize state: {}", e);
                    std::process::exit(exitcode::INTERNAL);
                }
            };

            // Write state.json
            let state_file = state_dir.join("state.json");
            if let Err(e) = fs::write(&state_file, state_json) {
                eprintln!("failed to write state.json: {}", e);
                std::process::exit(exitcode::INTERNAL);
            }

            std::process::exit(exitcode::SUCCESS);
        }
        StateCommands::Get { slug, field } => {
            // Check if field is known
            if !is_known_field(&field) {
                eprintln!("unknown field: {}", field);
                std::process::exit(exitcode::PRECONDITION);
            }

            // Read state.json file
            let state_file = Path::new(".heist").join(&slug).join("state.json");

            // Check if the file exists before parsing
            if !state_file.exists() {
                eprintln!("state file not found for slug: {}", slug);
                std::process::exit(exitcode::PRECONDITION);
            }

            let content = match fs::read_to_string(&state_file) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("failed to read state.json: {}", e);
                    std::process::exit(exitcode::INTERNAL);
                }
            };

            // Parse JSON
            let state_json: serde_json::Value = match serde_json::from_str(&content) {
                Ok(json) => json,
                Err(e) => {
                    eprintln!("failed to parse state.json: {}", e);
                    std::process::exit(exitcode::INTERNAL);
                }
            };

            // Check schema version
            if let Some(version) = state_json.get("schema_version").and_then(|v| v.as_u64()) {
                let version = version as u32;
                if version != CURRENT_SCHEMA_VERSION {
                    eprintln!("schema version mismatch: file has version {}, but CLI supports version {}", version, CURRENT_SCHEMA_VERSION);
                    std::process::exit(exitcode::PRECONDITION);
                }
            } else {
                eprintln!("schema version not found or invalid in state.json");
                std::process::exit(exitcode::INTERNAL);
            }

            // Get field value and print as plain text
            if let Some(value) = state_json.get(&field) {
                match value {
                    serde_json::Value::String(s) => {
                        println!("{}", s);
                    }
                    serde_json::Value::Number(n) => {
                        println!("{}", n);
                    }
                    serde_json::Value::Bool(b) => {
                        println!("{}", b);
                    }
                    serde_json::Value::Null => {
                        println!("null");
                    }
                    _ => {
                        println!("{}", value.to_string());
                    }
                }
                std::process::exit(exitcode::SUCCESS);
            } else {
                eprintln!("field not found: {}", field);
                std::process::exit(exitcode::INTERNAL);
            }
        }
        StateCommands::Set { slug, field, value } => {
            // Check if field is known
            if !is_known_field(&field) {
                eprintln!("unknown field: {}", field);
                std::process::exit(exitcode::PRECONDITION);
            }

            // Read state.json file
            let state_file = Path::new(".heist").join(&slug).join("state.json");

            // Check if the file exists before parsing
            if !state_file.exists() {
                eprintln!("state file not found for slug: {}", slug);
                std::process::exit(exitcode::PRECONDITION);
            }

            let content = match fs::read_to_string(&state_file) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("failed to read state.json: {}", e);
                    std::process::exit(exitcode::INTERNAL);
                }
            };

            // Parse JSON
            let mut state_json: serde_json::Value = match serde_json::from_str(&content) {
                Ok(json) => json,
                Err(e) => {
                    eprintln!("failed to parse state.json: {}", e);
                    std::process::exit(exitcode::INTERNAL);
                }
            };

            // Check schema version
            if let Some(version) = state_json.get("schema_version").and_then(|v| v.as_u64()) {
                let version = version as u32;
                if version != CURRENT_SCHEMA_VERSION {
                    eprintln!("schema version mismatch: file has version {}, but CLI supports version {}", version, CURRENT_SCHEMA_VERSION);
                    std::process::exit(exitcode::PRECONDITION);
                }
            } else {
                eprintln!("schema version not found or invalid in state.json");
                std::process::exit(exitcode::INTERNAL);
            }

            // Update the field with the new value
            state_json[&field] = serde_json::json!(value);

            // Update the updated field to today's date
            let today = get_today_date();
            state_json["updated"] = serde_json::json!(today);

            // Serialize back to JSON with pretty printing
            let updated_json = match serde_json::to_string_pretty(&state_json) {
                Ok(json) => json,
                Err(e) => {
                    eprintln!("failed to serialize state: {}", e);
                    std::process::exit(exitcode::INTERNAL);
                }
            };

            // Write state.json back
            if let Err(e) = fs::write(&state_file, updated_json) {
                eprintln!("failed to write state.json: {}", e);
                std::process::exit(exitcode::INTERNAL);
            }

            std::process::exit(exitcode::SUCCESS);
        }
        StateCommands::Schema { write_docs } => {
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

            // Create example state and pretty print
            let example_state = State::new("example");
            let json = match serde_json::to_string_pretty(&example_state) {
                Ok(json) => json,
                Err(e) => {
                    eprintln!("failed to serialize state: {}", e);
                    std::process::exit(exitcode::INTERNAL);
                }
            };

            let body = format!("{}\n\n{}", field_list, json);
            println!("{}", body);

            if write_docs {
                if let Err(e) = fs::create_dir_all("docs") {
                    eprintln!("failed to create docs directory: {}", e);
                    std::process::exit(exitcode::INTERNAL);
                }
                let docs_content = format!("# State schema\n\n{}\n", body);
                if let Err(e) = fs::write("docs/state-schema.md", docs_content) {
                    eprintln!("failed to write docs/state-schema.md: {}", e);
                    std::process::exit(exitcode::INTERNAL);
                }
            }

            std::process::exit(exitcode::SUCCESS);
        }
    }
}

fn handle_worktree(command: WorktreeCommands) {
    match command {
        WorktreeCommands::Add { slug } => {
            let repo_root = Path::new(".");

            // Detect the main branch
            let main_branch = worktree::detect_main_branch(repo_root);

            // Ensure .worktrees/ is ignored
            worktree::ensure_worktrees_ignored(repo_root);

            // Create worktree directory
            let worktree_path = repo_root.join(".worktrees").join(&slug);

            // Check if worktree already exists
            let worktree_exists = worktree::worktree_exists(repo_root, &slug);

            if !worktree_exists {
                // Run git worktree add (suppress stdout)
                let output = std::process::Command::new("git")
                    .args(&[
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

                    // Classify the git error
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
                    std::process::exit(exitcode::GIT);
                }
            }

            // Create .heist/<slug> symlink in the new worktree
            let main_heist_path = repo_root.join(".heist").join(&slug);
            let main_heist_canonical = main_heist_path.canonicalize()
                .expect("failed to canonicalize main repo .heist/<slug>");

            let worktree_heist_dir = worktree_path.join(".heist");
            if !worktree_heist_dir.exists() {
                fs::create_dir_all(&worktree_heist_dir)
                    .expect("failed to create .heist directory in worktree");
            }

            let worktree_heist_slug = worktree_heist_dir.join(&slug);
            if worktree_heist_slug.exists() {
                fs::remove_file(&worktree_heist_slug)
                    .expect("failed to remove existing symlink");
            }

            #[cfg(unix)]
            {
                use std::os::unix::fs as unix_fs;
                unix_fs::symlink(&main_heist_canonical, &worktree_heist_slug)
                    .expect("failed to create symlink");
            }

            #[cfg(not(unix))]
            {
                eprintln!("symlink creation not supported on this platform");
                std::process::exit(exitcode::INTERNAL);
            }

            // Update state.json with worktree and branch
            let state_file = repo_root.join(".heist").join(&slug).join("state.json");

            let content = fs::read_to_string(&state_file)
                .expect("failed to read state.json");

            let mut state_json: serde_json::Value = serde_json::from_str(&content)
                .expect("failed to parse state.json");

            // Update worktree and branch fields
            state_json["worktree"] = serde_json::json!(worktree_path.canonicalize()
                .expect("failed to canonicalize worktree path")
                .to_string_lossy()
                .to_string());
            state_json["branch"] = serde_json::json!(format!("heist/{}", slug));

            // Update the updated field to today's date
            let today = get_today_date();
            state_json["updated"] = serde_json::json!(today);

            // Serialize back to JSON with pretty printing
            let updated_json = serde_json::to_string_pretty(&state_json)
                .expect("failed to serialize state");

            // Write state.json back
            fs::write(&state_file, updated_json)
                .expect("failed to write state.json");

            // Print worktree path to stdout
            let worktree_absolute = worktree_path.canonicalize()
                .expect("failed to canonicalize worktree path");
            println!("{}", worktree_absolute.display());

            std::process::exit(exitcode::SUCCESS);
        }
        WorktreeCommands::Remove { slug } => {
            let repo_root = Path::new(".");

            // Detect the main branch
            let main_branch = worktree::detect_main_branch(repo_root);

            // Confirm heist/<slug> is merged into main
            let merged_output = std::process::Command::new("git")
                .args(&["branch", "--merged", &format!("origin/{}", main_branch)])
                .current_dir(repo_root)
                .output()
                .expect("failed to check merged branches");

            if !merged_output.status.success() {
                eprintln!("failed to check merged branches");
                std::process::exit(exitcode::GIT);
            }

            let merged_str = String::from_utf8_lossy(&merged_output.stdout);
            let branch_name = format!("heist/{}", slug);

            if !merged_str.contains(&branch_name) {
                eprintln!("branch {} is not merged into {}", branch_name, main_branch);
                std::process::exit(exitcode::PRECONDITION);
            }

            // Run git worktree remove .worktrees/<slug>
            let worktree_path = repo_root.join(".worktrees").join(&slug);
            let worktree_remove_output = std::process::Command::new("git")
                .args(&["worktree", "remove", worktree_path.to_string_lossy().as_ref()])
                .output()
                .expect("failed to run git worktree remove");

            if !worktree_remove_output.status.success() {
                let git_stderr = String::from_utf8_lossy(&worktree_remove_output.stderr);
                eprintln!("worktree-removal-failed: {}", git_stderr.trim());
                std::process::exit(exitcode::GIT);
            }

            // Run git branch -d heist/<slug>
            let branch_delete_output = std::process::Command::new("git")
                .args(&["branch", "-d", &format!("heist/{}", slug)])
                .output()
                .expect("failed to run git branch -d");

            if !branch_delete_output.status.success() {
                let git_stderr = String::from_utf8_lossy(&branch_delete_output.stderr);
                eprintln!("branch-deletion-failed: {}", git_stderr.trim());
                std::process::exit(exitcode::GIT);
            }

            // Also delete the remote branch
            let remote_branch_delete_output = std::process::Command::new("git")
                .args(&["push", "origin", "--delete", &format!("heist/{}", slug)])
                .output()
                .expect("failed to run git push origin --delete");

            if !remote_branch_delete_output.status.success() {
                let git_stderr = String::from_utf8_lossy(&remote_branch_delete_output.stderr);
                eprintln!("remote-branch-deletion-failed: {}", git_stderr.trim());
                std::process::exit(exitcode::GIT);
            }

            // Update state.json's stage to "done" if not already
            let state_file = repo_root.join(".heist").join(&slug).join("state.json");

            let content = fs::read_to_string(&state_file)
                .expect("failed to read state.json");

            let mut state_json: serde_json::Value = serde_json::from_str(&content)
                .expect("failed to parse state.json");

            // Set stage to "done"
            state_json["stage"] = serde_json::json!("done");

            // Update the updated field to today's date
            let today = get_today_date();
            state_json["updated"] = serde_json::json!(today);

            // Serialize back to JSON with pretty printing
            let updated_json = serde_json::to_string_pretty(&state_json)
                .expect("failed to serialize state");

            // Write state.json back
            fs::write(&state_file, updated_json)
                .expect("failed to write state.json");

            std::process::exit(exitcode::SUCCESS);
        }
    }
}

fn handle_validation(command: ValidationCommands) {
    match command {
        ValidationCommands::Resolve => {
            eprintln!("not implemented");
            std::process::exit(1);
        }
        ValidationCommands::Check => {
            eprintln!("not implemented");
            std::process::exit(1);
        }
    }
}

fn handle_resume() {
    eprintln!("not implemented");
    std::process::exit(1);
}

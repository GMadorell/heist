use clap::{Parser, Subcommand};
use std::fs;
use std::path::Path;

mod exitcode;
mod state;

use state::State;

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
    Get,
    /// Set state
    Set,
    /// Get state schema
    Schema,
}

#[derive(Subcommand)]
enum WorktreeCommands {
    /// Add a worktree
    Add,
    /// Remove a worktree
    Remove,
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
        StateCommands::Get => {
            eprintln!("not implemented");
            std::process::exit(1);
        }
        StateCommands::Set => {
            eprintln!("not implemented");
            std::process::exit(1);
        }
        StateCommands::Schema => {
            eprintln!("not implemented");
            std::process::exit(1);
        }
    }
}

fn handle_worktree(command: WorktreeCommands) {
    match command {
        WorktreeCommands::Add => {
            eprintln!("not implemented");
            std::process::exit(1);
        }
        WorktreeCommands::Remove => {
            eprintln!("not implemented");
            std::process::exit(1);
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

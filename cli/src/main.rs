use clap::{Parser, Subcommand};

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
    Init,
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
        StateCommands::Init => {
            eprintln!("not implemented");
            std::process::exit(1);
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

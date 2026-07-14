use clap::Parser;
use heist_cli::cli::{run, Cli};

fn main() {
    let cli = Cli::parse();
    run(cli).exit();
}

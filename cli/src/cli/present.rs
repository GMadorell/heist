use crate::domain::state::State;
use std::fmt::Display;

pub fn error(e: impl Display) {
    eprintln!("{}", e);
}

pub fn line(s: impl Display) {
    println!("{}", s);
}

pub fn state_init_failed(slug: &str, e: impl Display) {
    eprintln!("failed to init state for slug {}: {}", slug, e);
}

pub fn state_load_failed(slug: &str, e: impl Display) {
    eprintln!("failed to load state for slug {}: {}", slug, e);
}

pub fn state_save_failed(slug: &str, e: impl Display) {
    eprintln!("failed to save state for slug {}: {}", slug, e);
}

pub fn no_state_for_add(slug: &str) {
    eprintln!("no state found for slug {}; run `state init` first", slug);
}

pub fn no_state_for_remove(slug: &str) {
    eprintln!("no state found for slug {}", slug);
}

pub fn not_merged(branch: &str, main_branch: &str) {
    eprintln!("branch {} is not merged into {}", branch, main_branch);
}

pub fn validation_output(output: &str) {
    print!("{}", output);
}

pub fn validation_resolve_failed(e: impl Display) {
    eprintln!("failed to resolve validation: {}", e);
}

pub fn validation_ok() {
    println!("ok");
}

pub fn validation_missing() {
    println!("missing");
}

pub fn validation_check_failed(e: impl Display) {
    eprintln!("failed to check validation: {}", e);
}

pub fn resume_summary(state: &State) {
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
}

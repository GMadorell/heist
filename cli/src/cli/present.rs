use crate::app::list::ListRow;
use crate::app::worktree::CleanupOutcome;
use crate::domain::review::Lane;
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

pub fn not_merged(branch: &str, main_branch: &str, verification_error: Option<&str>) {
    eprintln!("branch {} is not merged into {}", branch, main_branch);
    if let Some(e) = verification_error {
        eprintln!(
            "note: could not verify via GitHub, relying on git ancestry only: {}",
            e
        );
    }
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

pub fn list_summary(rows: &[ListRow]) {
    for row in rows {
        let next_step = row
            .next_step
            .map(|stage| stage.as_str().to_string())
            .unwrap_or_else(|| "none".to_string());
        let worktree = row.worktree.as_ref().map(AsRef::as_ref).unwrap_or("none");

        println!(
            "{}  {}  {}  {}  {}",
            row.slug,
            row.stage.as_str(),
            next_step,
            worktree,
            row.mode.as_str()
        );
    }
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
    println!("mode: {}", state.mode.as_str());
    println!("next_step: {}", next_step);
    println!("worktree: {}", worktree);
}

pub fn cleanup_outcome(outcome: &CleanupOutcome) {
    match outcome {
        CleanupOutcome::Removed(slug) => println!("removed {}", slug),
        CleanupOutcome::Skipped {
            slug,
            verification_error: None,
        } => println!("skipped {} (unmerged)", slug),
        CleanupOutcome::Skipped {
            slug,
            verification_error: Some(e),
        } => println!(
            "skipped {} (unmerged; could not verify via GitHub: {})",
            slug, e
        ),
        CleanupOutcome::WouldRemove(slug) => println!("would remove {}", slug),
        CleanupOutcome::Failed { slug, reason } => println!("failed {}: {}", slug, reason),
    }
}

pub fn lane_list(lanes: &[Lane]) {
    for lane in lanes {
        println!("{}", lane.as_str());
    }
}

pub fn no_state_for_review(slug: &str) {
    eprintln!("no state found for slug {}", slug);
}

pub fn no_branch_for_review(slug: &str) {
    eprintln!(
        "no branch recorded for slug {}; run `heist worktree add` first",
        slug
    );
}

pub fn no_remote_default_for_review(slug: &str, e: impl Display) {
    eprintln!(
        "cannot compute diff for slug {}: origin's default branch doesn't resolve ({}); fetch the remote or set `refs/remotes/origin/HEAD`",
        slug, e
    );
}

pub fn base_resolution(merge_ref: &str, pr_base: &str, stale: bool) {
    println!("merge_ref: {}", merge_ref);
    println!("pr_base: {}", pr_base);
    println!("stale: {}", stale);
}

pub fn base_resolution_expired(merge_ref: &str, pr_base: &str, base_ref: &str) {
    println!("merge_ref: {}", merge_ref);
    println!("pr_base: {}", pr_base);
    println!("stale: false");
    eprintln!("note: {} merged", base_ref);
}

pub fn abandoned_base(base_ref: &str) {
    eprintln!("base {} has its PR closed unmerged", base_ref);
}

pub fn base_resolve_failed(base_ref: &str, diagnostic: &str) {
    eprintln!("cannot resolve base {} ({})", base_ref, diagnostic);
}

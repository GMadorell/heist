use crate::app::begin::RollbackFailure;
use crate::app::list::ListRow;
use crate::app::sync::SyncAction;
use crate::app::worktree::CleanupOutcome;
use crate::domain::review::Lane;
use crate::domain::state::State;
use crate::ports::git::GitError;
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

pub fn rollback_diagnostics(errors: &[RollbackFailure]) {
    if errors.is_empty() {
        return;
    }
    eprintln!("begin failed and rollback could not fully clean up:");
    for e in errors {
        match e {
            RollbackFailure::WorktreeRemove(e) => eprintln!("  - failed to remove worktree: {}", e),
            RollbackFailure::BranchDelete(e) => eprintln!("  - failed to delete branch: {}", e),
            RollbackFailure::HeistDirRemove(e) => {
                eprintln!("  - failed to remove state directory: {}", e)
            }
        }
    }
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

pub fn sync_action(action: &SyncAction) {
    match action {
        SyncAction::RebasedOntoMain { onto } => println!("synced: rebased onto {}", onto),
        SyncAction::MergedBase { base_ref } => println!("synced: merged {}", base_ref),
        SyncAction::MergedMainBaseMerged { onto } => {
            println!("synced: merged {} (base already merged)", onto)
        }
    }
}

pub fn base_resolution(kind: &str, merge_ref: &str, pr_base: &str) {
    println!("resolution: {}", kind);
    println!("merge_ref: {}", merge_ref);
    println!("pr_base: {}", pr_base);
}

pub fn base_resolution_expired(merge_ref: &str, pr_base: &str, base_ref: &str) {
    println!("resolution: expired");
    println!("merge_ref: {}", merge_ref);
    println!("pr_base: {}", pr_base);
    eprintln!("note: {} merged", base_ref);
}

pub fn abandoned_base(base_ref: &str) {
    println!("resolution: abandoned");
    eprintln!("base {} has its PR closed unmerged", base_ref);
}

pub fn base_resolve_failed(base_ref: &str, diagnostic: &str) {
    eprintln!("cannot resolve base {} ({})", base_ref, diagnostic);
}

pub fn base_verification_failed(base_ref: &str, message: &str) {
    eprintln!(
        "cannot verify PR state of base {}: {}\nfix the environment (install `gh`, run `gh auth login`, check network) and retry",
        base_ref, message
    );
}

pub fn base_immutable(slug: &str, existing: Option<&str>, requested: &str) {
    let existing = existing.unwrap_or("origin's default branch");
    eprintln!(
        "worktree for {} already exists; its start point ({}) cannot be changed to {}. Drop --base, or remove and re-add the worktree.",
        slug, existing, requested
    );
}

pub fn sync_not_set_up(slug: &str) {
    eprintln!(
        "cannot sync {}: no worktree recorded; run `heist worktree add` first",
        slug
    );
}

pub fn sync_wrong_checkout(slug: &str, expected: &str, actual: &str) {
    eprintln!(
        "refusing to sync {}: run this from the heist's worktree on branch {}, but the checkout is on {}",
        slug, expected, actual
    );
}

pub fn sync_fetch_failed(error: &GitError) {
    eprintln!(
        "refusing to sync: could not fetch origin, so local refs may be stale: {}. Fix the environment and re-run.",
        error
    );
}

pub fn abandoned_base_sync_refused(base_ref: &str) {
    eprintln!(
        "refusing to sync: base {} has its PR closed unmerged; a human must decide whether to drop, salvage, or reopen it",
        base_ref
    );
}

pub fn slug_collision(slug: &str, artifact: &str) {
    eprintln!("cannot begin {}: {} already exists.", slug, artifact);
    eprintln!(
        "note: pick a different slug, or clean up manually: git worktree remove --force .worktrees/{}, git branch -D heist/{}, rm -rf .heist/{}",
        slug, slug, slug
    );
}

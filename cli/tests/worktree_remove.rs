mod worktree_remove {
    use assert_cmd::Command;
    use std::fs;
    use std::path::Path;
    use std::process::Command as StdCommand;
    use tempfile::TempDir;

    fn run_git(dir: &Path, args: &[&str]) {
        let status = StdCommand::new("git")
            .arg("-c")
            .arg("commit.gpgsign=false")
            .args(args)
            .current_dir(dir)
            .status()
            .expect("failed to run git");
        assert!(status.success(), "git {:?} failed", args);
    }

    #[test]
    fn removes_merged_worktree_and_branch() {
        // Create temp directories for main repo and bare remote
        let main_temp = TempDir::new().expect("failed to create main temp dir");
        let main_repo = main_temp.path();

        let bare_temp = TempDir::new().expect("failed to create bare temp dir");
        let bare_repo = bare_temp.path();

        // Initialize bare remote repo
        run_git(bare_repo, &["init", "-q", "--bare"]);

        // Initialize main repo
        run_git(main_repo, &["init", "-q", "-b", "main"]);
        run_git(main_repo, &["config", "user.email", "test@example.com"]);
        run_git(main_repo, &["config", "user.name", "Test"]);

        // Add remote and make initial commit
        let bare_repo_str = bare_repo.to_string_lossy();
        run_git(main_repo, &["remote", "add", "origin", &bare_repo_str]);
        fs::write(main_repo.join("README.md"), "hello").expect("failed to write README");
        run_git(main_repo, &["add", "."]);
        run_git(main_repo, &["commit", "-q", "-m", "init"]);

        // Push to remote
        run_git(main_repo, &["push", "-u", "origin", "main"]);

        // Initialize state for my-slug
        let mut init_cmd = Command::cargo_bin("heist-cli").expect("failed to get cargo bin");
        init_cmd.current_dir(main_repo);
        init_cmd.arg("state").arg("init").arg("my-slug");
        init_cmd.assert().success();

        // Verify state.json was created
        let state_file = main_repo.join(".heist/my-slug/state.json");
        assert!(state_file.exists(), "state.json should exist");

        // Run heist-cli worktree add my-slug
        let mut add_cmd = Command::cargo_bin("heist-cli").expect("failed to get cargo bin");
        let output = add_cmd
            .current_dir(main_repo)
            .arg("worktree")
            .arg("add")
            .arg("my-slug")
            .output()
            .expect("failed to run worktree add");

        assert!(
            output.status.success(),
            "worktree add should succeed, got exit code {:?}, stderr: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );

        // Verify .worktrees/my-slug exists
        let worktree_path = main_repo.join(".worktrees/my-slug");
        assert!(worktree_path.exists(), ".worktrees/my-slug should exist");

        // Make a commit on the worktree on heist/my-slug branch
        fs::write(
            worktree_path.join("feature.txt"),
            "feature work",
        ).expect("failed to write feature.txt");
        run_git(&worktree_path, &["add", "."]);
        run_git(&worktree_path, &["commit", "-q", "-m", "add feature"]);

        // Push the heist/my-slug branch to origin
        run_git(&worktree_path, &["push", "-u", "origin", "heist/my-slug"]);

        // Fast-forward merge heist/my-slug into main in the main repo
        run_git(main_repo, &["checkout", "main"]);
        run_git(main_repo, &["merge", "--ff-only", "heist/my-slug"]);

        // Push main back to origin
        run_git(main_repo, &["push", "origin", "main"]);

        // Run heist-cli worktree remove my-slug
        let mut remove_cmd = Command::cargo_bin("heist-cli").expect("failed to get cargo bin");
        let output = remove_cmd
            .current_dir(main_repo)
            .arg("worktree")
            .arg("remove")
            .arg("my-slug")
            .output()
            .expect("failed to run worktree remove");

        // Check exit code is 0
        assert!(
            output.status.success(),
            "worktree remove should succeed, got exit code {:?}, stderr: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );

        // Verify .worktrees/my-slug is no longer a registered worktree
        let list_output = StdCommand::new("git")
            .args(["worktree", "list"])
            .current_dir(main_repo)
            .output()
            .expect("failed to run git worktree list");
        let list_str = String::from_utf8_lossy(&list_output.stdout);
        assert!(
            !list_str.contains(".worktrees/my-slug"),
            "worktree list should not contain .worktrees/my-slug after removal"
        );

        // Verify heist/my-slug branch no longer exists
        let branch_output = StdCommand::new("git")
            .args(["branch", "-a"])
            .current_dir(main_repo)
            .output()
            .expect("failed to run git branch -a");
        let branch_str = String::from_utf8_lossy(&branch_output.stdout);
        assert!(
            !branch_str.contains("heist/my-slug"),
            "branch list should not contain heist/my-slug after removal"
        );

        // Verify .heist/my-slug/ in main repo still exists untouched
        assert!(
            state_file.exists(),
            ".heist/my-slug/state.json should still exist after worktree removal"
        );

        // Verify state.json's stage is "done"
        let state_content = fs::read_to_string(&state_file)
            .expect("failed to read state.json");
        let state_json: serde_json::Value = serde_json::from_str(&state_content)
            .expect("failed to parse state.json");
        let stage = state_json["stage"].as_str().expect("stage should be string");
        assert_eq!(
            stage, "done",
            "stage should be 'done' after worktree removal, got: {}",
            stage
        );
    }
}

mod worktree_add {
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
    fn creates_worktree_symlink_and_updates_state() {
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

        // Read initial state to verify stage before worktree add
        let initial_content = fs::read_to_string(&state_file).expect("failed to read state.json");
        let initial_state: serde_json::Value =
            serde_json::from_str(&initial_content).expect("failed to parse state.json");
        let initial_stage = initial_state["stage"]
            .as_str()
            .expect("stage should be string");

        // Run heist-cli worktree add my-slug
        let mut cmd = Command::cargo_bin("heist-cli").expect("failed to get cargo bin");
        let output = cmd
            .current_dir(main_repo)
            .arg("worktree")
            .arg("add")
            .arg("my-slug")
            .output()
            .expect("failed to run worktree add");

        // Check exit code is 0
        assert!(
            output.status.success(),
            "command should succeed, got exit code {:?}, stderr: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );

        // Verify stdout is the worktree path followed by newline
        let stdout = String::from_utf8_lossy(&output.stdout);
        let worktree_path = main_repo.join(".worktrees/my-slug");
        let canonicalized_path = worktree_path
            .canonicalize()
            .expect("failed to canonicalize worktree path");
        let expected_output = format!("{}\n", canonicalized_path.display());
        assert_eq!(
            stdout.to_string(),
            expected_output,
            "stdout should be worktree path followed by newline"
        );

        // Verify .worktrees/my-slug exists
        assert!(worktree_path.exists(), ".worktrees/my-slug should exist");

        // Verify it's a registered git worktree on branch heist/my-slug
        let list_output = StdCommand::new("git")
            .args(["worktree", "list"])
            .current_dir(main_repo)
            .output()
            .expect("failed to run git worktree list");
        let list_str = String::from_utf8_lossy(&list_output.stdout);
        assert!(
            list_str.contains(".worktrees/my-slug"),
            "worktree list should contain .worktrees/my-slug"
        );
        assert!(
            list_str.contains("heist/my-slug"),
            "worktree list should contain heist/my-slug branch"
        );

        // Verify .worktrees/my-slug/.heist/my-slug is a symlink to main repo's .heist/my-slug
        let symlink_path = worktree_path.join(".heist/my-slug");
        assert!(
            symlink_path.exists(),
            ".worktrees/my-slug/.heist/my-slug should exist"
        );

        let symlink_target = fs::read_link(&symlink_path).expect("failed to read symlink");
        let expected_target = main_repo
            .join(".heist/my-slug")
            .canonicalize()
            .expect("failed to canonicalize expected target");
        let actual_target = symlink_target
            .canonicalize()
            .expect("failed to canonicalize actual target");

        assert_eq!(
            actual_target, expected_target,
            "symlink should point to main repo's .heist/my-slug"
        );

        // Verify state.json was updated with worktree and branch
        let updated_content =
            fs::read_to_string(&state_file).expect("failed to read updated state.json");
        let updated_state: serde_json::Value =
            serde_json::from_str(&updated_content).expect("failed to parse updated state.json");

        // Check worktree field is set
        let worktree_value = updated_state["worktree"].as_str();
        assert!(
            worktree_value.is_some(),
            "worktree field should not be null"
        );

        // Check branch field is set
        let branch_value = updated_state["branch"].as_str();
        assert!(branch_value.is_some(), "branch field should not be null");

        // Check that updated field is today's date
        let get_date_output = StdCommand::new("date")
            .arg("+%Y-%m-%d")
            .output()
            .expect("failed to get date");
        let today = String::from_utf8(get_date_output.stdout)
            .expect("invalid utf8")
            .trim()
            .to_string();

        let updated_date = updated_state["updated"]
            .as_str()
            .expect("updated should be string");
        assert_eq!(updated_date, today, "updated field should be today's date");

        // Check that stage is unchanged
        let updated_stage = updated_state["stage"]
            .as_str()
            .expect("stage should be string");
        assert_eq!(updated_stage, initial_stage, "stage should not change");
    }

    #[test]
    fn is_idempotent_on_reentry() {
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

        // First call to worktree add
        let mut cmd1 = Command::cargo_bin("heist-cli").expect("failed to get cargo bin");
        let output1 = cmd1
            .current_dir(main_repo)
            .arg("worktree")
            .arg("add")
            .arg("my-slug")
            .output()
            .expect("failed to run worktree add");

        assert!(
            output1.status.success(),
            "first worktree add should succeed, got exit code {:?}, stderr: {}",
            output1.status.code(),
            String::from_utf8_lossy(&output1.stderr)
        );

        let stdout1 = String::from_utf8_lossy(&output1.stdout);
        let worktree_path = main_repo.join(".worktrees/my-slug");
        let canonicalized_path = worktree_path
            .canonicalize()
            .expect("failed to canonicalize worktree path");
        let expected_output = format!("{}\n", canonicalized_path.display());

        assert_eq!(
            stdout1.to_string(),
            expected_output,
            "first call stdout should be worktree path followed by newline"
        );

        // Verify symlink exists and is correct
        let symlink_path = worktree_path.join(".heist/my-slug");
        assert!(
            symlink_path.exists(),
            ".worktrees/my-slug/.heist/my-slug should exist after first call"
        );

        let symlink_target = fs::read_link(&symlink_path).expect("failed to read symlink");
        let expected_target = main_repo
            .join(".heist/my-slug")
            .canonicalize()
            .expect("failed to canonicalize expected target");
        let actual_target = symlink_target
            .canonicalize()
            .expect("failed to canonicalize actual target");
        assert_eq!(
            actual_target, expected_target,
            "symlink should point to main repo's .heist/my-slug after first call"
        );

        // Second call to worktree add (should be idempotent)
        let mut cmd2 = Command::cargo_bin("heist-cli").expect("failed to get cargo bin");
        let output2 = cmd2
            .current_dir(main_repo)
            .arg("worktree")
            .arg("add")
            .arg("my-slug")
            .output()
            .expect("failed to run worktree add again");

        assert!(
            output2.status.success(),
            "second worktree add should succeed (idempotent), got exit code {:?}, stderr: {}",
            output2.status.code(),
            String::from_utf8_lossy(&output2.stderr)
        );

        let stdout2 = String::from_utf8_lossy(&output2.stdout);
        assert_eq!(
            stdout2.to_string(),
            expected_output,
            "second call stdout should be same worktree path followed by newline"
        );

        // Verify symlink still exists and is still correct
        let symlink_target2 =
            fs::read_link(&symlink_path).expect("failed to read symlink after second call");
        let actual_target2 = symlink_target2
            .canonicalize()
            .expect("failed to canonicalize actual target after second call");
        assert_eq!(
            actual_target2, expected_target,
            "symlink should still point to main repo's .heist/my-slug after second call"
        );

        // Test symlink recreation variant: delete symlink and re-run
        fs::remove_file(&symlink_path).expect("failed to delete symlink");
        assert!(!symlink_path.exists(), "symlink should be deleted");

        let mut cmd3 = Command::cargo_bin("heist-cli").expect("failed to get cargo bin");
        let output3 = cmd3
            .current_dir(main_repo)
            .arg("worktree")
            .arg("add")
            .arg("my-slug")
            .output()
            .expect("failed to run worktree add after symlink deletion");

        assert!(
            output3.status.success(),
            "worktree add after symlink deletion should succeed, got exit code {:?}, stderr: {}",
            output3.status.code(),
            String::from_utf8_lossy(&output3.stderr)
        );

        // Verify symlink was recreated
        assert!(
            symlink_path.exists(),
            ".worktrees/my-slug/.heist/my-slug should be recreated"
        );

        let symlink_target3 =
            fs::read_link(&symlink_path).expect("failed to read symlink after recreation");
        let actual_target3 = symlink_target3
            .canonicalize()
            .expect("failed to canonicalize actual target after recreation");
        assert_eq!(
            actual_target3, expected_target,
            "symlink should point to main repo's .heist/my-slug after recreation"
        );
    }

    #[test]
    fn branch_conflict_exits_git_error_code() {
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

        // Pre-create a branch named heist/my-slug (not as a worktree, just a branch)
        run_git(main_repo, &["branch", "heist/my-slug"]);

        // Initialize state for my-slug
        let mut init_cmd = Command::cargo_bin("heist-cli").expect("failed to get cargo bin");
        init_cmd.current_dir(main_repo);
        init_cmd.arg("state").arg("init").arg("my-slug");
        init_cmd.assert().success();

        // Run heist-cli worktree add my-slug (should fail because branch already exists)
        let mut cmd = Command::cargo_bin("heist-cli").expect("failed to get cargo bin");
        let output = cmd
            .current_dir(main_repo)
            .arg("worktree")
            .arg("add")
            .arg("my-slug")
            .output()
            .expect("failed to run worktree add");

        // Check exit code is 3 (GIT error)
        assert_eq!(
            output.status.code(),
            Some(3),
            "should exit with code 3 (GIT), got {:?}",
            output.status.code()
        );

        // Check stderr contains "already-exists"
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("already-exists"),
            "stderr should contain 'already-exists', got: {}",
            stderr
        );
    }
}

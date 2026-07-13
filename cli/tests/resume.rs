mod resume {
    use assert_cmd::Command;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn prints_slug_stage_next_step_and_worktree() {
        let temp_dir = TempDir::new().expect("failed to create temp directory");
        let temp_path = temp_dir.path();

        // Create .heist/my-slug directory
        fs::create_dir_all(temp_path.join(".heist/my-slug"))
            .expect("failed to create .heist/my-slug directory");

        // Create state.json with stage: forging and worktree: /abs/path/to/worktree
        let state_json = r#"{
  "schema_version": 1,
  "slug": "my-slug",
  "stage": "forging",
  "worktree": "/abs/path/to/worktree",
  "branch": null,
  "score_step": 0,
  "score_steps_total": 0,
  "fence_rounds": 0,
  "created": "2024-01-01",
  "updated": "2024-01-01"
}"#;

        fs::write(
            temp_path.join(".heist/my-slug/state.json"),
            state_json,
        )
        .expect("failed to write state.json");

        // Run heist-cli resume my-slug
        let mut cmd = Command::cargo_bin("heist-cli").expect("failed to get cargo bin");
        let output = cmd
            .current_dir(temp_path)
            .arg("resume")
            .arg("my-slug")
            .output()
            .expect("failed to run resume command");

        // Verify exit code is 0
        assert!(
            output.status.success(),
            "command should succeed, got exit code {:?}, stderr: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );

        // Verify stdout is exactly as specified
        let stdout = String::from_utf8_lossy(&output.stdout);
        let expected = "slug: my-slug\nstage: forging\nnext_step: 5\nworktree: /abs/path/to/worktree\n";
        assert_eq!(
            stdout.to_string(),
            expected,
            "stdout should be exactly: {}\nbut got: {}",
            expected,
            stdout
        );
    }
}

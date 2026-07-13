mod state_get {
    use assert_cmd::Command;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn prints_requested_field() {
        let temp_dir = TempDir::new().expect("failed to create temp directory");
        let temp_path = temp_dir.path();

        // Create .heist/my-slug/ directory
        fs::create_dir_all(temp_path.join(".heist/my-slug"))
            .expect("failed to create state directory");

        // Create state.json fixture with schema_version 1, stage "forging"
        let state_json = r#"{
  "schema_version": 1,
  "slug": "my-slug",
  "stage": "forging",
  "worktree": null,
  "branch": null,
  "score_step": 0,
  "score_steps_total": 0,
  "fence_rounds": 0,
  "created": "2026-07-13",
  "updated": "2026-07-13"
}"#;
        fs::write(temp_path.join(".heist/my-slug/state.json"), state_json)
            .expect("failed to write state.json");

        // Run heist-cli state get my-slug stage
        let mut cmd = Command::cargo_bin("heist-cli").expect("failed to get cargo bin");
        let output = cmd
            .current_dir(temp_path)
            .arg("state")
            .arg("get")
            .arg("my-slug")
            .arg("stage")
            .output()
            .expect("failed to run command");

        assert!(output.status.success(), "expected success, got {:?}", output.status);

        // Verify stdout is exactly "forging\n"
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout, "forging\n", "stdout should be exactly 'forging\\n', got: {:?}", stdout);
    }
}

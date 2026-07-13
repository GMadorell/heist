mod state_set {
    use assert_cmd::Command;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn updates_field_and_bumps_updated() {
        let temp_dir = TempDir::new().expect("failed to create temp directory");
        let temp_path = temp_dir.path();

        // Create .heist/my-slug/ directory
        fs::create_dir_all(temp_path.join(".heist/my-slug"))
            .expect("failed to create state directory");

        // Create state.json fixture with stage "planning" and an old updated date
        let state_json = r#"{
  "schema_version": 1,
  "slug": "my-slug",
  "stage": "planning",
  "worktree": null,
  "branch": null,
  "score_step": 0,
  "score_steps_total": 0,
  "fence_rounds": 0,
  "created": "2026-07-10",
  "updated": "2026-07-10"
}"#;
        fs::write(temp_path.join(".heist/my-slug/state.json"), state_json)
            .expect("failed to write state.json");

        // Get today's date for later verification
        let today = get_today_date();

        // Run heist-cli state set my-slug stage fence_review
        let mut cmd = Command::cargo_bin("heist-cli").expect("failed to get cargo bin");
        let output = cmd
            .current_dir(temp_path)
            .arg("state")
            .arg("set")
            .arg("my-slug")
            .arg("stage")
            .arg("fence_review")
            .output()
            .expect("failed to run command");

        assert!(
            output.status.success(),
            "expected success, got {:?}, stderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );

        // Re-read state.json
        let content = fs::read_to_string(temp_path.join(".heist/my-slug/state.json"))
            .expect("failed to read state.json");
        let state: serde_json::Value =
            serde_json::from_str(&content).expect("failed to parse state.json");

        // Assert stage is now "fence_review"
        assert_eq!(
            state["stage"], "fence_review",
            "stage should be 'fence_review', got: {}",
            state["stage"]
        );

        // Assert updated equals today's date
        assert_eq!(
            state["updated"], today,
            "updated should be today's date ({}), got: {}",
            today, state["updated"]
        );

        // Assert other fields are unchanged
        assert_eq!(
            state["schema_version"], 1,
            "schema_version should be unchanged"
        );
        assert_eq!(state["slug"], "my-slug", "slug should be unchanged");
        assert_eq!(
            state["worktree"],
            serde_json::Value::Null,
            "worktree should be unchanged"
        );
        assert_eq!(
            state["branch"],
            serde_json::Value::Null,
            "branch should be unchanged"
        );
        assert_eq!(state["score_step"], 0, "score_step should be unchanged");
        assert_eq!(
            state["score_steps_total"], 0,
            "score_steps_total should be unchanged"
        );
        assert_eq!(state["fence_rounds"], 0, "fence_rounds should be unchanged");
        assert_eq!(
            state["created"], "2026-07-10",
            "created should be unchanged"
        );
    }

    #[test]
    fn rejects_unknown_field() {
        let temp_dir = TempDir::new().expect("failed to create temp directory");
        let temp_path = temp_dir.path();

        // Create .heist/my-slug/ directory
        fs::create_dir_all(temp_path.join(".heist/my-slug"))
            .expect("failed to create state directory");

        // Create state.json fixture with all valid fields
        let state_json = r#"{
  "schema_version": 1,
  "slug": "my-slug",
  "stage": "planning",
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

        // Run heist-cli state set my-slug bogus_field x
        let mut cmd = Command::cargo_bin("heist-cli").expect("failed to get cargo bin");
        let output = cmd
            .current_dir(temp_path)
            .arg("state")
            .arg("set")
            .arg("my-slug")
            .arg("bogus_field")
            .arg("x")
            .output()
            .expect("failed to run command");

        // Assert exit code is 2 (PRECONDITION)
        assert_eq!(
            output.status.code(),
            Some(2),
            "expected exit code 2, got {:?}",
            output.status.code()
        );

        // Assert stderr contains "bogus_field"
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("bogus_field"),
            "stderr should contain 'bogus_field', got: {:?}",
            stderr
        );
    }

    #[test]
    fn rejects_schema_version_mismatch() {
        let temp_dir = TempDir::new().expect("failed to create temp directory");
        let temp_path = temp_dir.path();

        // Create .heist/my-slug/ directory
        fs::create_dir_all(temp_path.join(".heist/my-slug"))
            .expect("failed to create state directory");

        // Create state.json fixture with schema_version 99 (mismatched)
        let state_json = r#"{
  "schema_version": 99,
  "slug": "my-slug",
  "stage": "planning",
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

        // Run heist-cli state set my-slug stage done
        let mut cmd = Command::cargo_bin("heist-cli").expect("failed to get cargo bin");
        let output = cmd
            .current_dir(temp_path)
            .arg("state")
            .arg("set")
            .arg("my-slug")
            .arg("stage")
            .arg("done")
            .output()
            .expect("failed to run command");

        // Assert exit code is 2 (PRECONDITION)
        assert_eq!(
            output.status.code(),
            Some(2),
            "expected exit code 2, got {:?}",
            output.status.code()
        );

        // Assert stderr mentions both 99 and 1
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("99"),
            "stderr should contain '99', got: {:?}",
            stderr
        );
        assert!(
            stderr.contains("1"),
            "stderr should contain '1', got: {:?}",
            stderr
        );
    }

    fn get_today_date() -> String {
        let output = std::process::Command::new("date")
            .arg("+%Y-%m-%d")
            .output()
            .expect("failed to get date");
        String::from_utf8(output.stdout)
            .expect("invalid utf8 from date command")
            .trim()
            .to_string()
    }
}

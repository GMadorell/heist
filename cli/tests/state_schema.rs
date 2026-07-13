mod state_schema {
    use assert_cmd::Command;

    #[test]
    fn prints_field_list_and_example() {
        // Get today's date using the same method as State::new
        let today = get_today_date();

        // Run heist-cli state schema
        let mut cmd = Command::cargo_bin("heist-cli").expect("failed to get cargo bin");
        let output = cmd
            .arg("state")
            .arg("schema")
            .output()
            .expect("failed to run command");

        assert!(
            output.status.success(),
            "command should succeed, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Build the expected output with dynamic date
        let expected = format!(
            r#"schema_version: u32
slug: string
stage: string (casing|planning|fence_review|human_review|forging|safehouse|implementing|cleaning|done)
worktree: string|null
branch: string|null
score_step: u32
score_steps_total: u32
fence_rounds: u32
created: string
updated: string

{{
  "schema_version": 1,
  "slug": "example",
  "stage": "casing",
  "worktree": null,
  "branch": null,
  "score_step": 0,
  "score_steps_total": 0,
  "fence_rounds": 0,
  "created": "{}",
  "updated": "{}"
}}"#,
            today, today
        );

        assert_eq!(
            stdout.trim(),
            expected.trim(),
            "stdout should match expected golden text"
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

    #[test]
    fn write_docs_refreshes_docs_file() {
        let today = get_today_date();
        let temp_dir = tempfile::TempDir::new().expect("failed to create temp directory");
        let temp_path = temp_dir.path();

        let mut cmd = Command::cargo_bin("heist-cli").expect("failed to get cargo bin");
        let output = cmd
            .current_dir(temp_path)
            .arg("state")
            .arg("schema")
            .arg("--write-docs")
            .output()
            .expect("failed to run command");

        assert!(
            output.status.success(),
            "command should succeed, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8_lossy(&output.stdout);

        let body = format!(
            r#"schema_version: u32
slug: string
stage: string (casing|planning|fence_review|human_review|forging|safehouse|implementing|cleaning|done)
worktree: string|null
branch: string|null
score_step: u32
score_steps_total: u32
fence_rounds: u32
created: string
updated: string

{{
  "schema_version": 1,
  "slug": "example",
  "stage": "casing",
  "worktree": null,
  "branch": null,
  "score_step": 0,
  "score_steps_total": 0,
  "fence_rounds": 0,
  "created": "{}",
  "updated": "{}"
}}"#,
            today, today
        );

        // stdout unchanged from Step 16's golden text
        assert_eq!(
            stdout.trim(),
            body.trim(),
            "stdout should match Step 16 golden text"
        );

        // docs/state-schema.md written with heading + same content
        let docs_file = temp_path.join("docs/state-schema.md");
        assert!(docs_file.exists(), "docs/state-schema.md should exist");
        let docs_content = std::fs::read_to_string(&docs_file).expect("failed to read docs file");

        let expected_docs = format!("# State schema\n\n{}", body);
        assert_eq!(
            docs_content.trim(),
            expected_docs.trim(),
            "docs/state-schema.md should contain heading + golden text"
        );
    }
}

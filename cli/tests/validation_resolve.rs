mod validation_resolve {
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
    fn single_path_merges_root_and_leaf() {
        // Create temp directory for repo
        let temp_dir = TempDir::new().expect("failed to create temp directory");
        let repo_root = temp_dir.path();

        // Initialize git repo
        run_git(repo_root, &["init", "-q", "-b", "main"]);
        run_git(repo_root, &["config", "user.email", "test@example.com"]);
        run_git(repo_root, &["config", "user.name", "Test"]);

        // Create root validation.md with all 6 sections
        let root_validation = r#"# Validation

## Build
root build command

## Lint
root lint config

## Test
root test runner

## Docs
root docs instruction

## PR conventions
root pr convention

## Notes
root notes"#;

        fs::write(repo_root.join("validation.md"), root_validation)
            .expect("failed to write root validation.md");

        // Create cli directory and cli/validation.md with only Build/Lint/Test (different content)
        fs::create_dir_all(repo_root.join("cli"))
            .expect("failed to create cli directory");

        let cli_validation = r#"## Build
cli build command

## Lint
cli lint config

## Test
cli test runner"#;

        fs::write(repo_root.join("cli/validation.md"), cli_validation)
            .expect("failed to write cli/validation.md");

        // Create cli/src directory (needed for the path to make sense)
        fs::create_dir_all(repo_root.join("cli/src"))
            .expect("failed to create cli/src directory");

        // Create a dummy file at cli/src/main.rs (not really needed for the test, but for realism)
        fs::write(repo_root.join("cli/src/main.rs"), "// stub")
            .expect("failed to write cli/src/main.rs");

        // Commit everything
        run_git(repo_root, &["add", "."]);
        run_git(repo_root, &["commit", "-q", "-m", "init"]);

        // Run heist-cli validation resolve cli/src/main.rs
        let mut cmd = Command::cargo_bin("heist-cli").expect("failed to get cargo bin");
        let output = cmd
            .current_dir(repo_root)
            .arg("validation")
            .arg("resolve")
            .arg("cli/src/main.rs")
            .output()
            .expect("failed to run validation resolve");

        // Check exit code is 0
        assert!(
            output.status.success(),
            "command should succeed, got exit code {:?}, stderr: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );

        // Get stdout and verify it matches the expected golden string
        let stdout = String::from_utf8_lossy(&output.stdout);

        let expected_golden = r#"## Build
cli build command

## Lint
cli lint config

## Test
cli test runner

## Docs
root docs instruction

## PR conventions
root pr convention

## Notes
root notes
"#;

        assert_eq!(
            stdout.to_string(),
            expected_golden,
            "stdout should match the expected golden string"
        );
    }
}

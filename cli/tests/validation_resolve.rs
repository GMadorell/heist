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

    /// Set up a fixture repo with root validation.md, cli/validation.md, and plugin/validation.md.
    /// Returns the repo root path.
    fn setup_fixture() -> TempDir {
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
        fs::create_dir_all(repo_root.join("cli")).expect("failed to create cli directory");

        let cli_validation = r#"## Build
cli build command

## Lint
cli lint config

## Test
cli test runner"#;

        fs::write(repo_root.join("cli/validation.md"), cli_validation)
            .expect("failed to write cli/validation.md");

        // Create plugin directory and plugin/validation.md (different from cli)
        fs::create_dir_all(repo_root.join("plugin")).expect("failed to create plugin directory");

        let plugin_validation = r#"## Build
plugin build command

## Lint
plugin lint config

## Test
plugin test runner"#;

        fs::write(repo_root.join("plugin/validation.md"), plugin_validation)
            .expect("failed to write plugin/validation.md");

        // Create cli/src directory
        fs::create_dir_all(repo_root.join("cli/src")).expect("failed to create cli/src directory");

        // Create a dummy file at cli/src/main.rs
        fs::write(repo_root.join("cli/src/main.rs"), "// stub")
            .expect("failed to write cli/src/main.rs");

        // Create plugin/skills/heist directory
        fs::create_dir_all(repo_root.join("plugin/skills/heist"))
            .expect("failed to create plugin/skills/heist directory");

        // Create a dummy file at plugin/skills/heist/pipeline.md
        fs::write(
            repo_root.join("plugin/skills/heist/pipeline.md"),
            "# pipeline",
        )
        .expect("failed to write plugin/skills/heist/pipeline.md");

        // Create cli/tests directory and validation_resolve.rs
        fs::create_dir_all(repo_root.join("cli/tests"))
            .expect("failed to create cli/tests directory");

        fs::write(
            repo_root.join("cli/tests/validation_resolve.rs"),
            "// test file",
        )
        .expect("failed to write cli/tests/validation_resolve.rs");

        // Commit everything
        run_git(repo_root, &["add", "."]);
        run_git(repo_root, &["commit", "-q", "-m", "init"]);

        temp_dir
    }

    #[test]
    fn single_path_merges_root_and_leaf() {
        let temp_dir = setup_fixture();
        let repo_root = temp_dir.path();

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

    #[test]
    fn multi_path_returns_distinct_scopes() {
        let temp_dir = setup_fixture();
        let repo_root = temp_dir.path();

        // Run heist-cli validation resolve with two paths in different scopes
        let mut cmd = Command::cargo_bin("heist-cli").expect("failed to get cargo bin");
        let output = cmd
            .current_dir(repo_root)
            .arg("validation")
            .arg("resolve")
            .arg("plugin/skills/heist/pipeline.md")
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

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Expect two distinct labeled blocks: one for plugin scope, one for cli scope
        // plugin scope should have plugin's Build/Lint/Test overrides
        assert!(stdout.contains("## Build"), "should contain Build section");
        assert!(
            stdout.contains("plugin build command"),
            "should contain plugin build command"
        );
        assert!(
            stdout.contains("cli build command"),
            "should contain cli build command"
        );

        // Check for scope labels (the deepest validation.md directory found)
        // For plugin/skills/heist/pipeline.md, scope is plugin
        // For cli/src/main.rs, scope is cli
        // We should see multiple blocks labeled with their scopes
        let lines: Vec<&str> = stdout.lines().collect();

        // Count how many times we see "## Build" - there should be 2 (one per scope)
        let build_count = lines
            .iter()
            .filter(|line| line.contains("## Build"))
            .count();
        assert_eq!(
            build_count, 2,
            "should have exactly 2 Build sections (one per scope), got: {}",
            stdout
        );
    }

    #[test]
    fn multi_path_dedupes_same_scope() {
        let temp_dir = setup_fixture();
        let repo_root = temp_dir.path();

        // Run heist-cli validation resolve with two paths in the SAME scope (cli)
        let mut cmd = Command::cargo_bin("heist-cli").expect("failed to get cargo bin");
        let output = cmd
            .current_dir(repo_root)
            .arg("validation")
            .arg("resolve")
            .arg("cli/src/main.rs")
            .arg("cli/tests/validation_resolve.rs")
            .output()
            .expect("failed to run validation resolve");

        // Check exit code is 0
        assert!(
            output.status.success(),
            "command should succeed, got exit code {:?}, stderr: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Expect only ONE block (deduped, since both paths resolve to same cli scope)
        let build_count = stdout
            .lines()
            .filter(|line| line.contains("## Build"))
            .count();
        assert_eq!(
            build_count, 1,
            "should have exactly 1 Build section (deduped same scope), got: {}",
            stdout
        );
    }
}

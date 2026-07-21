use assert_cmd::Command;

#[test]
fn prints_one_line_per_tool_and_exits_ok_if_all_present() {
    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .arg("doctor")
        .output()
        .expect("failed to run doctor command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.trim_end().lines().collect();
    assert_eq!(lines.len(), 3, "expected 3 lines, got: {}", stdout);

    let expected_tools = ["git", "gh", "crit"];
    for (line, tool) in lines.iter().zip(expected_tools.iter()) {
        assert!(
            *line == format!("{}: ok", tool) || *line == format!("{}: missing", tool),
            "unexpected line: {}",
            line
        );
    }

    // This test environment has git/gh/crit on PATH, so doctor should report
    // everything ok and exit 0.
    assert!(lines.iter().all(|line| line.ends_with("ok")));
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn exits_precondition_if_any_tool_missing() {
    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .arg("doctor")
        .env("PATH", "/nonexistent-heist-doctor-test-path")
        .output()
        .expect("failed to run doctor command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.trim_end().lines().collect();
    assert_eq!(lines.len(), 3, "expected 3 lines, got: {}", stdout);

    let expected_tools = ["git", "gh", "crit"];
    for (line, tool) in lines.iter().zip(expected_tools.iter()) {
        assert_eq!(*line, format!("{}: missing", tool));
    }

    assert_eq!(output.status.code(), Some(2));
}

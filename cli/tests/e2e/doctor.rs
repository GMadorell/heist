use assert_cmd::Command;

#[test]
fn prints_one_line_per_tool_and_exits_precondition_if_any_missing() {
    let mut cmd = Command::cargo_bin("heist").expect("failed to get cargo bin");
    let output = cmd
        .arg("doctor")
        .output()
        .expect("failed to run doctor command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.trim_end().lines().collect();
    assert_eq!(lines.len(), 3, "expected 3 lines, got: {}", stdout);

    let expected_tools = ["git", "gh", "crit"];
    let mut any_missing = false;
    for (line, tool) in lines.iter().zip(expected_tools.iter()) {
        assert!(
            *line == format!("{}: ok", tool) || *line == format!("{}: missing", tool),
            "unexpected line: {}",
            line
        );
        if line.ends_with("missing") {
            any_missing = true;
        }
    }

    let expected_code = if any_missing { 2 } else { 0 };
    assert_eq!(output.status.code(), Some(expected_code));
}

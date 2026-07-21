use super::{Finding, Score, Shape, Step};

pub fn parse(text: &str) -> Result<Score, Vec<Finding>> {
    let lines: Vec<&str> = text.lines().collect();
    let mut steps = Vec::new();
    let mut findings = Vec::new();
    let mut seen_numbers = std::collections::HashSet::new();
    let mut current_enclosing_wave = 0u32;
    let mut in_fence: Option<&'static str> = None;
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        let next_fence = toggle_fence(trimmed, in_fence);
        if next_fence != in_fence || trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = next_fence;
            i += 1;
            continue;
        }
        if in_fence.is_some() {
            i += 1;
            continue;
        }

        if let Some(wave_num) = wave_header_number(line) {
            current_enclosing_wave = wave_num;
            i += 1;
            continue;
        }

        // Malformed header attempts are flagged rather than silently
        // absorbed as body text or dropped from the model.
        if let Some(bad) = malformed_wave_header(line) {
            findings.push(Finding {
                step: 0,
                message: format!(
                    "line {}: malformed '## Wave' header: '{}' is not a valid wave number",
                    i + 1,
                    bad
                ),
            });
            i += 1;
            continue;
        }
        if let Some((bad, _title)) = malformed_step_header(line) {
            findings.push(Finding {
                step: 0,
                message: format!(
                    "line {}: malformed step header: '{}' is not a valid step number",
                    i + 1,
                    bad
                ),
            });
            i += 1;
            continue;
        }

        if let Some((step_num, title)) = step_header_parts(line) {
            let header_line = i;
            let step_end = find_step_boundary(&lines, header_line + 1);
            let step = parse_step(
                &lines,
                header_line,
                step_end,
                step_num,
                title,
                current_enclosing_wave,
                &mut findings,
            );
            if !seen_numbers.insert(step_num) {
                findings.push(Finding {
                    step: step_num,
                    message: "duplicate step number".to_string(),
                });
            }
            steps.push(step);
            i = step_end;
            continue;
        }

        i += 1;
    }

    if findings.is_empty() {
        Ok(Score { steps })
    } else {
        Err(findings)
    }
}

// --- line classification -----------------------------------------------

/// Toggles fence state given a trimmed line. Returns the new state. Shared
/// by every scan pass so fence-detection logic lives in one place.
fn toggle_fence(trimmed: &str, current: Option<&'static str>) -> Option<&'static str> {
    let marker = if trimmed.starts_with("```") {
        Some("```")
    } else if trimmed.starts_with("~~~") {
        Some("~~~")
    } else {
        None
    };
    match marker {
        Some(m) if current == Some(m) => None,
        Some(m) if current.is_none() => Some(m),
        _ => current,
    }
}

/// Returns the wave number if `line` is a well-formed `## Wave N` header
/// token (literal prefix, then only ASCII digits, nothing else).
fn wave_header_number(line: &str) -> Option<u32> {
    let rest = line.strip_prefix("## Wave ")?;
    let rest = rest.trim();
    if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    rest.parse::<u32>().ok()
}

/// Returns `(step_number, title)` if `line` is a well-formed Step header
/// token (canonical `### Step N: <title>` or the tolerated flat `## Step
/// N: <title>`).
fn step_header_parts(line: &str) -> Option<(u32, &str)> {
    let rest = line
        .strip_prefix("### Step ")
        .or_else(|| line.strip_prefix("## Step "))?;
    let colon_pos = rest.find(": ")?;
    let num_str = &rest[..colon_pos];
    if num_str.is_empty() || !num_str.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let step_num = num_str.parse::<u32>().ok()?;
    Some((step_num, &rest[colon_pos + 2..]))
}

/// Returns the offending token if `line` looks like a deliberate attempt at
/// a `## Wave <N>` header (reserved prefix, exactly one whitespace-separated
/// token) whose number isn't valid. The single-token heuristic distinguishes
/// an actual typo'd header (`## Wave two`) from ordinary column-0 prose that
/// happens to share the prefix (a multi-word sentence quoted inside a
/// Change field) — the latter stays body text and raises no finding.
fn malformed_wave_header(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("## Wave ")?;
    let trimmed = rest.trim();
    if trimmed.is_empty() || trimmed.split_whitespace().count() != 1 {
        return None;
    }
    if trimmed.chars().all(|c| c.is_ascii_digit()) {
        return None; // valid, handled by wave_header_number
    }
    Some(trimmed)
}

/// Returns `(bad_id, title)` if `line` looks like a deliberate attempt at a
/// Step header (canonical or flat prefix, then a single non-whitespace
/// token before `": "`) whose step number isn't valid. Same single-token
/// heuristic as `malformed_wave_header`.
fn malformed_step_header(line: &str) -> Option<(&str, &str)> {
    let rest = line
        .strip_prefix("### Step ")
        .or_else(|| line.strip_prefix("## Step "))?;
    let colon_pos = rest.find(": ")?;
    let id_str = &rest[..colon_pos];
    if id_str.is_empty() || id_str.contains(char::is_whitespace) {
        return None;
    }
    if id_str.chars().all(|c| c.is_ascii_digit()) {
        return None; // valid, handled by step_header_parts
    }
    Some((id_str, &rest[colon_pos + 2..]))
}

/// True for a well-formed Wave/Step header *or* a plausible-but-malformed
/// attempt at one, so a malformed header still ends the previous step's
/// boundary instead of being silently swallowed into its body.
fn is_header_token(line: &str) -> bool {
    wave_header_number(line).is_some()
        || step_header_parts(line).is_some()
        || malformed_wave_header(line).is_some()
        || malformed_step_header(line).is_some()
}

// --- step body parsing ---------------------------------------------------

struct RawFields {
    wave: u32,
    wave_seen: bool,
    files: Vec<String>,
    files_seen: bool,
    red: String,
    green: String,
    verify: String,
    change: String,
    depends_on: Vec<u32>,
}

/// Finds the boundary (exclusive end index) of the step whose body starts
/// at `start`: the next Wave/Step header token outside a fence, or EOF.
fn find_step_boundary(lines: &[&str], start: usize) -> usize {
    let mut idx = start;
    let mut fence: Option<&'static str> = None;
    while idx < lines.len() {
        let trimmed = lines[idx].trim();
        let toggled = toggle_fence(trimmed, fence);
        if toggled != fence || trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fence = toggled;
            idx += 1;
            continue;
        }
        if fence.is_none() && is_header_token(lines[idx]) {
            break;
        }
        idx += 1;
    }
    idx
}

fn collect_step_fields(
    lines: &[&str],
    start: usize,
    end: usize,
    step_num: u32,
    findings: &mut Vec<Finding>,
) -> RawFields {
    let mut wave = 0u32;
    let mut wave_seen = false;
    let mut files = Vec::new();
    let mut files_seen = false;
    let mut red = String::new();
    let mut green = String::new();
    let mut verify = String::new();
    let mut change = String::new();
    let mut depends_on = Vec::new();

    let mut idx = start;
    let mut fence: Option<&'static str> = None;
    while idx < end {
        let line = lines[idx];
        let trimmed = line.trim();

        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fence = toggle_fence(trimmed, fence);
            idx += 1;
            continue;
        }
        if fence.is_some() {
            idx += 1;
            continue;
        }

        if line.starts_with("- **Wave**: ") {
            wave_seen = true;
            wave = parse_field_value(line, "- **Wave**: ", &mut idx, step_num, findings);
        } else if line.starts_with("- **Files**: ") {
            files_seen = true;
            let files_str = collect_field_value(line, "- **Files**: ", lines, &mut idx, end);
            let trimmed_files: Vec<String> =
                files_str.split(',').map(|s| s.trim().to_string()).collect();
            if trimmed_files.iter().any(|f| f.is_empty()) {
                findings.push(Finding {
                    step: step_num,
                    message: "Files line contains a blank entry".to_string(),
                });
            }
            files = trimmed_files
                .into_iter()
                .filter(|f| !f.is_empty())
                .collect();
        } else if line.starts_with("- **Red**: ") {
            red = collect_field_value(line, "- **Red**: ", lines, &mut idx, end);
        } else if line.starts_with("- **Green**: ") {
            green = collect_field_value(line, "- **Green**: ", lines, &mut idx, end);
        } else if line.starts_with("- **Verify**: ") {
            verify = collect_field_value(line, "- **Verify**: ", lines, &mut idx, end);
        } else if line.starts_with("- **Change**: ") {
            change = collect_field_value(line, "- **Change**: ", lines, &mut idx, end);
        } else if line.starts_with("- Depends on: ") {
            let depends_str = collect_field_value(line, "- Depends on: ", lines, &mut idx, end);
            depends_on = parse_depends_on(&depends_str, step_num, findings);
        } else {
            idx += 1;
        }
    }

    RawFields {
        wave,
        wave_seen,
        files,
        files_seen,
        red,
        green,
        verify,
        change,
        depends_on,
    }
}

fn resolve_shape(
    red: String,
    green: String,
    change: String,
    verify: String,
    step_num: u32,
    findings: &mut Vec<Finding>,
) -> Shape {
    let has_red_green = !red.is_empty() || !green.is_empty();
    let has_change = !change.is_empty();

    if has_red_green && has_change {
        findings.push(Finding {
            step: step_num,
            message: "ambiguous shape: both Red-Green and Change fields present".to_string(),
        });
        return Shape::RedGreen {
            red: String::new(),
            green: String::new(),
            verify: String::new(),
        };
    }

    if has_red_green {
        if red.is_empty() {
            findings.push(Finding {
                step: step_num,
                message: "missing mandatory field 'Red' for Red-Green shape".to_string(),
            });
        }
        if green.is_empty() {
            findings.push(Finding {
                step: step_num,
                message: "missing mandatory field 'Green' for Red-Green shape".to_string(),
            });
        }
        if verify.is_empty() {
            findings.push(Finding {
                step: step_num,
                message: "missing mandatory field 'Verify' for Red-Green shape".to_string(),
            });
        }
        return Shape::RedGreen { red, green, verify };
    }

    if has_change {
        if verify.is_empty() {
            findings.push(Finding {
                step: step_num,
                message: "missing mandatory field 'Verify' for Change shape".to_string(),
            });
        }
        return Shape::Change { change, verify };
    }

    findings.push(Finding {
        step: step_num,
        message: "unknown shape: neither Red-Green nor Change fields present".to_string(),
    });
    Shape::RedGreen {
        red: String::new(),
        green: String::new(),
        verify: String::new(),
    }
}

#[allow(clippy::too_many_arguments)]
fn parse_step(
    lines: &[&str],
    header_line: usize,
    step_end: usize,
    step_num: u32,
    title: &str,
    enclosing_wave: u32,
    findings: &mut Vec<Finding>,
) -> Step {
    let fields = collect_step_fields(lines, header_line + 1, step_end, step_num, findings);

    if !fields.wave_seen {
        findings.push(Finding {
            step: step_num,
            message: "missing mandatory field 'Wave'".to_string(),
        });
    }
    if !fields.files_seen {
        findings.push(Finding {
            step: step_num,
            message: "missing mandatory field 'Files'".to_string(),
        });
    }

    let shape = resolve_shape(
        fields.red,
        fields.green,
        fields.change,
        fields.verify,
        step_num,
        findings,
    );

    Step {
        number: step_num,
        title: title.to_string(),
        wave: fields.wave,
        enclosing_wave,
        files: fields.files,
        shape,
        depends_on: fields.depends_on,
        raw: lines[header_line..step_end].join("\n"),
    }
}

fn parse_field_value(
    line: &str,
    prefix: &str,
    idx: &mut usize,
    step_num: u32,
    findings: &mut Vec<Finding>,
) -> u32 {
    let rest = &line[prefix.len()..];
    let value = rest.trim().parse::<u32>().unwrap_or_else(|_| {
        findings.push(Finding {
            step: step_num,
            message: format!(
                "invalid Wave value '{}': must be a non-negative integer",
                rest.trim()
            ),
        });
        0
    });
    *idx += 1;
    value
}

/// Collects a field's value plus any continuation lines (lines that don't
/// start a new field bullet), preserving fenced blocks verbatim.
fn collect_field_value(
    line: &str,
    prefix: &str,
    lines: &[&str],
    idx: &mut usize,
    step_end: usize,
) -> String {
    let mut value = line[prefix.len()..].to_string();
    *idx += 1;
    let mut in_fence: Option<&'static str> = None;

    while *idx < step_end {
        let next_line = lines[*idx];
        let next_trimmed = next_line.trim();

        if next_trimmed.starts_with("```") || next_trimmed.starts_with("~~~") {
            in_fence = toggle_fence(next_trimmed, in_fence);
            if !value.is_empty() {
                value.push('\n');
            }
            value.push_str(next_line);
            *idx += 1;
            continue;
        }
        if in_fence.is_some() {
            if !value.is_empty() {
                value.push('\n');
            }
            value.push_str(next_line);
            *idx += 1;
            continue;
        }

        if next_line.starts_with("- **") || next_line.starts_with("- Depends on: ") {
            break;
        }
        if next_line.starts_with("- ")
            && next_line[2..]
                .chars()
                .next()
                .is_some_and(|c| c.is_alphabetic())
        {
            break;
        }
        if next_line.is_empty() {
            break;
        }
        if !value.is_empty() {
            value.push('\n');
        }
        value.push_str(next_line);
        *idx += 1;
    }

    value
}

fn parse_depends_on(value: &str, step_num: u32, findings: &mut Vec<Finding>) -> Vec<u32> {
    let trimmed = value.trim();
    if trimmed == "none" {
        return Vec::new();
    }
    if trimmed.is_empty() {
        findings.push(Finding {
            step: step_num,
            message: "malformed Depends on syntax".to_string(),
        });
        return Vec::new();
    }

    let mut depends_on = Vec::new();
    let mut has_error = false;

    for token in trimmed.split(',') {
        let token = token.trim();
        if let Some(num_str) = token.strip_prefix("step ") {
            if let Ok(num) = num_str.parse::<u32>() {
                depends_on.push(num);
            } else {
                has_error = true;
                break;
            }
        } else {
            has_error = true;
            break;
        }
    }

    if has_error {
        findings.push(Finding {
            step: step_num,
            message: "malformed Depends on syntax".to_string(),
        });
        return Vec::new();
    }

    depends_on
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ignores_unfenced_near_miss_header_lines_inside_field_text() {
        // Regression test: a line that merely *starts with* "## Wave " or
        // "### Step "/"## Step " but doesn't satisfy the full anchored
        // grammar (digits-only wave number; digits+": " step header) must
        // not be treated as a boundary token, even outside a fence.
        let text = "\
## Wave 1

### Step 1: document the grammar
- **Wave**: 1
- **Files**: /tmp/a.rs
- **Change**: see the note below.
  ## Wave 99 is not a real header here, just prose.
  ### Step abc: not a real step header either.
  more change text.
- **Verify**: cargo build
- Depends on: none
";
        let score = parse(text).expect("should parse");
        assert_eq!(
            score.steps.len(),
            1,
            "near-miss header-like lines must not split the step"
        );
        match &score.steps[0].shape {
            Shape::Change { change, verify } => {
                assert!(
                    change.contains("## Wave 99 is not a real header here"),
                    "near-miss line must survive verbatim inside the Change field, got: {:?}",
                    change
                );
                assert_eq!(verify.trim(), "cargo build");
            }
            other => panic!("expected Change shape, got {:?}", other),
        }
    }

    #[test]
    fn parse_flags_malformed_step_header_instead_of_dropping_the_step() {
        let text = "\
## Wave 1

### Step abc: add widget
- **Wave**: 1
- **Files**: /tmp/a.rs
- **Change**: add widget.
- **Verify**: cargo build
- Depends on: none
";
        let findings = parse(text).expect_err("should fail to parse");
        assert!(
            findings.iter().any(|f| f.message.contains("abc")
                && f.message.to_lowercase().contains("step")
                && f.message.to_lowercase().contains("malformed")),
            "expected a malformed-step-header finding naming the bad id, got: {:?}",
            findings
        );
    }

    #[test]
    fn parse_flags_malformed_wave_header_directly() {
        let text = "\
## Wave one

### Step 1: add widget
- **Wave**: 1
- **Files**: /tmp/a.rs
- **Change**: add widget.
- **Verify**: cargo build
- Depends on: none
";
        let findings = parse(text).expect_err("should fail to parse");
        assert!(
            findings.iter().any(|f| f.message.contains("one")
                && f.message.to_lowercase().contains("wave")
                && f.message.to_lowercase().contains("malformed")),
            "expected a malformed-Wave-header finding naming the bad token, got: {:?}",
            findings
        );
    }

    #[test]
    fn parse_single_red_green_step_under_one_wave() {
        let text = "\
# Score: demo

## Wave 1

### Step 1: add widget
- **Wave**: 1
- **Files**: /tmp/a.rs
- **Red**: write test for widget.
- **Green**: minimal change to add widget.
- **Verify**: cargo test -- --exact widget_test
- Depends on: none
";
        let score = parse(text).expect("should parse");
        assert_eq!(score.steps.len(), 1);
        let step = &score.steps[0];
        assert_eq!(step.number, 1);
        assert_eq!(step.title, "add widget");
        assert_eq!(step.wave, 1);
        assert_eq!(step.enclosing_wave, 1);
        assert_eq!(step.files, vec!["/tmp/a.rs".to_string()]);
        assert_eq!(step.depends_on, Vec::<u32>::new());
        match &step.shape {
            Shape::RedGreen { red, green, verify } => {
                assert_eq!(red.trim(), "write test for widget.");
                assert_eq!(green.trim(), "minimal change to add widget.");
                assert_eq!(verify.trim(), "cargo test -- --exact widget_test");
            }
            other => panic!("expected RedGreen shape, got {:?}", other),
        }
    }

    #[test]
    fn parse_change_shape_step() {
        let text = "\
## Wave 1

### Step 1: wire config
- **Wave**: 1
- **Files**: /tmp/a.rs
- **Change**: wire the config loader.
- **Verify**: cargo build
- Depends on: none
";
        let score = parse(text).expect("should parse");
        match &score.steps[0].shape {
            Shape::Change { change, verify } => {
                assert_eq!(change.trim(), "wire the config loader.");
                assert_eq!(verify.trim(), "cargo build");
            }
            other => panic!("expected Change shape, got {:?}", other),
        }
    }

    #[test]
    fn parse_flags_unknown_shape_when_neither_red_green_nor_change_present() {
        let text = "\
## Wave 1

### Step 1: mystery step
- **Wave**: 1
- **Files**: /tmp/a.rs
- **Verify**: cargo build
- Depends on: none
";
        let findings = parse(text).expect_err("should fail to parse");
        assert!(
            findings
                .iter()
                .any(|f| f.step == 1 && f.message.to_lowercase().contains("shape")),
            "expected a shape-related finding, got: {:?}",
            findings
        );
    }

    #[test]
    fn parse_flags_duplicate_step_number() {
        let text = "\
## Wave 1

### Step 1: add widget
- **Wave**: 1
- **Files**: /tmp/a.rs
- **Change**: add widget.
- **Verify**: cargo build
- Depends on: none

### Step 1: add widget again
- **Wave**: 1
- **Files**: /tmp/b.rs
- **Change**: add widget again.
- **Verify**: cargo build
- Depends on: none
";
        let findings = parse(text).expect_err("should fail to parse");
        assert!(
            findings
                .iter()
                .any(|f| f.step == 1 && f.message.to_lowercase().contains("duplicate")),
            "expected a duplicate-step-number finding, got: {:?}",
            findings
        );
    }

    #[test]
    fn parse_flags_missing_wave_field() {
        let text = "\
## Wave 1

### Step 1: add widget
- **Files**: /tmp/a.rs
- **Change**: add widget.
- **Verify**: cargo build
- Depends on: none
";
        let findings = parse(text).expect_err("should fail to parse");
        assert!(
            findings.iter().any(|f| f.step == 1
                && f.message.to_lowercase().contains("wave")
                && f.message.to_lowercase().contains("missing")),
            "expected a missing-Wave-field finding, got: {:?}",
            findings
        );
    }

    #[test]
    fn parse_flags_missing_files_field() {
        let text = "\
## Wave 1

### Step 1: add widget
- **Wave**: 1
- **Change**: add widget.
- **Verify**: cargo build
- Depends on: none
";
        let findings = parse(text).expect_err("should fail to parse");
        assert!(
            findings.iter().any(|f| f.step == 1
                && f.message.to_lowercase().contains("files")
                && f.message.to_lowercase().contains("missing")),
            "expected a missing-Files-field finding, got: {:?}",
            findings
        );
    }

    #[test]
    fn parse_flags_blank_entry_in_files_line() {
        let text = "\
## Wave 1

### Step 1: add widget
- **Wave**: 1
- **Files**: /tmp/a.rs, , /tmp/b.rs
- **Change**: add widget.
- **Verify**: cargo build
- Depends on: none
";
        let findings = parse(text).expect_err("should fail to parse");
        assert!(
            findings
                .iter()
                .any(|f| f.step == 1 && f.message.to_lowercase().contains("blank")),
            "expected a blank-Files-entry finding, got: {:?}",
            findings
        );
    }

    #[test]
    fn parse_flags_missing_green_for_red_green_shape() {
        let text = "\
## Wave 1

### Step 1: add widget
- **Wave**: 1
- **Files**: /tmp/a.rs
- **Red**: write test for widget.
- **Verify**: cargo test -- --exact widget_test
- Depends on: none
";
        let findings = parse(text).expect_err("should fail to parse");
        assert!(
            findings.iter().any(|f| f.step == 1
                && f.message.contains("Green")
                && f.message.to_lowercase().contains("red-green")),
            "expected a missing-Green-field finding, got: {:?}",
            findings
        );
    }

    #[test]
    fn parse_flags_missing_verify_for_change_shape() {
        let text = "\
## Wave 1

### Step 1: wire config
- **Wave**: 1
- **Files**: /tmp/a.rs
- **Change**: wire the config loader.
- Depends on: none
";
        let findings = parse(text).expect_err("should fail to parse");
        assert!(
            findings.iter().any(|f| f.step == 1
                && f.message.contains("Verify")
                && f.message.to_lowercase().contains("change")),
            "expected a missing-Verify-field finding, got: {:?}",
            findings
        );
    }

    #[test]
    fn parse_depends_on_none_and_multi_dep() {
        let text = "\
## Wave 1

### Step 1: add widget
- **Wave**: 1
- **Files**: /tmp/a.rs
- **Change**: add widget.
- **Verify**: cargo build
- Depends on: none

### Step 2: add gadget
- **Wave**: 1
- **Files**: /tmp/b.rs
- **Change**: add gadget.
- **Verify**: cargo build
- Depends on: step 1

## Wave 2

### Step 3: combine
- **Wave**: 2
- **Files**: /tmp/c.rs
- **Change**: combine widget and gadget.
- **Verify**: cargo build
- Depends on: step 1, step 2
";
        let score = parse(text).expect("should parse");
        assert_eq!(score.steps[0].depends_on, Vec::<u32>::new());
        assert_eq!(score.steps[1].depends_on, vec![1]);
        assert_eq!(score.steps[2].depends_on, vec![1, 2]);
    }

    #[test]
    fn parse_flags_malformed_depends_on_syntax() {
        let text = "\
## Wave 1

### Step 1: add widget
- **Wave**: 1
- **Files**: /tmp/a.rs
- **Change**: add widget.
- **Verify**: cargo build
- Depends on: whatever
";
        let findings = parse(text).expect_err("should fail to parse");
        assert!(
            findings.iter().any(|f| f.step == 1
                && f.message.to_lowercase().contains("depends on")
                && f.message.to_lowercase().contains("malformed")),
            "expected a malformed-Depends-on finding, got: {:?}",
            findings
        );
    }

    #[test]
    fn parse_accepts_flat_step_header_level() {
        let text = "\
## Wave 1

## Step 1: add widget
- **Wave**: 1
- **Files**: /tmp/a.rs
- **Change**: add widget.
- **Verify**: cargo build
- Depends on: none
";
        let score = parse(text).expect("should parse");
        assert_eq!(score.steps.len(), 1);
        assert_eq!(score.steps[0].number, 1);
        assert_eq!(score.steps[0].title, "add widget");
    }

    #[test]
    fn parse_ignores_headers_and_field_bullets_inside_a_fenced_example() {
        let text = "\
## Wave 1

### Step 1: document the grammar
- **Wave**: 1
- **Files**: /tmp/a.rs
- **Change**: add an example block:
~~~markdown
## Wave 99
### Step 99: fake step
- **Files**: should not be parsed
~~~
end of example.
- **Verify**: cargo build
- Depends on: none
";
        let score = parse(text).expect("should parse");
        assert_eq!(
            score.steps.len(),
            1,
            "the fenced ## Wave 99 / ### Step 99 lines must not be treated as real tokens"
        );
        match &score.steps[0].shape {
            Shape::Change { change, .. } => {
                assert!(
                    change.contains("## Wave 99"),
                    "the fenced example text must survive verbatim inside the Change field, got: {:?}",
                    change
                );
            }
            other => panic!("expected Change shape, got {:?}", other),
        }
    }

    #[test]
    fn parse_captures_raw_verbatim_step_slice() {
        let text = "\
## Wave 1

### Step 1: add widget
- **Wave**: 1
- **Files**: /tmp/a.rs
- **Change**: add widget.
- **Verify**: cargo build
- Depends on: none

### Step 2: add gadget
- **Wave**: 1
- **Files**: /tmp/b.rs
- **Change**: add gadget.
- **Verify**: cargo build
- Depends on: none
";
        let score = parse(text).expect("should parse");
        assert!(
            score.steps[0].raw.starts_with("### Step 1: add widget"),
            "raw should start at the step's own header line, got: {:?}",
            score.steps[0].raw
        );
        assert!(
            !score.steps[0].raw.contains("### Step 2"),
            "raw must stop before the next step's header, got: {:?}",
            score.steps[0].raw
        );
        assert!(score.steps[1].raw.starts_with("### Step 2: add gadget"));
    }
}

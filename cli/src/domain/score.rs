use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct Score {
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub number: u32,
    pub title: String,
    pub wave: u32,
    pub enclosing_wave: u32,
    pub files: Vec<String>,
    pub shape: Shape,
    pub depends_on: Vec<u32>,
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    RedGreen {
        red: String,
        green: String,
        verify: String,
    },
    Change {
        change: String,
        verify: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub step: u32,
    pub message: String,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "step {}: {}", self.step, self.message)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoSuchWave(pub u32);

impl fmt::Display for NoSuchWave {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "no wave {} in score", self.0)
    }
}

pub fn parse(text: &str) -> Result<Score, Vec<Finding>> {
    let lines: Vec<&str> = text.lines().collect();
    let mut steps = Vec::new();
    let mut findings = Vec::new();
    let mut i = 0;
    let mut current_enclosing_wave = 0u32;
    let mut in_fence: Option<&'static str> = None;
    let mut seen_numbers = std::collections::HashSet::new();

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        // Track fence state using trimmed line for fence detection
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let fence_marker = if trimmed.starts_with("```") {
                "```"
            } else {
                "~~~"
            };
            if in_fence == Some(fence_marker) {
                in_fence = None;
            } else if in_fence.is_none() {
                in_fence = Some(fence_marker);
            }
            i += 1;
            continue;
        }

        // Skip Wave/Step evaluation if inside a fence
        if in_fence.is_some() {
            i += 1;
            continue;
        }

        // Check for Wave header: "## Wave " followed by digits
        if let Some(rest) = line.strip_prefix("## Wave ") {
            if let Ok(wave_num) = rest.trim().parse::<u32>() {
                current_enclosing_wave = wave_num;
            }
            i += 1;
            continue;
        }

        // Check for Step header: "### Step " or "## Step " followed by digits, ": ", then title
        if let Some(rest) = line
            .strip_prefix("### Step ")
            .or_else(|| line.strip_prefix("## Step "))
        {
            if let Some(colon_pos) = rest.find(": ") {
                if let Ok(step_num) = rest[..colon_pos].parse::<u32>() {
                    let title = rest[colon_pos + 2..].to_string();
                    let step_header_line = i;
                    i += 1;

                    // Scan forward to find step boundary (next Wave or Step header, or EOF)
                    let mut step_end = i;
                    let mut scan_fence: Option<&'static str> = None;
                    while step_end < lines.len() {
                        let next_line = lines[step_end];
                        let next_trimmed = next_line.trim();

                        // Track fence state during step boundary scan
                        if next_trimmed.starts_with("```") || next_trimmed.starts_with("~~~") {
                            let fence_marker = if next_trimmed.starts_with("```") {
                                "```"
                            } else {
                                "~~~"
                            };
                            if scan_fence == Some(fence_marker) {
                                scan_fence = None;
                            } else if scan_fence.is_none() {
                                scan_fence = Some(fence_marker);
                            }
                            step_end += 1;
                            continue;
                        }

                        // Only check for headers if not inside a fence
                        if scan_fence.is_none()
                            && (next_line.starts_with("## Wave ")
                                || next_line.starts_with("### Step ")
                                || next_line.starts_with("## Step "))
                        {
                            break;
                        }
                        step_end += 1;
                    }

                    // Extract fields from body lines (everything after header)
                    let mut wave = 0u32;
                    let mut wave_seen = false;
                    let mut files = Vec::new();
                    let mut files_seen = false;
                    let mut red = String::new();
                    let mut green = String::new();
                    let mut verify = String::new();
                    let mut change = String::new();
                    let mut depends_on = Vec::new();

                    let mut field_idx = i;
                    let mut field_fence: Option<&'static str> = None;
                    while field_idx < step_end {
                        let field_line = lines[field_idx];
                        let field_trimmed = field_line.trim();

                        // Track fence state in field collection
                        if field_trimmed.starts_with("```") || field_trimmed.starts_with("~~~") {
                            let fence_marker = if field_trimmed.starts_with("```") {
                                "```"
                            } else {
                                "~~~"
                            };
                            if field_fence == Some(fence_marker) {
                                field_fence = None;
                            } else if field_fence.is_none() {
                                field_fence = Some(fence_marker);
                            }
                            field_idx += 1;
                            continue;
                        }

                        // Skip field marker detection if inside a fence
                        if field_fence.is_some() {
                            field_idx += 1;
                            continue;
                        }

                        if field_line.starts_with("- **Wave**: ") {
                            wave_seen = true;
                            wave = parse_field_value(
                                field_line,
                                "- **Wave**: ",
                                &lines,
                                &mut field_idx,
                            );
                        } else if field_line.starts_with("- **Files**: ") {
                            files_seen = true;
                            let files_str = collect_field_value(
                                field_line,
                                "- **Files**: ",
                                &lines,
                                &mut field_idx,
                                step_end,
                            );
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
                        } else if field_line.starts_with("- **Red**: ") {
                            red = collect_field_value(
                                field_line,
                                "- **Red**: ",
                                &lines,
                                &mut field_idx,
                                step_end,
                            );
                        } else if field_line.starts_with("- **Green**: ") {
                            green = collect_field_value(
                                field_line,
                                "- **Green**: ",
                                &lines,
                                &mut field_idx,
                                step_end,
                            );
                        } else if field_line.starts_with("- **Verify**: ") {
                            verify = collect_field_value(
                                field_line,
                                "- **Verify**: ",
                                &lines,
                                &mut field_idx,
                                step_end,
                            );
                        } else if field_line.starts_with("- **Change**: ") {
                            change = collect_field_value(
                                field_line,
                                "- **Change**: ",
                                &lines,
                                &mut field_idx,
                                step_end,
                            );
                        } else if field_line.starts_with("- Depends on: ") {
                            let depends_str = collect_field_value(
                                field_line,
                                "- Depends on: ",
                                &lines,
                                &mut field_idx,
                                step_end,
                            );
                            depends_on = parse_depends_on(&depends_str, step_num, &mut findings);
                        } else {
                            field_idx += 1;
                        }
                    }

                    if !wave_seen {
                        findings.push(Finding {
                            step: step_num,
                            message: "missing mandatory field 'Wave'".to_string(),
                        });
                    }

                    if !files_seen {
                        findings.push(Finding {
                            step: step_num,
                            message: "missing mandatory field 'Files'".to_string(),
                        });
                    }

                    let shape = if (!red.is_empty() || !green.is_empty()) && !change.is_empty() {
                        findings.push(Finding {
                            step: step_num,
                            message: "ambiguous shape: both Red-Green and Change fields present"
                                .to_string(),
                        });
                        Shape::RedGreen {
                            red: String::new(),
                            green: String::new(),
                            verify: String::new(),
                        }
                    } else if !red.is_empty()
                        && !green.is_empty()
                        && !verify.is_empty()
                        && change.is_empty()
                    {
                        Shape::RedGreen { red, green, verify }
                    } else if !change.is_empty()
                        && red.is_empty()
                        && green.is_empty()
                        && !verify.is_empty()
                    {
                        Shape::Change { change, verify }
                    } else if red.is_empty() && green.is_empty() && change.is_empty() {
                        findings.push(Finding {
                            step: step_num,
                            message: "unknown shape: neither Red-Green nor Change fields present"
                                .to_string(),
                        });
                        Shape::RedGreen {
                            red: String::new(),
                            green: String::new(),
                            verify: String::new(),
                        }
                    } else if (!red.is_empty() || !green.is_empty()) && change.is_empty() {
                        // RedGreen-leaning: detect missing mandatory fields
                        if red.is_empty() {
                            findings.push(Finding {
                                step: step_num,
                                message: "missing mandatory field 'Red' for Red-Green shape"
                                    .to_string(),
                            });
                        }
                        if green.is_empty() {
                            findings.push(Finding {
                                step: step_num,
                                message: "missing mandatory field 'Green' for Red-Green shape"
                                    .to_string(),
                            });
                        }
                        if verify.is_empty() {
                            findings.push(Finding {
                                step: step_num,
                                message: "missing mandatory field 'Verify' for Red-Green shape"
                                    .to_string(),
                            });
                        }
                        Shape::RedGreen { red, green, verify }
                    } else if !change.is_empty() && red.is_empty() && green.is_empty() {
                        // Change-leaning: detect missing mandatory fields
                        if verify.is_empty() {
                            findings.push(Finding {
                                step: step_num,
                                message: "missing mandatory field 'Verify' for Change shape"
                                    .to_string(),
                            });
                        }
                        Shape::Change { change, verify }
                    } else {
                        findings.push(Finding {
                            step: step_num,
                            message: "unrecognized step shape".to_string(),
                        });
                        Shape::RedGreen {
                            red: String::new(),
                            green: String::new(),
                            verify: String::new(),
                        }
                    };

                    if !seen_numbers.insert(step_num) {
                        findings.push(Finding {
                            step: step_num,
                            message: "duplicate step number".to_string(),
                        });
                    }

                    steps.push(Step {
                        number: step_num,
                        title,
                        wave,
                        enclosing_wave: current_enclosing_wave,
                        files,
                        shape,
                        depends_on,
                        raw: lines[step_header_line..step_end].join("\n"),
                    });

                    i = step_end;
                    continue;
                }
            }
        }

        i += 1;
    }

    if findings.is_empty() {
        Ok(Score { steps })
    } else {
        Err(findings)
    }
}

fn parse_field_value(line: &str, prefix: &str, _lines: &[&str], idx: &mut usize) -> u32 {
    let rest = &line[prefix.len()..];
    let value = rest.trim().parse::<u32>().unwrap_or(0);
    *idx += 1;
    value
}

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

    // Collect continuation lines (lines that don't start with "- **" or "- ")
    while *idx < step_end {
        let next_line = lines[*idx];
        let next_trimmed = next_line.trim();

        // Track fence state during continuation scanning
        if next_trimmed.starts_with("```") || next_trimmed.starts_with("~~~") {
            let fence_marker = if next_trimmed.starts_with("```") {
                "```"
            } else {
                "~~~"
            };
            if in_fence == Some(fence_marker) {
                in_fence = None;
            } else if in_fence.is_none() {
                in_fence = Some(fence_marker);
            }
            if !value.is_empty() {
                value.push('\n');
            }
            value.push_str(next_line);
            *idx += 1;
            continue;
        }

        // If inside a fence, treat as continuation regardless of format
        if in_fence.is_some() {
            if !value.is_empty() {
                value.push('\n');
            }
            value.push_str(next_line);
            *idx += 1;
            continue;
        }

        // Outside fence: apply normal continuation line rules
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

pub fn check(score: &Score) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut last_wave: Option<u32> = None;

    let mut step_counts: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    for step in &score.steps {
        *step_counts.entry(step.number).or_insert(0) += 1;
    }

    let wave_map: std::collections::HashMap<u32, u32> = score
        .steps
        .iter()
        .map(|step| (step.number, step.wave))
        .collect();

    let mut steps_by_wave: std::collections::HashMap<u32, Vec<usize>> =
        std::collections::HashMap::new();
    for (idx, step) in score.steps.iter().enumerate() {
        steps_by_wave
            .entry(step.enclosing_wave)
            .or_default()
            .push(idx);
    }

    for step in &score.steps {
        if let Some(previous) = last_wave {
            if step.enclosing_wave < previous {
                findings.push(Finding {
                    step: step.number,
                    message: format!(
                        "'## Wave {}' header is out of order (must be strictly ascending; previous wave header was {})",
                        step.enclosing_wave, previous
                    ),
                });
            }
        }

        if step.wave != step.enclosing_wave {
            findings.push(Finding {
                step: step.number,
                message: format!(
                    "Wave field ({}) does not match its enclosing '## Wave {}' header",
                    step.wave, step.enclosing_wave
                ),
            });
        }

        if let Some(&count) = step_counts.get(&step.number) {
            if count > 1 {
                findings.push(Finding {
                    step: step.number,
                    message: "duplicate step number appears more than once".to_string(),
                });
            }
        }

        for dep in &step.depends_on {
            if !wave_map.contains_key(dep) {
                findings.push(Finding {
                    step: step.number,
                    message: format!("depends on step {}, which does not exist", dep),
                });
            } else if wave_map[dep] >= step.wave {
                findings.push(Finding {
                    step: step.number,
                    message: format!(
                        "depends on step {}, which is not in a strictly-lower wave",
                        dep
                    ),
                });
            }
        }

        last_wave = Some(step.enclosing_wave);
    }

    for (wave, indices) in steps_by_wave.iter() {
        let mut sorted_indices = indices.clone();
        sorted_indices.sort_by_key(|&idx| score.steps[idx].number);

        for i in 0..sorted_indices.len() {
            for j in (i + 1)..sorted_indices.len() {
                let idx_a = sorted_indices[i];
                let idx_b = sorted_indices[j];
                let step_a = &score.steps[idx_a];
                let step_b = &score.steps[idx_b];

                for file_a in &step_a.files {
                    if step_b.files.contains(file_a) {
                        findings.push(Finding {
                            step: step_b.number,
                            message: format!(
                                "shares file {} with step {} in wave {}",
                                file_a, step_a.number, wave
                            ),
                        });
                    }
                }
            }
        }
    }

    findings
}

pub fn wave_blocks(_score: &Score, _wave: u32) -> Result<Vec<(u32, String)>, NoSuchWave> {
    todo!("implemented incrementally by later steps")
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn valid_change_step(number: u32, wave: u32, enclosing_wave: u32, file: &str) -> Step {
        Step {
            number,
            title: format!("step {}", number),
            wave,
            enclosing_wave,
            files: vec![file.to_string()],
            shape: Shape::Change {
                change: "do the thing.".to_string(),
                verify: "cargo build".to_string(),
            },
            depends_on: Vec::new(),
            raw: format!("### Step {}: step {}\n", number, number),
        }
    }

    #[test]
    fn check_flags_non_ascending_wave_headers() {
        let score = Score {
            steps: vec![
                valid_change_step(1, 2, 2, "/tmp/a.rs"),
                valid_change_step(2, 1, 1, "/tmp/b.rs"),
            ],
        };
        let findings = check(&score);
        assert!(
            findings.iter().any(|f| f.step == 2
                && f.message.to_lowercase().contains("wave")
                && f.message.to_lowercase().contains("ascending")),
            "expected a non-ascending-wave-header finding anchored to step 2, got: {:?}",
            findings
        );
    }

    #[test]
    fn check_flags_wave_field_mismatch_with_enclosing_header() {
        let score = Score {
            steps: vec![valid_change_step(1, 1, 2, "/tmp/a.rs")],
        };
        let findings = check(&score);
        assert!(
            findings.iter().any(|f| f.step == 1
                && f.message.contains('1')
                && f.message.contains('2')
                && f.message.to_lowercase().contains("wave")),
            "expected a Wave-field-mismatch finding for step 1 (wave=1, enclosing_wave=2), got: {:?}",
            findings
        );
    }

    #[test]
    fn check_flags_duplicate_step_numbers() {
        let score = Score {
            steps: vec![
                valid_change_step(3, 1, 1, "/tmp/a.rs"),
                valid_change_step(3, 1, 1, "/tmp/b.rs"),
            ],
        };
        let findings = check(&score);
        assert!(
            findings
                .iter()
                .any(|f| f.step == 3 && f.message.to_lowercase().contains("duplicate")),
            "expected a duplicate-step-number finding, got: {:?}",
            findings
        );
    }

    #[test]
    fn check_flags_dependency_on_nonexistent_step() {
        let mut step = valid_change_step(1, 1, 1, "/tmp/a.rs");
        step.depends_on = vec![99];
        let score = Score { steps: vec![step] };
        let findings = check(&score);
        assert!(
            findings.iter().any(|f| f.step == 1
                && f.message.contains("99")
                && f.message.to_lowercase().contains("not exist")),
            "expected a nonexistent-dependency finding, got: {:?}",
            findings
        );
    }

    #[test]
    fn check_flags_dependency_not_in_strictly_lower_wave() {
        let mut first = valid_change_step(1, 2, 2, "/tmp/a.rs");
        let mut second = valid_change_step(2, 2, 2, "/tmp/b.rs");
        second.depends_on = vec![1];
        first.depends_on = Vec::new();
        let score = Score {
            steps: vec![first, second],
        };
        let findings = check(&score);
        assert!(
            findings.iter().any(|f| f.step == 2
                && f.message.contains('1')
                && f.message.to_lowercase().contains("lower")),
            "expected a not-strictly-lower-wave dependency finding, got: {:?}",
            findings
        );
    }

    #[test]
    fn check_flags_shared_file_within_same_wave() {
        let mut first = valid_change_step(1, 1, 1, "/tmp/shared.rs");
        let mut second = valid_change_step(2, 1, 1, "/tmp/shared.rs");
        first.files = vec!["/tmp/shared.rs".to_string()];
        second.files = vec!["/tmp/shared.rs".to_string()];
        let score = Score {
            steps: vec![first, second],
        };
        let findings = check(&score);
        assert!(
            findings.iter().any(|f| f.step == 2
                && f.message.contains("/tmp/shared.rs")
                && f.message.contains('1')),
            "expected a file-disjointness finding naming the shared path and the other step, got: {:?}",
            findings
        );
    }

    #[test]
    fn check_allows_shared_file_across_different_waves() {
        let mut first = valid_change_step(1, 1, 1, "/tmp/shared.rs");
        let mut second = valid_change_step(2, 2, 2, "/tmp/shared.rs");
        first.files = vec!["/tmp/shared.rs".to_string()];
        second.files = vec!["/tmp/shared.rs".to_string()];
        let score = Score {
            steps: vec![first, second],
        };
        let findings = check(&score);
        assert!(
            !findings
                .iter()
                .any(|f| f.message.contains("/tmp/shared.rs")),
            "a file shared across different waves must not be flagged, got: {:?}",
            findings
        );
    }
}

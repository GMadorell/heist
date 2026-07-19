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

    while i < lines.len() {
        let line = lines[i];

        // Track fence state (placeholder for now)
        if line.starts_with("```") || line.starts_with("~~~") {
            let fence_marker = if line.starts_with("```") {
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

        // Check for Wave header: "## Wave " followed by digits
        if let Some(rest) = line.strip_prefix("## Wave ") {
            if let Ok(wave_num) = rest.trim().parse::<u32>() {
                current_enclosing_wave = wave_num;
            }
            i += 1;
            continue;
        }

        // Check for Step header: "### Step " followed by digits, ": ", then title
        if let Some(rest) = line.strip_prefix("### Step ") {
            if let Some(colon_pos) = rest.find(": ") {
                if let Ok(step_num) = rest[..colon_pos].parse::<u32>() {
                    let title = rest[colon_pos + 2..].to_string();
                    i += 1;

                    // Scan forward to find step boundary (next Wave or Step header, or EOF)
                    let mut step_end = i;
                    while step_end < lines.len() {
                        let next_line = lines[step_end];
                        if next_line.starts_with("## Wave ") || next_line.starts_with("### Step ") {
                            break;
                        }
                        step_end += 1;
                    }

                    // Extract fields from body lines (everything after header)
                    let mut wave = 0u32;
                    let mut files = Vec::new();
                    let mut red = String::new();
                    let mut green = String::new();
                    let mut verify = String::new();
                    let mut change = String::new();
                    let mut depends_on = Vec::new();

                    let mut field_idx = i;
                    while field_idx < step_end {
                        let field_line = lines[field_idx];

                        if field_line.starts_with("- **Wave**: ") {
                            wave = parse_field_value(
                                field_line,
                                "- **Wave**: ",
                                &lines,
                                &mut field_idx,
                            );
                        } else if field_line.starts_with("- **Files**: ") {
                            let files_str = collect_field_value(
                                field_line,
                                "- **Files**: ",
                                &lines,
                                &mut field_idx,
                            );
                            files = files_str.split(',').map(|s| s.trim().to_string()).collect();
                        } else if field_line.starts_with("- **Red**: ") {
                            red = collect_field_value(
                                field_line,
                                "- **Red**: ",
                                &lines,
                                &mut field_idx,
                            );
                        } else if field_line.starts_with("- **Green**: ") {
                            green = collect_field_value(
                                field_line,
                                "- **Green**: ",
                                &lines,
                                &mut field_idx,
                            );
                        } else if field_line.starts_with("- **Verify**: ") {
                            verify = collect_field_value(
                                field_line,
                                "- **Verify**: ",
                                &lines,
                                &mut field_idx,
                            );
                        } else if field_line.starts_with("- **Change**: ") {
                            change = collect_field_value(
                                field_line,
                                "- **Change**: ",
                                &lines,
                                &mut field_idx,
                            );
                        } else if field_line.starts_with("- Depends on: ") {
                            let depends_str = collect_field_value(
                                field_line,
                                "- Depends on: ",
                                &lines,
                                &mut field_idx,
                            );
                            depends_on = parse_depends_on(&depends_str);
                        } else {
                            field_idx += 1;
                        }
                    }

                    let shape = if !red.is_empty()
                        && !green.is_empty()
                        && !verify.is_empty()
                        && change.is_empty()
                    {
                        Shape::RedGreen { red, green, verify }
                    } else if !change.is_empty() && red.is_empty() && green.is_empty() {
                        Shape::Change { change, verify }
                    } else {
                        findings.push(Finding {
                            step: step_num,
                            message: "unrecognized step shape".to_string(),
                        });
                        // Use placeholder shape
                        Shape::RedGreen {
                            red: String::new(),
                            green: String::new(),
                            verify: String::new(),
                        }
                    };

                    steps.push(Step {
                        number: step_num,
                        title,
                        wave,
                        enclosing_wave: current_enclosing_wave,
                        files,
                        shape,
                        depends_on,
                        raw: String::new(),
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

fn collect_field_value(line: &str, prefix: &str, lines: &[&str], idx: &mut usize) -> String {
    let mut value = line[prefix.len()..].to_string();
    *idx += 1;

    // Collect continuation lines (lines that don't start with "- **" or "- ")
    while *idx < lines.len() {
        let next_line = lines[*idx];
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

fn parse_depends_on(value: &str) -> Vec<u32> {
    let trimmed = value.trim();
    if trimmed == "none" {
        return Vec::new();
    }

    trimmed
        .split(", ")
        .filter_map(|s| s.trim().parse::<u32>().ok())
        .collect()
}

pub fn check(_score: &Score) -> Vec<Finding> {
    todo!("implemented incrementally by later steps")
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
}

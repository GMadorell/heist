use super::{Finding, Score};

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
        match last_wave {
            None => {
                // Wheelman loops waves `1..=M`; a score.md whose first wave
                // header isn't `## Wave 1` would break that loop before it
                // even starts.
                if step.enclosing_wave != 1 {
                    findings.push(Finding {
                        step: step.number,
                        message: format!(
                            "wave numbering must start at 1 (found first wave {})",
                            step.enclosing_wave
                        ),
                    });
                }
            }
            Some(previous) if step.enclosing_wave != previous => {
                if step.enclosing_wave < previous {
                    findings.push(Finding {
                        step: step.number,
                        message: format!(
                            "'## Wave {}' header is out of order (must be strictly ascending; previous wave header was {})",
                            step.enclosing_wave, previous
                        ),
                    });
                } else if step.enclosing_wave > previous + 1 {
                    // Wheelman's `waves: M` / `1..=M` loop assumes wave
                    // numbers are contiguous, not just ascending; a gap
                    // would make it request a wave that doesn't exist.
                    findings.push(Finding {
                        step: step.number,
                        message: format!(
                            "wave numbers must be contiguous (no wave {} between wave {} and wave {})",
                            previous + 1,
                            previous,
                            step.enclosing_wave
                        ),
                    });
                }
            }
            _ => {}
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

    let mut waves_sorted: Vec<&u32> = steps_by_wave.keys().collect();
    waves_sorted.sort();
    for wave in waves_sorted {
        let indices = &steps_by_wave[wave];
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::score::{Shape, Step};

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
    fn check_flags_wave_number_gap() {
        // Waves 1 and 3 present, wave 2 missing: `wheelman.md` loops
        // `1..=M` off the distinct-wave count, so a gap would make it
        // request a wave that doesn't exist.
        let score = Score {
            steps: vec![
                valid_change_step(1, 1, 1, "/tmp/a.rs"),
                valid_change_step(2, 3, 3, "/tmp/b.rs"),
            ],
        };
        let findings = check(&score);
        assert!(
            findings.iter().any(|f| f.step == 2
                && f.message.contains('2')
                && f.message.to_lowercase().contains("contiguous")),
            "expected a wave-gap finding anchored to step 2, got: {:?}",
            findings
        );
    }

    #[test]
    fn check_flags_wave_numbering_not_starting_at_one() {
        let score = Score {
            steps: vec![valid_change_step(1, 2, 2, "/tmp/a.rs")],
        };
        let findings = check(&score);
        assert!(
            findings.iter().any(|f| f.step == 1
                && f.message.to_lowercase().contains("start")
                && f.message.to_lowercase().contains("wave")),
            "expected a wave-must-start-at-1 finding, got: {:?}",
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

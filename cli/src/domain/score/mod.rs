mod check;
mod parser;

pub use check::check;
pub use parser::parse;

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct Score {
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub number: u32,
    pub title: String,
    /// The wave this step declares via its own `- **Wave**: N` field line.
    pub wave: u32,
    /// The wave derived from the `## Wave N` header this step is nested
    /// under.
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

pub fn wave_blocks(score: &Score, wave: u32) -> Result<Vec<(u32, String)>, NoSuchWave> {
    let blocks: Vec<(u32, String)> = score
        .steps
        .iter()
        .filter(|s| s.enclosing_wave == wave)
        .map(|s| (s.number, s.raw.clone()))
        .collect();
    if blocks.is_empty() {
        Err(NoSuchWave(wave))
    } else {
        Ok(blocks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn wave_blocks_returns_numbered_raw_slices_for_the_requested_wave() {
        let mut first = valid_change_step(1, 1, 1, "/tmp/a.rs");
        let mut second = valid_change_step(2, 1, 1, "/tmp/b.rs");
        let third = valid_change_step(3, 2, 2, "/tmp/c.rs");
        first.raw = "AAA".to_string();
        second.raw = "BBB".to_string();
        let score = Score {
            steps: vec![first, second, third],
        };

        let blocks = wave_blocks(&score, 1).expect("wave 1 should exist");
        assert_eq!(blocks, vec![(1, "AAA".to_string()), (2, "BBB".to_string())]);

        let err = wave_blocks(&score, 3).expect_err("wave 3 should not exist");
        assert_eq!(err, NoSuchWave(3));
    }
}

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

pub fn parse(_text: &str) -> Result<Score, Vec<Finding>> {
    todo!("implemented incrementally by later steps")
}

pub fn check(_score: &Score) -> Vec<Finding> {
    todo!("implemented incrementally by later steps")
}

pub fn wave_blocks(_score: &Score, _wave: u32) -> Result<Vec<(u32, String)>, NoSuchWave> {
    todo!("implemented incrementally by later steps")
}

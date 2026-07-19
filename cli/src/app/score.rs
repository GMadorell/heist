use crate::domain::error::StateError;
use crate::domain::score::{self, Finding};
use crate::ports::state_repository::StateRepository;

pub struct CheckOutcome {
    pub steps: usize,
    pub waves: usize,
}

pub struct RecordOutcome {
    pub steps: usize,
    pub waves: usize,
}

#[allow(dead_code)]
enum LoadError {
    NoState,
    NoScore,
    Io(std::io::Error),
    Findings(Vec<Finding>),
}

#[allow(dead_code)]
fn load_and_check(
    repo: &dyn StateRepository,
    slug: &str,
) -> Result<(score::Score, usize), LoadError> {
    if !repo.exists(slug) {
        return Err(LoadError::NoState);
    }
    let text = repo.load_score(slug).map_err(LoadError::Io)?;
    let text = text.ok_or(LoadError::NoScore)?;
    let parsed = score::parse(&text).map_err(LoadError::Findings)?;
    let findings = score::check(&parsed);
    if !findings.is_empty() {
        return Err(LoadError::Findings(findings));
    }
    let waves: std::collections::BTreeSet<u32> =
        parsed.steps.iter().map(|s| s.enclosing_wave).collect();
    let waves_count = waves.len();
    Ok((parsed, waves_count))
}

pub enum CheckError {
    NoState,
    NoScore,
    Io(std::io::Error),
    Findings(Vec<Finding>),
}

impl From<LoadError> for CheckError {
    fn from(e: LoadError) -> Self {
        match e {
            LoadError::NoState => CheckError::NoState,
            LoadError::NoScore => CheckError::NoScore,
            LoadError::Io(e) => CheckError::Io(e),
            LoadError::Findings(f) => CheckError::Findings(f),
        }
    }
}

pub enum RecordError {
    NoState,
    NoScore,
    Io(std::io::Error),
    Findings(Vec<Finding>),
    Save(StateError),
}

impl From<LoadError> for RecordError {
    fn from(e: LoadError) -> Self {
        match e {
            LoadError::NoState => RecordError::NoState,
            LoadError::NoScore => RecordError::NoScore,
            LoadError::Io(e) => RecordError::Io(e),
            LoadError::Findings(f) => RecordError::Findings(f),
        }
    }
}

pub enum WaveError {
    NoState,
    NoScore,
    Io(std::io::Error),
    Findings(Vec<Finding>),
    NoSuchWave(u32),
}

pub fn check(_repo: &dyn StateRepository, _slug: &str) -> Result<CheckOutcome, CheckError> {
    todo!("implemented in a later step")
}

pub fn record(
    _repo: &dyn StateRepository,
    _clock: &dyn crate::ports::clock::Clock,
    _slug: &str,
) -> Result<RecordOutcome, RecordError> {
    todo!("implemented in a later step")
}

pub fn wave(
    _repo: &dyn StateRepository,
    _slug: &str,
    _n: u32,
) -> Result<Vec<(u32, String)>, WaveError> {
    todo!("implemented in a later step")
}

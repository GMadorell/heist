use crate::domain::error::StateError;
use crate::domain::score::{self, Finding};
use crate::ports::score_repository::ScoreRepository;
use crate::ports::state_repository::StateRepository;

#[derive(Debug)]
pub struct CheckOutcome {
    pub steps: usize,
    pub waves: usize,
}

#[derive(Debug)]
pub enum CheckError {
    NoState,
    NoScore,
    Io(std::io::Error),
    Findings(Vec<Finding>),
}

pub fn check(
    repo: &dyn StateRepository,
    scores: &dyn ScoreRepository,
    slug: &str,
) -> Result<CheckOutcome, CheckError> {
    let (parsed, waves) = load_and_check(repo, scores, slug)?;
    Ok(CheckOutcome {
        steps: parsed.steps.len(),
        waves,
    })
}

pub struct RecordOutcome {
    pub steps: usize,
    pub waves: usize,
}

#[derive(Debug)]
pub enum RecordError {
    NoState,
    NoScore,
    Io(std::io::Error),
    Findings(Vec<Finding>),
    Save(StateError),
}

pub fn record(
    repo: &dyn StateRepository,
    scores: &dyn ScoreRepository,
    clock: &dyn crate::ports::clock::Clock,
    slug: &str,
) -> Result<RecordOutcome, RecordError> {
    let (parsed, waves) = load_and_check(repo, scores, slug)?;
    let mut state = repo.load(slug).map_err(RecordError::Save)?;
    state.score_steps_total = crate::domain::value::ScoreStepsTotal::new(parsed.steps.len() as u32);
    state.score_waves_total = crate::domain::value::ScoreWavesTotal::new(waves as u32);
    state.updated = clock.today();
    repo.save(slug, &state).map_err(RecordError::Save)?;
    Ok(RecordOutcome {
        steps: parsed.steps.len(),
        waves,
    })
}

#[derive(Debug)]
pub enum WaveError {
    NoState,
    NoScore,
    Io(std::io::Error),
    Findings(Vec<Finding>),
    NoSuchWave(u32),
}

pub fn wave(
    repo: &dyn StateRepository,
    scores: &dyn ScoreRepository,
    slug: &str,
    n: u32,
) -> Result<Vec<(u32, String)>, WaveError> {
    if !repo.exists(slug) {
        return Err(WaveError::NoState);
    }
    let text = scores.load_score(slug).map_err(WaveError::Io)?;
    let text = text.ok_or(WaveError::NoScore)?;
    let parsed = score::parse(&text).map_err(WaveError::Findings)?;
    score::wave_blocks(&parsed, n).map_err(|score::NoSuchWave(n)| WaveError::NoSuchWave(n))
}

enum LoadError {
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

fn load_and_check(
    repo: &dyn StateRepository,
    scores: &dyn ScoreRepository,
    slug: &str,
) -> Result<(score::Score, usize), LoadError> {
    if !repo.exists(slug) {
        return Err(LoadError::NoState);
    }
    let text = scores.load_score(slug).map_err(LoadError::Io)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::testing::InMemoryStateRepository;
    use crate::domain::state::State;
    use crate::domain::value::DateValue;

    const VALID_SCORE: &str = "\
## Wave 1

### Step 1: add widget
- **Wave**: 1
- **Files**: /tmp/a.rs
- **Change**: add widget.
- **Verify**: cargo build
- Depends on: none
";

    const MALFORMED_SCORE: &str = "\
## Wave 1

### Step 1: add widget
- **Wave**: 1
- **Change**: add widget.
- **Verify**: cargo build
- Depends on: none
";

    fn fixed_date() -> DateValue {
        DateValue::parse("today", "2026-01-01").expect("valid date")
    }

    #[test]
    fn check_returns_steps_and_waves_for_a_valid_score() {
        let repo = InMemoryStateRepository::new()
            .with_state("foo", State::new("foo", fixed_date()).expect("valid slug"))
            .with_score("foo", VALID_SCORE);

        let outcome = check(&repo, &repo, "foo").expect("should check ok");
        assert_eq!(outcome.steps, 1);
        assert_eq!(outcome.waves, 1);
    }

    #[test]
    fn check_returns_findings_for_a_malformed_score() {
        let repo = InMemoryStateRepository::new()
            .with_state("foo", State::new("foo", fixed_date()).expect("valid slug"))
            .with_score("foo", MALFORMED_SCORE);

        let err = check(&repo, &repo, "foo").expect_err("should fail");
        match err {
            CheckError::Findings(findings) => assert!(!findings.is_empty()),
            _ => panic!("expected CheckError::Findings"),
        }
    }

    #[test]
    fn record_persists_totals_and_bumps_updated() {
        use crate::adapters::testing::FixedClock;

        let created = fixed_date();
        let today = DateValue::parse("today", "2026-01-02").expect("valid date");
        let repo = InMemoryStateRepository::new()
            .with_state("foo", State::new("foo", created).expect("valid slug"))
            .with_score("foo", VALID_SCORE);
        let clock = FixedClock(today.clone());

        let outcome = record(&repo, &repo, &clock, "foo").expect("should record ok");
        assert_eq!(outcome.steps, 1);
        assert_eq!(outcome.waves, 1);

        let saved = repo.get("foo").expect("state should exist");
        assert_eq!(saved.score_steps_total.to_string(), "1");
        assert_eq!(saved.score_waves_total.to_string(), "1");
        assert_eq!(saved.updated, today);
    }

    const TWO_WAVE_SCORE: &str = "\
## Wave 1

### Step 1: add widget
- **Wave**: 1
- **Files**: /tmp/a.rs
- **Change**: add widget.
- **Verify**: cargo build
- Depends on: none

## Wave 2

### Step 2: wire widget
- **Wave**: 2
- **Files**: /tmp/b.rs
- **Change**: wire widget.
- **Verify**: cargo build
- Depends on: step 1
";

    #[test]
    fn wave_returns_numbered_blocks_and_no_such_wave_error() {
        let repo = InMemoryStateRepository::new()
            .with_state("foo", State::new("foo", fixed_date()).expect("valid slug"))
            .with_score("foo", TWO_WAVE_SCORE);

        let blocks = wave(&repo, &repo, "foo", 1).expect("wave 1 should exist");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].0, 1);
        assert!(blocks[0].1.starts_with("### Step 1: add widget"));

        let err = wave(&repo, &repo, "foo", 3).expect_err("wave 3 should not exist");
        match err {
            WaveError::NoSuchWave(3) => {}
            _ => panic!("expected WaveError::NoSuchWave(3)"),
        }
    }
}

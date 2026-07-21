use crate::common::TempCwd;
use heist_cli::adapters::file_heist_dir_repository::FileHeistDirRepository;
use heist_cli::adapters::file_state_repository::FileStateRepository;
use heist_cli::domain::error::StateError;
use heist_cli::domain::state::State;
use heist_cli::domain::value::{DateValue, ScoreWave};
use heist_cli::ports::heist_dir_repository::HeistDirRepository;
use heist_cli::ports::state_repository::StateRepository;
use std::path::PathBuf;

fn fixed_date() -> DateValue {
    DateValue::parse("today", "2026-01-01").expect("valid date")
}

fn state_file_path(slug: &str) -> PathBuf {
    PathBuf::from(".heist").join(slug).join("state.json")
}

#[test]
fn exists_false_before_init() {
    let _cwd = TempCwd::new();
    let repo = FileStateRepository;
    assert!(!repo.exists("foo"));
}

#[test]
fn save_writes_state_file_visible_to_exists_and_load() {
    let _cwd = TempCwd::new();
    let repo = FileStateRepository;
    let state = State::new("foo", fixed_date()).expect("valid slug");
    FileHeistDirRepository
        .create("foo")
        .expect("create slug dir");

    repo.save("foo", &state).expect("save should succeed");

    assert!(repo.exists("foo"));
    assert!(state_file_path("foo").exists());
    assert_eq!(repo.load("foo").expect("load should succeed"), state);
}

#[test]
fn load_missing_slug_is_missing() {
    let _cwd = TempCwd::new();
    let repo = FileStateRepository;
    assert!(matches!(repo.load("nope"), Err(StateError::Missing)));
}

#[test]
fn load_unparseable_file_is_unparseable() {
    let _cwd = TempCwd::new();
    let repo = FileStateRepository;
    let path = state_file_path("foo");
    FileHeistDirRepository
        .create("foo")
        .expect("create slug dir");
    std::fs::write(&path, "not json").expect("write garbage state file");

    assert!(matches!(repo.load("foo"), Err(StateError::Unparseable(_))));
}

#[test]
fn save_then_load_roundtrips() {
    let _cwd = TempCwd::new();
    let repo = FileStateRepository;
    let mut state = State::new("foo", fixed_date()).expect("valid slug");
    state.score_wave = ScoreWave::new(3);
    FileHeistDirRepository
        .create("foo")
        .expect("create slug dir");

    repo.save("foo", &state).expect("save should succeed");

    assert_eq!(
        repo.load("foo").expect("load should succeed").score_wave,
        ScoreWave::new(3)
    );
}

#[test]
fn list_slugs_is_empty_when_dot_heist_is_missing() {
    let _cwd = TempCwd::new();
    let repo = FileStateRepository;
    assert_eq!(
        repo.list_slugs().expect("list_slugs should succeed"),
        vec![]
    );
}

#[test]
fn list_slugs_returns_initialised_slugs_sorted() {
    let _cwd = TempCwd::new();
    let repo = FileStateRepository;
    FileHeistDirRepository
        .create("zeta")
        .expect("create slug dir");
    repo.save(
        "zeta",
        &State::new("zeta", fixed_date()).expect("valid slug"),
    )
    .expect("save should succeed");
    FileHeistDirRepository
        .create("alpha")
        .expect("create slug dir");
    repo.save(
        "alpha",
        &State::new("alpha", fixed_date()).expect("valid slug"),
    )
    .expect("save should succeed");

    let slugs: Vec<String> = repo
        .list_slugs()
        .expect("list_slugs should succeed")
        .iter()
        .map(|s| s.to_string())
        .collect();

    assert_eq!(slugs, vec!["alpha".to_string(), "zeta".to_string()]);
}

#[test]
fn list_slugs_ignores_directories_without_a_state_file() {
    let _cwd = TempCwd::new();
    let repo = FileStateRepository;
    FileHeistDirRepository
        .create("foo")
        .expect("create slug dir");
    repo.save("foo", &State::new("foo", fixed_date()).expect("valid slug"))
        .expect("save should succeed");
    FileHeistDirRepository
        .create("empty-dir")
        .expect("create dir without state.json");

    let slugs: Vec<String> = repo
        .list_slugs()
        .expect("list_slugs should succeed")
        .iter()
        .map(|s| s.to_string())
        .collect();

    assert_eq!(slugs, vec!["foo".to_string()]);
}

#[test]
fn save_without_slug_dir_is_unreadable() {
    let _cwd = TempCwd::new();
    let repo = FileStateRepository;
    let state = State::new("foo", fixed_date()).expect("valid slug");

    assert!(matches!(
        repo.save("foo", &state),
        Err(StateError::Unreadable(_))
    ));
}

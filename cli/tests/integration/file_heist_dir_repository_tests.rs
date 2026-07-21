use crate::common::TempCwd;
use heist_cli::adapters::file_heist_dir_repository::FileHeistDirRepository;
use heist_cli::domain::error::StateError;
use heist_cli::domain::value::SlugValue;
use heist_cli::ports::heist_dir_repository::HeistDirRepository;
use std::path::PathBuf;

fn slug(s: &str) -> SlugValue {
    SlugValue::parse(s).expect("valid slug")
}

fn heist_dir_path(slug: &str) -> PathBuf {
    PathBuf::from(".heist").join(slug)
}

#[test]
fn create_creates_slug_dir() {
    let _cwd = TempCwd::new();
    let repo = FileHeistDirRepository;

    repo.create(&slug("foo")).expect("create should succeed");

    assert!(heist_dir_path("foo").exists());
}

#[test]
fn create_rejects_already_initialised_slug() {
    let _cwd = TempCwd::new();
    let repo = FileHeistDirRepository;
    repo.create(&slug("foo"))
        .expect("first create should succeed");

    assert!(matches!(
        repo.create(&slug("foo")),
        Err(StateError::AlreadyExists)
    ));
}

#[test]
fn create_rejects_pre_existing_empty_slug_dir() {
    let _cwd = TempCwd::new();
    let repo = FileHeistDirRepository;
    std::fs::create_dir_all(".heist/foo").expect("create empty slug dir");

    assert!(matches!(
        repo.create(&slug("foo")),
        Err(StateError::AlreadyExists)
    ));
}

#[test]
fn remove_deletes_slug_directory() {
    let _cwd = TempCwd::new();
    let repo = FileHeistDirRepository;
    repo.create(&slug("foo")).expect("create should succeed");
    std::fs::write(heist_dir_path("foo").join("state.json"), "{}")
        .expect("write placeholder state file");

    repo.remove(&slug("foo")).expect("remove should succeed");

    assert!(!heist_dir_path("foo").exists());
}

#[test]
fn remove_is_a_no_op_when_slug_dir_absent() {
    let _cwd = TempCwd::new();
    let repo = FileHeistDirRepository;
    assert!(repo.remove(&slug("nope")).is_ok());
}

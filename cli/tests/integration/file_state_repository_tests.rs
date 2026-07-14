use heist_cli::domain::value::ScoreStep;
use heist_cli::domain::error::StateError;
use heist_cli::domain::state::State;
use heist_cli::ports::state_repository::StateRepository;
use heist_cli::state_repository::FileStateRepository;
use std::path::PathBuf;
use std::sync::Mutex;
use tempfile::TempDir;

// `FileStateRepository` resolves paths relative to the process cwd
// (joining onto `.heist/<slug>/state.json`), so every test here must chdir.
// The cwd is process-global, so tests that chdir must not run concurrently
// with each other; this lock serializes just those tests.
static CWD_LOCK: Mutex<()> = Mutex::new(());

fn state_file_path(slug: &str) -> PathBuf {
    PathBuf::from(".heist").join(slug).join("state.json")
}

/// Chdirs into a fresh temp dir for the guard's lifetime, restoring the
/// original cwd and releasing the lock on drop.
struct TempCwd {
    _lock: std::sync::MutexGuard<'static, ()>,
    _dir: TempDir,
    original: std::path::PathBuf,
}

impl TempCwd {
    fn new() -> Self {
        let lock = CWD_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let original = std::env::current_dir().expect("read cwd");
        let dir = TempDir::new().expect("create tempdir");
        std::env::set_current_dir(dir.path()).expect("chdir into tempdir");
        TempCwd {
            _lock: lock,
            _dir: dir,
            original,
        }
    }
}

impl Drop for TempCwd {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.original);
    }
}

#[test]
fn exists_false_before_init() {
    let _cwd = TempCwd::new();
    let repo = FileStateRepository;
    assert!(!repo.exists("foo"));
}

#[test]
fn init_creates_state_file_visible_to_exists_and_load() {
    let _cwd = TempCwd::new();
    let repo = FileStateRepository;
    let state = State::new("foo").expect("valid slug");

    repo.init("foo", &state).expect("init should succeed");

    assert!(repo.exists("foo"));
    assert!(state_file_path("foo").exists());
    assert_eq!(repo.load("foo").expect("load should succeed"), state);
}

#[test]
fn init_rejects_already_initialised_slug() {
    let _cwd = TempCwd::new();
    let repo = FileStateRepository;
    let state = State::new("foo").expect("valid slug");
    repo.init("foo", &state).expect("first init should succeed");

    assert!(matches!(
        repo.init("foo", &state),
        Err(StateError::AlreadyExists)
    ));
}

#[test]
fn init_rejects_pre_existing_empty_slug_dir() {
    let _cwd = TempCwd::new();
    let repo = FileStateRepository;
    std::fs::create_dir_all(".heist/foo").expect("create empty slug dir");

    let state = State::new("foo").expect("valid slug");
    assert!(matches!(
        repo.init("foo", &state),
        Err(StateError::AlreadyExists)
    ));
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
    std::fs::create_dir_all(path.parent().expect("state path has a parent"))
        .expect("create slug dir");
    std::fs::write(&path, "not json").expect("write garbage state file");

    assert!(matches!(repo.load("foo"), Err(StateError::Unparseable(_))));
}

#[test]
fn save_then_load_roundtrips() {
    let _cwd = TempCwd::new();
    let repo = FileStateRepository;
    let mut state = State::new("foo").expect("valid slug");
    state.score_step = ScoreStep::new(3);
    std::fs::create_dir_all(".heist/foo").expect("create slug dir");

    repo.save("foo", &state).expect("save should succeed");

    assert_eq!(
        repo.load("foo").expect("load should succeed").score_step,
        ScoreStep::new(3)
    );
}

#[test]
fn save_without_slug_dir_is_unreadable() {
    let _cwd = TempCwd::new();
    let repo = FileStateRepository;
    let state = State::new("foo").expect("valid slug");

    assert!(matches!(
        repo.save("foo", &state),
        Err(StateError::Unreadable(_))
    ));
}

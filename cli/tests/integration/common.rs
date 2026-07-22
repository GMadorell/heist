use std::sync::Mutex;
use tempfile::TempDir;

/// Parse-or-panic constructors for value objects in test fixtures, where the
/// input is a hardcoded literal known to be valid and a parse failure means
/// the test itself is broken.
///
/// Integration tests link the lib built without `cfg(test)`, so they can't
/// reach `heist_cli::domain::testing::valid` (the unit-test equivalent) and
/// need their own copy.
pub mod valid {
    use heist_cli::domain::value::{BranchValue, DateValue, RefValue, SlugValue};

    pub fn slug(s: &str) -> SlugValue {
        SlugValue::parse(s).expect("valid slug")
    }

    pub fn date(s: &str) -> DateValue {
        DateValue::parse("date", s).expect("valid date")
    }

    pub fn branch(s: &str) -> BranchValue {
        BranchValue::try_from_raw("branch", s).expect("valid branch")
    }

    pub fn ref_value(s: &str) -> RefValue {
        RefValue::try_from_raw(s).expect("valid ref")
    }
}

// The process cwd is a single global resource, shared by every test in this
// binary. Any test file that chdirs must serialize through this one lock, or
// two files' TempCwd guards can race and step on each other's tempdir.
static CWD_LOCK: Mutex<()> = Mutex::new(());

/// Chdirs into a fresh temp dir for the guard's lifetime, restoring the
/// original cwd and releasing the lock on drop.
pub struct TempCwd {
    _lock: std::sync::MutexGuard<'static, ()>,
    _dir: TempDir,
    original: std::path::PathBuf,
}

impl TempCwd {
    pub fn new() -> Self {
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

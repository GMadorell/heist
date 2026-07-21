use std::sync::Mutex;
use tempfile::TempDir;

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

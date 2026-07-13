//! Persistence seam for slug state.
//!
//! Command handlers depend on the [`StateRepository`] trait rather than reaching
//! for [`State::load`]/[`State::save`]/[`state_file_path`] directly, so state
//! access can be swapped for an in-memory fake in unit tests.

use crate::state::{state_file_path, State, StateError};

/// Read/write access to a slug's persisted [`State`].
pub trait StateRepository {
    /// Whether a slug already has persisted state.
    ///
    /// Part of the repository contract for callers that only need presence, not
    /// the value; the CLI's current commands don't call it directly yet.
    #[allow(dead_code)]
    fn exists(&self, slug: &str) -> bool;

    /// Create a slug's state for the first time.
    ///
    /// Errors with [`StateError::AlreadyExists`] if the slug is already present,
    /// so re-initialising a slug is rejected rather than silently overwriting.
    fn init(&self, slug: &str, state: &State) -> Result<(), StateError>;

    /// Load a slug's state, or a [`StateError`] describing why it couldn't be read.
    fn load(&self, slug: &str) -> Result<State, StateError>;

    /// Persist a slug's state, overwriting any existing value.
    fn save(&self, slug: &str, state: &State) -> Result<(), StateError>;
}

/// The real, filesystem-backed repository: `.heist/<slug>/state.json`.
///
/// Behaves identically to the direct `State::load`/`save` call sites it replaced:
/// same errors, same exit-code mapping.
pub struct FileStateRepository;

impl StateRepository for FileStateRepository {
    fn exists(&self, slug: &str) -> bool {
        state_file_path(slug).exists()
    }

    fn init(&self, slug: &str, state: &State) -> Result<(), StateError> {
        let state_file = state_file_path(slug);
        let state_dir = state_file.parent().expect("state path has a parent");

        // Reject on directory existence (not file existence) so a pre-existing
        // but empty `.heist/<slug>/` still counts as "already initialised".
        if state_dir.exists() {
            return Err(StateError::AlreadyExists);
        }
        std::fs::create_dir_all(state_dir).map_err(StateError::Unreadable)?;
        state.save(&state_file)
    }

    fn load(&self, slug: &str) -> Result<State, StateError> {
        State::load(&state_file_path(slug))
    }

    fn save(&self, slug: &str, state: &State) -> Result<(), StateError> {
        state.save(&state_file_path(slug))
    }
}

/// In-memory repository for unit tests.
///
/// Faithfully reproduces the file version's error semantics: `load` on an
/// unknown slug yields [`StateError::Missing`], and `init` on a known slug
/// yields [`StateError::AlreadyExists`].
#[cfg(test)]
pub struct InMemoryStateRepository {
    states: std::cell::RefCell<std::collections::HashMap<String, State>>,
}

#[cfg(test)]
impl InMemoryStateRepository {
    pub fn new() -> Self {
        InMemoryStateRepository {
            states: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    /// Seed a slug's state up front (bypassing `init`), for arranging test scenarios.
    pub fn with_state(self, slug: &str, state: State) -> Self {
        self.states.borrow_mut().insert(slug.to_string(), state);
        self
    }

    /// Snapshot a slug's current state, or `None` if absent.
    pub fn get(&self, slug: &str) -> Option<State> {
        self.states.borrow().get(slug).cloned()
    }
}

#[cfg(test)]
impl StateRepository for InMemoryStateRepository {
    fn exists(&self, slug: &str) -> bool {
        self.states.borrow().contains_key(slug)
    }

    fn init(&self, slug: &str, state: &State) -> Result<(), StateError> {
        let mut states = self.states.borrow_mut();
        if states.contains_key(slug) {
            return Err(StateError::AlreadyExists);
        }
        states.insert(slug.to_string(), state.clone());
        Ok(())
    }

    fn load(&self, slug: &str) -> Result<State, StateError> {
        self.states
            .borrow()
            .get(slug)
            .cloned()
            .ok_or(StateError::Missing)
    }

    fn save(&self, slug: &str, state: &State) -> Result<(), StateError> {
        self.states
            .borrow_mut()
            .insert(slug.to_string(), state.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_rejects_existing_slug() {
        let repo = InMemoryStateRepository::new();
        assert!(repo.init("foo", &State::new("foo")).is_ok());
        assert!(matches!(
            repo.init("foo", &State::new("foo")),
            Err(StateError::AlreadyExists)
        ));
    }

    #[test]
    fn exists_tracks_presence() {
        let repo = InMemoryStateRepository::new();
        assert!(!repo.exists("foo"));
        repo.init("foo", &State::new("foo"))
            .expect("init should succeed");
        assert!(repo.exists("foo"));
    }

    #[test]
    fn load_missing_slug_is_missing() {
        let repo = InMemoryStateRepository::new();
        assert!(matches!(repo.load("nope"), Err(StateError::Missing)));
    }

    #[test]
    fn save_then_load_roundtrips() {
        let repo = InMemoryStateRepository::new();
        let mut state = State::new("foo");
        state.score_step = 3;
        repo.save("foo", &state).expect("save should succeed");
        assert_eq!(repo.load("foo").expect("load should succeed").score_step, 3);
    }
}

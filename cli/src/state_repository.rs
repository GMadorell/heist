use crate::domain::error::StateError;
use crate::domain::state::State;
use std::path::{Path, PathBuf};

pub trait StateRepository {
    fn exists(&self, slug: &str) -> bool;

    fn init(&self, slug: &str, state: &State) -> Result<(), StateError>;

    fn load(&self, slug: &str) -> Result<State, StateError>;

    fn save(&self, slug: &str, state: &State) -> Result<(), StateError>;
}

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
        save_state_file(state, &state_file)
    }

    fn load(&self, slug: &str) -> Result<State, StateError> {
        load_state_file(&state_file_path(slug))
    }

    fn save(&self, slug: &str, state: &State) -> Result<(), StateError> {
        save_state_file(state, &state_file_path(slug))
    }
}

fn state_file_path(slug: &str) -> PathBuf {
    Path::new(".heist").join(slug).join("state.json")
}

fn load_state_file(path: &Path) -> Result<State, StateError> {
    if !path.exists() {
        return Err(StateError::Missing);
    }
    let content = std::fs::read_to_string(path).map_err(StateError::Unreadable)?;
    let state: State = serde_json::from_str(&content).map_err(StateError::Unparseable)?;
    Ok(state)
}

fn save_state_file(state: &State, path: &Path) -> Result<(), StateError> {
    let json = serde_json::to_string_pretty(state).map_err(StateError::Unparseable)?;
    std::fs::write(path, json).map_err(StateError::Unreadable)
}

pub struct InMemoryStateRepository {
    states: std::cell::RefCell<std::collections::HashMap<String, State>>,
}

impl InMemoryStateRepository {
    pub fn new() -> Self {
        InMemoryStateRepository {
            states: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    pub fn with_state(self, slug: &str, state: State) -> Self {
        self.states.borrow_mut().insert(slug.to_string(), state);
        self
    }

    pub fn get(&self, slug: &str) -> Option<State> {
        self.states.borrow().get(slug).cloned()
    }
}

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

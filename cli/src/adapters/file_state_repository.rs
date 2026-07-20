use crate::adapters::file_heist_dir_repository::heist_dir_path;
use crate::domain::error::StateError;
use crate::domain::state::State;
use crate::domain::value::SlugValue;
use crate::ports::state_repository::StateRepository;
use std::path::{Path, PathBuf};

pub struct FileStateRepository;

impl StateRepository for FileStateRepository {
    fn exists(&self, slug: &str) -> bool {
        state_file_path(slug).exists()
    }

    fn init(&self, slug: &str, state: &State) -> Result<(), StateError> {
        save_state_file(state, &state_file_path(slug))
    }

    fn load(&self, slug: &str) -> Result<State, StateError> {
        load_state_file(&state_file_path(slug))
    }

    fn save(&self, slug: &str, state: &State) -> Result<(), StateError> {
        save_state_file(state, &state_file_path(slug))
    }

    fn list_slugs(&self) -> Result<Vec<SlugValue>, StateError> {
        let heist_dir = Path::new(".heist");
        if !heist_dir.exists() {
            return Ok(Vec::new());
        }

        let mut slugs = Vec::new();
        for entry in std::fs::read_dir(heist_dir).map_err(StateError::Unreadable)? {
            let entry = entry.map_err(StateError::Unreadable)?;
            if !entry.path().is_dir() {
                continue;
            }
            let dir_name = entry.file_name().to_string_lossy().into_owned();
            if !state_file_path(&dir_name).exists() {
                continue;
            }
            // A directory name that isn't a valid slug can't have been created
            // by `state init`, so it isn't a heist: skip it rather than error.
            if let Ok(slug) = SlugValue::parse(&dir_name) {
                slugs.push(slug);
            }
        }
        slugs.sort_by(|a, b| a.as_ref().cmp(b.as_ref()));
        Ok(slugs)
    }
}

fn state_file_path(slug: &str) -> PathBuf {
    heist_dir_path(slug).join("state.json")
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

use crate::domain::error::StateError;
use crate::domain::state::State;
use crate::domain::value::{DateValue, SlugValue};
use crate::ports::clock::Clock;
use crate::ports::git::{GitError, GitRepository, WorktreeInfo};
use crate::ports::state_repository::StateRepository;
use crate::ports::validation_source::ValidationSource;
use crate::ports::worktree_fs::WorktreeFs;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::error::Error;
use std::path::{Path, PathBuf};

pub struct FixedClock(pub DateValue);

impl Clock for FixedClock {
    fn today(&self) -> DateValue {
        self.0.clone()
    }
}

/// In-memory worktree fs for unit tests: echoes paths back with no real fs.
pub struct FakeWorktreeFs;

impl WorktreeFs for FakeWorktreeFs {
    fn ensure_worktrees_ignored(&self, _repo_root: &Path) -> std::io::Result<bool> {
        Ok(false)
    }

    fn link_heist_dir(
        &self,
        _repo_root: &Path,
        _worktree_path: &Path,
        _slug: &str,
    ) -> std::io::Result<()> {
        Ok(())
    }

    fn canonicalize(&self, path: &Path) -> std::io::Result<PathBuf> {
        Ok(path.to_path_buf())
    }
}

pub struct InMemoryStateRepository {
    states: std::cell::RefCell<std::collections::HashMap<String, State>>,
}

impl Default for InMemoryStateRepository {
    fn default() -> Self {
        Self::new()
    }
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

    fn list_slugs(&self) -> Result<Vec<SlugValue>, StateError> {
        let mut slugs: Vec<SlugValue> = self
            .states
            .borrow()
            .keys()
            .map(|k| SlugValue::parse(k).expect("test slug should be valid"))
            .collect();
        slugs.sort_by(|a, b| a.as_ref().cmp(b.as_ref()));
        Ok(slugs)
    }
}

/// In-memory git for unit tests
pub struct FakeGit {
    default_branch: String,
    merged_branches: std::collections::HashSet<String>,
    worktrees: std::cell::RefCell<std::collections::HashSet<String>>,
    worktree_infos: Vec<WorktreeInfo>,
    add_error: Option<GitError>,
    remove_error: Option<GitError>,
    delete_error: Option<GitError>,
    merge_check_error: Option<GitError>,
    removed_worktree_paths: RefCell<Vec<PathBuf>>,
    deleted_branch_names: RefCell<Vec<String>>,
}

impl Default for FakeGit {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeGit {
    pub fn new() -> Self {
        FakeGit {
            default_branch: "main".to_string(),
            merged_branches: std::collections::HashSet::new(),
            worktrees: std::cell::RefCell::new(std::collections::HashSet::new()),
            worktree_infos: Vec::new(),
            add_error: None,
            remove_error: None,
            delete_error: None,
            merge_check_error: None,
            removed_worktree_paths: RefCell::new(Vec::new()),
            deleted_branch_names: RefCell::new(Vec::new()),
        }
    }

    pub fn with_default_branch(mut self, branch: &str) -> Self {
        self.default_branch = branch.to_string();
        self
    }

    pub fn with_merged_branch(mut self, branch: &str) -> Self {
        self.merged_branches.insert(branch.to_string());
        self
    }

    pub fn with_existing_worktree(self, slug: &str) -> Self {
        self.worktrees.borrow_mut().insert(slug.to_string());
        self
    }

    pub fn with_worktree_info(mut self, name: &str, path: &str, branch: Option<&str>) -> Self {
        self.worktree_infos.push(WorktreeInfo {
            name: name.to_string(),
            path: std::path::PathBuf::from(path),
            branch: branch.map(str::to_string),
        });
        self
    }

    pub fn failing_add(mut self, error: GitError) -> Self {
        self.add_error = Some(error);
        self
    }

    pub fn failing_remove(mut self, error: GitError) -> Self {
        self.remove_error = Some(error);
        self
    }

    pub fn failing_delete(mut self, error: GitError) -> Self {
        self.delete_error = Some(error);
        self
    }

    pub fn failing_merge_check(mut self, error: GitError) -> Self {
        self.merge_check_error = Some(error);
        self
    }

    pub fn removed_worktree_paths(&self) -> Vec<PathBuf> {
        self.removed_worktree_paths.borrow().clone()
    }

    pub fn deleted_branch_names(&self) -> Vec<String> {
        self.deleted_branch_names.borrow().clone()
    }
}

impl GitRepository for FakeGit {
    fn default_branch(&self, _repo_root: &Path) -> String {
        self.default_branch.clone()
    }

    fn is_branch_merged(
        &self,
        _repo_root: &Path,
        branch: &str,
        _into: &str,
    ) -> Result<bool, GitError> {
        if let Some(err) = &self.merge_check_error {
            return Err(err.clone());
        }
        Ok(self.merged_branches.contains(branch))
    }

    fn worktree_exists(&self, _repo_root: &Path, slug: &str) -> bool {
        self.worktrees.borrow().contains(slug)
    }

    fn add_worktree(
        &self,
        _repo_root: &Path,
        _path: &Path,
        branch: &str,
        _start_point: &str,
    ) -> Result<(), GitError> {
        if let Some(err) = &self.add_error {
            return Err(err.clone());
        }
        // Register by the branch's slug suffix (`heist/<slug>` -> `<slug>`).
        let slug = branch.rsplit('/').next().unwrap_or(branch);
        self.worktrees.borrow_mut().insert(slug.to_string());
        Ok(())
    }

    fn remove_worktree(&self, _repo_root: &Path, path: &Path) -> Result<(), GitError> {
        self.removed_worktree_paths.borrow_mut().push(path.to_path_buf());
        if let Some(err) = &self.remove_error {
            return Err(err.clone());
        }
        Ok(())
    }

    fn delete_branch(&self, _repo_root: &Path, branch: &str) -> Result<(), GitError> {
        self.deleted_branch_names.borrow_mut().push(branch.to_string());
        if let Some(err) = &self.delete_error {
            return Err(err.clone());
        }
        Ok(())
    }

    fn list_worktrees(&self, _repo_root: &Path) -> Result<Vec<WorktreeInfo>, GitError> {
        Ok(self.worktree_infos.clone())
    }
}

/// In-memory validation source for domain validation tests: a fixed repo
/// root plus a map of directory -> validation.md contents.
pub struct InMemoryValidationSource {
    repo_root: PathBuf,
    files: BTreeMap<PathBuf, String>,
}

impl InMemoryValidationSource {
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        InMemoryValidationSource {
            repo_root: repo_root.into(),
            files: BTreeMap::new(),
        }
    }

    pub fn with_validation(mut self, dir: impl Into<PathBuf>, contents: impl Into<String>) -> Self {
        self.files.insert(dir.into(), contents.into());
        self
    }
}

impl ValidationSource for InMemoryValidationSource {
    fn repo_root(&self) -> Result<PathBuf, Box<dyn Error>> {
        Ok(self.repo_root.clone())
    }

    fn read_validation(&self, dir: &Path) -> Result<Option<String>, Box<dyn Error>> {
        Ok(self.files.get(dir).cloned())
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn canonicalize(&self, path: &Path) -> Result<PathBuf, Box<dyn Error>> {
        Ok(path.canonicalize()?)
    }
}

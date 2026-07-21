use crate::domain::error::StateError;
use crate::domain::state::State;
use crate::domain::tool::Tool;
use crate::domain::value::{BranchValue, DateValue, RefValue, SlugValue};
use crate::ports::clock::Clock;
use crate::ports::git::{GitError, GitRepository, MergeCheck, PrState, WorktreeInfo};
use crate::ports::heist_dir_repository::HeistDirRepository;
use crate::ports::score_repository::ScoreRepository;
use crate::ports::state_repository::StateRepository;
use crate::ports::tool_probe::ToolProbe;
use crate::ports::validation_source::ValidationSource;
use crate::ports::worktree_fs::WorktreeFs;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
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

/// In-memory tool probe for unit tests: reports a tool available only if
/// explicitly registered via `with_available`.
pub struct FakeToolProbe {
    available: HashSet<Tool>,
}

impl Default for FakeToolProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeToolProbe {
    pub fn new() -> Self {
        FakeToolProbe {
            available: HashSet::new(),
        }
    }

    pub fn with_available(mut self, tool: Tool) -> Self {
        self.available.insert(tool);
        self
    }
}

impl ToolProbe for FakeToolProbe {
    fn is_available(&self, tool: Tool) -> bool {
        self.available.contains(&tool)
    }
}

pub struct InMemoryStateRepository {
    states: std::cell::RefCell<std::collections::HashMap<String, State>>,
    /// Slug -> error to return from `load`
    load_errors: std::cell::RefCell<std::collections::HashMap<String, StateError>>,
    scores: std::cell::RefCell<std::collections::HashMap<String, String>>,
    score_errors: std::cell::RefCell<std::collections::HashMap<String, String>>,
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
            load_errors: std::cell::RefCell::new(std::collections::HashMap::new()),
            scores: std::cell::RefCell::new(std::collections::HashMap::new()),
            score_errors: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    pub fn with_state(self, slug: &str, state: State) -> Self {
        self.states.borrow_mut().insert(slug.to_string(), state);
        self
    }

    /// Makes `exists(slug)` true but `load(slug)` fail with `error`
    pub fn with_load_error(self, slug: &str, error: StateError) -> Self {
        self.load_errors
            .borrow_mut()
            .insert(slug.to_string(), error);
        self
    }

    pub fn with_score(self, slug: &str, content: &str) -> Self {
        self.scores
            .borrow_mut()
            .insert(slug.to_string(), content.to_string());
        self
    }

    pub fn with_score_io_error(self, slug: &str, message: &str) -> Self {
        self.score_errors
            .borrow_mut()
            .insert(slug.to_string(), message.to_string());
        self
    }

    pub fn get(&self, slug: &str) -> Option<State> {
        self.states.borrow().get(slug).cloned()
    }
}

impl StateRepository for InMemoryStateRepository {
    fn exists(&self, slug: &SlugValue) -> bool {
        let key = slug.as_ref();
        self.states.borrow().contains_key(key) || self.load_errors.borrow().contains_key(key)
    }

    fn load(&self, slug: &SlugValue) -> Result<State, StateError> {
        let key = slug.as_ref();
        if let Some(error) = self.load_errors.borrow_mut().remove(key) {
            return Err(error);
        }
        self.states
            .borrow()
            .get(key)
            .cloned()
            .ok_or(StateError::Missing)
    }

    fn save(&self, slug: &SlugValue, state: &State) -> Result<(), StateError> {
        self.states
            .borrow_mut()
            .insert(slug.as_ref().to_string(), state.clone());
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

impl ScoreRepository for InMemoryStateRepository {
    fn load_score(&self, slug: &SlugValue) -> Result<Option<String>, std::io::Error> {
        let key = slug.as_ref();
        if let Some(message) = self.score_errors.borrow().get(key) {
            return Err(std::io::Error::other(message.clone()));
        }
        Ok(self.scores.borrow().get(key).cloned())
    }
}

pub struct InMemoryHeistDirRepository {
    dirs: std::cell::RefCell<std::collections::HashSet<String>>,
}

impl Default for InMemoryHeistDirRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryHeistDirRepository {
    pub fn new() -> Self {
        InMemoryHeistDirRepository {
            dirs: std::cell::RefCell::new(std::collections::HashSet::new()),
        }
    }

    pub fn with_dir(self, slug: &str) -> Self {
        self.dirs.borrow_mut().insert(slug.to_string());
        self
    }

    pub fn exists(&self, slug: &str) -> bool {
        self.dirs.borrow().contains(slug)
    }
}

impl HeistDirRepository for InMemoryHeistDirRepository {
    fn create(&self, slug: &SlugValue) -> Result<(), StateError> {
        let key = slug.as_ref();
        let mut dirs = self.dirs.borrow_mut();
        if dirs.contains(key) {
            return Err(StateError::AlreadyExists);
        }
        dirs.insert(key.to_string());
        Ok(())
    }

    fn remove(&self, slug: &SlugValue) -> Result<(), StateError> {
        self.dirs.borrow_mut().remove(slug.as_ref());
        Ok(())
    }
}

/// In-memory git for unit tests
pub struct FakeGit {
    default_branch: String,
    merged_branches: std::collections::HashSet<String>,
    worktrees: std::cell::RefCell<std::collections::HashSet<String>>,
    branches: RefCell<HashSet<String>>,
    worktree_infos: Vec<WorktreeInfo>,
    add_error: Option<GitError>,
    remove_error: Option<GitError>,
    delete_error: Option<GitError>,
    merge_check_error_for: Option<(String, GitError)>,
    verification_error_for: Option<(String, String)>,
    remote_default_resolve_error: Option<GitError>,
    resolve_ref_error_for: Option<(String, GitError)>,
    removed_worktree_paths: RefCell<Vec<PathBuf>>,
    deleted_branch_names: RefCell<Vec<String>>,
    add_worktree_start_points: RefCell<Vec<String>>,
    changed_paths: Vec<PathBuf>,
    changed_paths_error: Option<GitError>,
    file_contents: std::collections::HashMap<PathBuf, String>,
    ancestors: HashSet<(String, String)>,
    pr_states: HashMap<String, PrState>,
    pr_state_error_for: Option<(String, GitError)>,
    is_ancestor_calls: RefCell<u32>,
    rebase_calls: RefCell<Vec<String>>,
    merge_calls: RefCell<Vec<String>>,
    failing_rebase: Option<GitError>,
    failing_merge: Option<GitError>,
    current_branch: Option<String>,
    fetch_calls: RefCell<Vec<String>>,
    fetch_error: Option<GitError>,
    /// Ordered log of mutating/fetch operations (`fetch`, `rebase`, `merge`)
    /// so tests can assert a fetch happened before any rebase/merge.
    call_log: RefCell<Vec<String>>,
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
            branches: RefCell::new(HashSet::new()),
            worktree_infos: Vec::new(),
            add_error: None,
            remove_error: None,
            delete_error: None,
            merge_check_error_for: None,
            verification_error_for: None,
            remote_default_resolve_error: None,
            resolve_ref_error_for: None,
            removed_worktree_paths: RefCell::new(Vec::new()),
            deleted_branch_names: RefCell::new(Vec::new()),
            add_worktree_start_points: RefCell::new(Vec::new()),
            changed_paths: Vec::new(),
            changed_paths_error: None,
            file_contents: std::collections::HashMap::new(),
            ancestors: HashSet::new(),
            pr_states: HashMap::new(),
            pr_state_error_for: None,
            is_ancestor_calls: RefCell::new(0),
            rebase_calls: RefCell::new(Vec::new()),
            merge_calls: RefCell::new(Vec::new()),
            failing_rebase: None,
            failing_merge: None,
            current_branch: None,
            fetch_calls: RefCell::new(Vec::new()),
            fetch_error: None,
            call_log: RefCell::new(Vec::new()),
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

    pub fn with_branch(self, branch: &str) -> Self {
        self.branches.borrow_mut().insert(branch.to_string());
        self
    }

    pub fn with_existing_worktree(self, slug: &str) -> Self {
        self.worktrees.borrow_mut().insert(slug.to_string());
        self
    }

    pub fn with_worktree_info(mut self, path: &str, branch: Option<&str>) -> Self {
        self.worktree_infos.push(WorktreeInfo {
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

    /// Fails the merge check only for the given branch, leaving the
    /// top-level `remote_default_resolves` probe and every other branch's
    /// check unaffected.
    pub fn failing_merge_check_for(mut self, branch: &str, error: GitError) -> Self {
        self.merge_check_error_for = Some((branch.to_string(), error));
        self
    }

    pub fn failing_verification_for(mut self, branch: &str, message: &str) -> Self {
        self.verification_error_for = Some((branch.to_string(), message.to_string()));
        self
    }

    /// Fails the top-level `origin/<default>` resolvability probe that
    /// `cleanup` runs before sweeping worktrees.
    pub fn failing_remote_default_resolve(mut self, error: GitError) -> Self {
        self.remote_default_resolve_error = Some(error);
        self
    }

    pub fn failing_resolve_ref_for(mut self, ref_spec: &str, error: GitError) -> Self {
        self.resolve_ref_error_for = Some((ref_spec.to_string(), error));
        self
    }

    pub fn with_changed_paths(mut self, paths: &[&str]) -> Self {
        self.changed_paths = paths.iter().map(PathBuf::from).collect();
        self
    }

    pub fn failing_changed_paths(mut self, error: GitError) -> Self {
        self.changed_paths_error = Some(error);
        self
    }

    pub fn with_file_content(mut self, path: &str, content: &str) -> Self {
        self.file_contents
            .insert(PathBuf::from(path), content.to_string());
        self
    }

    pub fn with_ancestor(mut self, ancestor: &str, descendant: &str) -> Self {
        self.ancestors
            .insert((ancestor.to_string(), descendant.to_string()));
        self
    }

    pub fn with_pr_state(mut self, branch: &str, state: PrState) -> Self {
        self.pr_states.insert(branch.to_string(), state);
        self
    }

    pub fn failing_pr_state_for(mut self, branch: &str, error: GitError) -> Self {
        self.pr_state_error_for = Some((branch.to_string(), error));
        self
    }

    pub fn is_ancestor_call_count(&self) -> u32 {
        *self.is_ancestor_calls.borrow()
    }

    pub fn removed_worktree_paths(&self) -> Vec<PathBuf> {
        self.removed_worktree_paths.borrow().clone()
    }

    pub fn deleted_branch_names(&self) -> Vec<String> {
        self.deleted_branch_names.borrow().clone()
    }

    pub fn add_worktree_start_points(&self) -> Vec<String> {
        self.add_worktree_start_points.borrow().clone()
    }

    pub fn rebase_calls(&self) -> Vec<String> {
        self.rebase_calls.borrow().clone()
    }

    pub fn merge_calls(&self) -> Vec<String> {
        self.merge_calls.borrow().clone()
    }

    pub fn failing_rebase(mut self, error: GitError) -> Self {
        self.failing_rebase = Some(error);
        self
    }

    pub fn failing_merge(mut self, error: GitError) -> Self {
        self.failing_merge = Some(error);
        self
    }

    pub fn with_current_branch(mut self, branch: &str) -> Self {
        self.current_branch = Some(branch.to_string());
        self
    }

    pub fn failing_fetch(mut self, error: GitError) -> Self {
        self.fetch_error = Some(error);
        self
    }

    pub fn fetch_calls(&self) -> Vec<String> {
        self.fetch_calls.borrow().clone()
    }

    pub fn call_log(&self) -> Vec<String> {
        self.call_log.borrow().clone()
    }
}

impl GitRepository for FakeGit {
    fn default_branch(&self, _repo_root: &Path) -> String {
        self.default_branch.clone()
    }

    fn branch_exists(&self, _repo_root: &Path, branch: &BranchValue) -> Result<bool, GitError> {
        Ok(self.branches.borrow().contains(branch.as_ref()))
    }

    fn current_branch(&self, _repo_root: &Path) -> Result<Option<String>, GitError> {
        Ok(self.current_branch.clone())
    }

    fn fetch(&self, _repo_root: &Path, remote: &str) -> Result<(), GitError> {
        self.fetch_calls.borrow_mut().push(remote.to_string());
        self.call_log.borrow_mut().push("fetch".to_string());
        if let Some(err) = &self.fetch_error {
            return Err(err.clone());
        }
        Ok(())
    }

    fn is_branch_merged(
        &self,
        _repo_root: &Path,
        branch: &BranchValue,
        _into: &str,
    ) -> Result<MergeCheck, GitError> {
        let branch_str = branch.as_ref();
        if let Some((failing_branch, err)) = &self.merge_check_error_for {
            if failing_branch == branch_str {
                return Err(err.clone());
            }
        }
        if self.merged_branches.contains(branch_str) {
            return Ok(MergeCheck::Merged);
        }
        if let Some((failing_branch, message)) = &self.verification_error_for {
            if failing_branch == branch_str {
                return Ok(MergeCheck::NotMerged {
                    verification_error: Some(message.clone()),
                });
            }
        }
        Ok(MergeCheck::NotMerged {
            verification_error: None,
        })
    }

    fn worktree_exists(&self, _repo_root: &Path, slug: &SlugValue) -> Result<bool, GitError> {
        Ok(self.worktrees.borrow().contains(slug.as_ref()))
    }

    fn add_worktree(
        &self,
        _repo_root: &Path,
        _path: &Path,
        branch: &BranchValue,
        start_point: &RefValue,
    ) -> Result<(), GitError> {
        if let Some(err) = &self.add_error {
            return Err(err.clone());
        }
        self.add_worktree_start_points
            .borrow_mut()
            .push(start_point.as_ref().to_string());
        // Register by the branch's slug suffix (`heist/<slug>` -> `<slug>`).
        let branch_str = branch.as_ref();
        let slug = branch_str.rsplit('/').next().unwrap_or(branch_str);
        self.worktrees.borrow_mut().insert(slug.to_string());
        self.branches.borrow_mut().insert(branch_str.to_string());
        Ok(())
    }

    fn remove_worktree(&self, _repo_root: &Path, path: &Path) -> Result<(), GitError> {
        self.removed_worktree_paths
            .borrow_mut()
            .push(path.to_path_buf());
        if let Some(err) = &self.remove_error {
            return Err(err.clone());
        }
        Ok(())
    }

    fn delete_branch(&self, _repo_root: &Path, branch: &BranchValue) -> Result<(), GitError> {
        self.deleted_branch_names
            .borrow_mut()
            .push(branch.as_ref().to_string());
        if let Some(err) = &self.delete_error {
            return Err(err.clone());
        }
        Ok(())
    }

    fn list_worktrees(&self, _repo_root: &Path) -> Result<Vec<WorktreeInfo>, GitError> {
        Ok(self.worktree_infos.clone())
    }

    fn remote_default_resolves(
        &self,
        _repo_root: &Path,
        _main_branch: &str,
    ) -> Result<(), GitError> {
        if let Some(err) = &self.remote_default_resolve_error {
            return Err(err.clone());
        }
        Ok(())
    }

    fn resolve_ref(&self, _repo_root: &Path, ref_spec: &RefValue) -> Result<(), GitError> {
        if let Some((ref_name, err)) = &self.resolve_ref_error_for {
            if ref_name == ref_spec.as_ref() {
                return Err(err.clone());
            }
        }
        Ok(())
    }

    fn changed_paths(
        &self,
        _repo_root: &Path,
        _base_branch: &str,
        _head_ref: &RefValue,
    ) -> Result<Vec<PathBuf>, GitError> {
        if let Some(err) = &self.changed_paths_error {
            return Err(err.clone());
        }
        Ok(self.changed_paths.clone())
    }

    fn read_file_at(
        &self,
        _repo_root: &Path,
        _rev: &RefValue,
        path: &Path,
    ) -> Result<Option<String>, GitError> {
        Ok(self.file_contents.get(path).cloned())
    }

    fn is_ancestor(
        &self,
        _repo_root: &Path,
        ancestor_ref: &RefValue,
        descendant_ref: &RefValue,
    ) -> Result<bool, GitError> {
        *self.is_ancestor_calls.borrow_mut() += 1;
        if ancestor_ref.as_ref() == descendant_ref.as_ref() {
            return Ok(true);
        }
        Ok(self.ancestors.contains(&(
            ancestor_ref.as_ref().to_string(),
            descendant_ref.as_ref().to_string(),
        )))
    }

    fn pr_state(&self, _repo_root: &Path, base_ref: &RefValue) -> Result<PrState, GitError> {
        if let Some((failing_branch, err)) = &self.pr_state_error_for {
            if failing_branch == base_ref.as_ref() {
                return Err(err.clone());
            }
        }
        Ok(self
            .pr_states
            .get(base_ref.as_ref())
            .cloned()
            .unwrap_or(PrState::None))
    }

    fn rebase(&self, _repo_root: &Path, onto: &RefValue) -> Result<(), GitError> {
        self.rebase_calls.borrow_mut().push(onto.to_string());
        self.call_log.borrow_mut().push("rebase".to_string());
        if let Some(err) = &self.failing_rebase {
            return Err(err.clone());
        }
        Ok(())
    }

    fn merge(&self, _repo_root: &Path, other_ref: &RefValue) -> Result<(), GitError> {
        self.merge_calls.borrow_mut().push(other_ref.to_string());
        self.call_log.borrow_mut().push("merge".to_string());
        if let Some(err) = &self.failing_merge {
            return Err(err.clone());
        }
        Ok(())
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

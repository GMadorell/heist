use crate::domain::validation::{self, ValidationError};
use crate::ports::validation_source::ValidationSource;
use std::path::{Path, PathBuf};

pub fn resolve(src: &dyn ValidationSource, paths: &[PathBuf]) -> Result<String, ValidationError> {
    if paths.len() == 1 {
        validation::resolve_validation(src, &paths[0])
    } else {
        validation::resolve_validations(src, paths)
    }
}

pub fn check(src: &dyn ValidationSource, path: &Path) -> Result<bool, ValidationError> {
    validation::check_validation_exists(src, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::testing::InMemoryValidationSource;

    #[test]
    fn resolve_threads_cwd_distinct_from_repo_root_through_to_validation_dirs() {
        // Regression test for the cwd-vs-repo_root wiring: a relative path
        // must resolve against `src.cwd()`, not `src.repo_root()`. Uses real
        // tempdirs because `validation_dirs` canonicalizes on disk.
        let temp_dir = tempfile::TempDir::new().expect("should create temp dir");
        let repo_root = temp_dir.path().join("repo");
        let cwd_dir = repo_root.join("a");
        let sibling_dir = repo_root.join("b");
        std::fs::create_dir_all(&cwd_dir).expect("should create cwd dir");
        std::fs::create_dir_all(&sibling_dir).expect("should create sibling dir");

        let repo_root = repo_root.canonicalize().expect("repo_root canonicalizes");
        let cwd_dir = cwd_dir.canonicalize().expect("cwd_dir canonicalizes");
        let sibling_dir = sibling_dir
            .canonicalize()
            .expect("sibling_dir canonicalizes");

        let src = InMemoryValidationSource::new(repo_root)
            .with_cwd(cwd_dir)
            .with_validation(
                sibling_dir,
                "# Validation\n\n## Build\nnone\n\n## Lint\nnone\n\n## Test\nnone\n\n## Docs\nnone\n\n## PR conventions\nnone\n\n## Notes\nnone",
            );

        // From cwd (`a/`), `../b/file.md` targets `b/`, which only has a
        // validation.md if resolution actually used cwd instead of repo_root.
        let result = resolve(&src, &[PathBuf::from("../b/file.md")]);
        assert!(
            result.is_ok(),
            "should resolve relative to cwd, not repo_root: {:?}",
            result.err()
        );
        assert!(result.unwrap().contains("## Build"));
    }
}

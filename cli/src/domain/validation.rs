use crate::ports::validation_source::ValidationSource;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum ValidationError {
    PathNotAbsolute {
        path: PathBuf,
    },
    PathOutsideProject {
        requested: PathBuf,
        project_root: PathBuf,
    },
    Other(Box<dyn Error>),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationError::PathNotAbsolute { path } => {
                write!(f, "path {} must be absolute", path.display())
            }
            ValidationError::PathOutsideProject {
                requested,
                project_root,
            } => write!(
                f,
                "path {} is outside the project root {}",
                requested.display(),
                project_root.display()
            ),
            ValidationError::Other(e) => write!(f, "{}", e),
        }
    }
}

impl Error for ValidationError {}

impl From<Box<dyn Error>> for ValidationError {
    fn from(e: Box<dyn Error>) -> Self {
        ValidationError::Other(e)
    }
}

impl From<ParseError> for ValidationError {
    fn from(e: ParseError) -> Self {
        ValidationError::Other(Box::new(e))
    }
}

impl From<&str> for ValidationError {
    fn from(s: &str) -> Self {
        ValidationError::Other(s.into())
    }
}

/// Fixed output order for sections.
pub const SECTION_ORDER: [&str; 6] = ["Build", "Lint", "Test", "Docs", "PR conventions", "Notes"];

/// Resolve validation for a single path into its rendered section block.
///
/// Walks from the repo root down to the path's directory, merging every
/// `validation.md` found along the way, then renders sections in
/// `SECTION_ORDER`.
pub fn resolve_validation(
    src: &dyn ValidationSource,
    path: &Path,
) -> Result<String, ValidationError> {
    let repo_root = src.repo_root()?;
    let (merged, _scope) = resolve_validation_with_scope(src, path, &repo_root)?;

    let mut output = String::new();
    for section in SECTION_ORDER {
        if let Some(body) = merged.get(section) {
            output.push_str(&format!("## {}\n{}\n\n", section, body));
        }
    }
    if output.ends_with("\n\n") {
        output.pop();
    }
    Ok(output)
}

/// Resolve validation for multiple paths, grouped by scope.
///
/// Paths that resolve to the same deepest `validation.md` directory share one
/// labeled block; distinct scopes each get their own.
pub fn resolve_validations(
    src: &dyn ValidationSource,
    paths: &[PathBuf],
) -> Result<String, ValidationError> {
    let repo_root = src.repo_root()?;

    let mut scope_to_sections: BTreeMap<PathBuf, BTreeMap<String, String>> = BTreeMap::new();
    for path in paths {
        let (merged, scope) = resolve_validation_with_scope(src, path, &repo_root)?;
        scope_to_sections.insert(scope, merged);
    }

    let mut output = String::new();
    for (scope, merged) in scope_to_sections {
        let label = if scope.as_os_str().is_empty() || scope == Path::new(".") {
            ".".to_string()
        } else {
            scope.to_string_lossy().to_string()
        };
        output.push_str(&format!("### {}\n\n", label));

        for section in SECTION_ORDER {
            if let Some(body) = merged.get(section) {
                output.push_str(&format!("## {}\n{}\n\n", section, body));
            }
        }
    }

    while output.ends_with("\n\n\n") {
        output.pop();
    }
    if !output.ends_with('\n') {
        output.push('\n');
    }
    Ok(output)
}

/// Whether at least one `validation.md` exists along the path's ancestor chain.
pub fn check_validation_exists(
    src: &dyn ValidationSource,
    path: &Path,
) -> Result<bool, ValidationError> {
    let repo_root = src.repo_root()?;
    for dir in validation_dirs(path, &repo_root)? {
        if src.read_validation(&dir)?.is_some() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Parse a `validation.md` body into its sections.
///
/// Headings and bullet markers are normalized so cosmetic variance (casing,
/// spacing, `-` vs `*`) doesn't change the result. The required-sections rule
/// is enforced on the merged chain (`require_sections`), not per file, so an
/// ancestor may carry only repo-global sections.
pub fn parse_sections(text: &str) -> Result<BTreeMap<String, String>, ParseError> {
    let mut sections = BTreeMap::new();
    let lines: Vec<&str> = text.lines().collect();

    let canonical_names = ["Build", "Lint", "Test", "Docs", "PR conventions", "Notes"];

    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let line_number = i + 1;

        if line.starts_with("##") {
            if !line.starts_with("## ") {
                return Err(ParseError::new(format!(
                    "malformed heading on line {}: {}",
                    line_number, line
                )));
            }

            let raw_heading = line[3..].trim().to_string();
            let heading_lower = raw_heading.to_lowercase();
            let normalized_heading = canonical_names
                .iter()
                .find(|canonical| canonical.to_lowercase() == heading_lower)
                .map(|canonical| canonical.to_string())
                .unwrap_or(raw_heading);

            let mut body_lines = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].starts_with("## ") {
                let body_line = lines[i];
                let body_line_number = i + 1;

                if body_line.starts_with("##") && !body_line.starts_with("## ") {
                    return Err(ParseError::new(format!(
                        "malformed heading on line {}: {}",
                        body_line_number, body_line
                    )));
                }

                let normalized_line = if body_line.trim_start().starts_with('-')
                    || body_line.trim_start().starts_with('*')
                {
                    let trimmed = body_line.trim_start();
                    let normalized = if trimmed.starts_with('*') {
                        trimmed.replacen('*', "-", 1)
                    } else {
                        trimmed.to_string()
                    };
                    let leading_spaces = body_line.len() - body_line.trim_start().len();
                    format!("{}{}", " ".repeat(leading_spaces), normalized)
                } else {
                    body_line.to_string()
                };
                body_lines.push(normalized_line);
                i += 1;
            }

            let body = body_lines.join("\n").trim().to_string();
            sections.insert(normalized_heading, body);
        } else {
            i += 1;
        }
    }

    Ok(sections)
}

/// Enforce that the effective (merged) validation supplies `Build`/`Lint`/`Test`.
fn require_sections(sections: &BTreeMap<String, String>) -> Result<(), ParseError> {
    let missing: Vec<&str> = ["Build", "Lint", "Test"]
        .iter()
        .filter(|&&section| !sections.contains_key(section))
        .copied()
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(ParseError::new(format!(
            "missing required section(s): {}",
            missing.join(", ")
        )))
    }
}

/// Merge section layers, nearest layer winning per whole section.
///
/// A leaf's `## Build` replaces an ancestor's `## Build` entirely (no
/// line-level merge), so a leaf never inherits half of an overridden section.
pub fn merge(layers: &[BTreeMap<String, String>]) -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
    for layer in layers {
        for (key, value) in layer {
            result.insert(key.clone(), value.clone());
        }
    }
    result
}

/// Candidate `validation.md` directories from repo root down to the path's
/// directory.
fn validation_dirs(path: &Path, repo_root: &Path) -> Result<Vec<PathBuf>, ValidationError> {
    if !path.is_absolute() {
        return Err(ValidationError::PathNotAbsolute {
            path: path.to_path_buf(),
        });
    }

    let canonical_repo_root = repo_root
        .canonicalize()
        .map_err(|e| ValidationError::Other(Box::new(e)))?;

    let canonical_target = if path.exists() {
        path.canonicalize()
            .map_err(|e| ValidationError::Other(Box::new(e)))?
    } else {
        let parent = path.parent().ok_or_else(|| {
            ValidationError::Other(
                format!("path {} has no parent directory", path.display()).into(),
            )
        })?;
        let canonical_parent = parent
            .canonicalize()
            .map_err(|e| ValidationError::Other(Box::new(e)))?;
        match path.file_name() {
            Some(name) => canonical_parent.join(name),
            None => canonical_parent,
        }
    };

    let target_dir = canonical_target
        .parent()
        .unwrap_or(canonical_target.as_path());

    let rel = target_dir
        .strip_prefix(&canonical_repo_root)
        .map_err(|_| ValidationError::PathOutsideProject {
            requested: path.to_path_buf(),
            project_root: canonical_repo_root.clone(),
        })?;

    let mut dirs = Vec::new();
    let mut current = canonical_repo_root.clone();
    dirs.push(current.clone());
    for component in rel.components() {
        current.push(component);
        dirs.push(current.clone());
    }

    Ok(dirs)
}

/// Merge every `validation.md` along the path's chain, returning the merged
/// sections and the deepest scope directory (relative to repo root).
fn resolve_validation_with_scope(
    src: &dyn ValidationSource,
    path: &Path,
    repo_root: &Path,
) -> Result<(BTreeMap<String, String>, PathBuf), ValidationError> {
    let mut layers = Vec::new();
    let mut scope_dir = PathBuf::from(".");

    for dir in validation_dirs(path, repo_root)? {
        if let Some(text) = src.read_validation(&dir)? {
            layers.push(parse_sections(&text)?);
            if let Ok(rel) = dir.strip_prefix(repo_root) {
                scope_dir = rel.to_path_buf();
            }
        }
    }

    if layers.is_empty() {
        return Err("no validation.md files found".into());
    }
    let merged = merge(&layers);
    require_sections(&merged)?;
    Ok((merged, scope_dir))
}

#[derive(Debug)]
pub struct ParseError {
    message: String,
}

impl ParseError {
    pub fn new(message: impl Into<String>) -> Self {
        ParseError {
            message: message.into(),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for ParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_six_sections() {
        let fixture = r#"# Validation

## Build
None, this is a plugin.

## Lint
None, no linter configured.

## Test
No automated test suite.

## Docs
Keep README in sync.

## PR conventions
Main branch: main

## Notes
No CI configured."#;

        let result = parse_sections(fixture);
        assert!(result.is_ok(), "parse_sections should succeed");

        let sections = result.unwrap();

        assert_eq!(
            sections.len(),
            6,
            "should have exactly 6 sections, got: {:?}",
            sections.keys().collect::<Vec<_>>()
        );

        assert!(sections.contains_key("Build"), "should have Build section");
        assert!(sections.contains_key("Lint"), "should have Lint section");
        assert!(sections.contains_key("Test"), "should have Test section");
        assert!(sections.contains_key("Docs"), "should have Docs section");
        assert!(
            sections.contains_key("PR conventions"),
            "should have PR conventions section"
        );
        assert!(sections.contains_key("Notes"), "should have Notes section");

        assert_eq!(sections["Build"].trim(), "None, this is a plugin.");
        assert_eq!(sections["Lint"].trim(), "None, no linter configured.");
        assert_eq!(sections["Test"].trim(), "No automated test suite.");
        assert_eq!(sections["Docs"].trim(), "Keep README in sync.");
        assert_eq!(sections["PR conventions"].trim(), "Main branch: main");
        assert_eq!(sections["Notes"].trim(), "No CI configured.");
    }

    #[test]
    fn tolerates_case_whitespace_and_bullet_variance() {
        let canonical = r#"# Validation

## Build
None, this is a plugin.

## Lint
None, no linter configured.

## Test
No automated test suite.

## Docs
Keep README in sync.

## PR conventions
Main branch: main

## Notes
No CI configured."#;

        let mutated = r#"# Validation

##  build
None, this is a plugin.

##  lint
None, no linter configured.

##  test
No automated test suite.

##  docs
Keep README in sync.

##  pr conventions
Main branch: main

##  notes
No CI configured."#;

        let canonical_sections = parse_sections(canonical).expect("canonical should parse");
        let mutated_sections = parse_sections(mutated).expect("mutated should parse");

        assert_eq!(
            canonical_sections.keys().collect::<Vec<_>>(),
            mutated_sections.keys().collect::<Vec<_>>(),
            "keys should be identical after normalization"
        );

        for (key, canonical_body) in &canonical_sections {
            let mutated_body = mutated_sections
                .get(key)
                .unwrap_or_else(|| panic!("mutated should have key '{}'", key));
            assert_eq!(
                canonical_body.replace('*', "-"),
                mutated_body.replace('*', "-"),
                "body content should be identical for section '{}'",
                key
            );
        }
    }

    #[test]
    fn require_sections_rejects_merged_result_missing_required() {
        // An ancestor may lack Build/Lint/Test, but the merged chain must not.
        let mut merged = BTreeMap::new();
        merged.insert("Build".to_string(), "b".to_string());
        merged.insert("Lint".to_string(), "l".to_string());
        merged.insert("Notes".to_string(), "n".to_string());

        let result = require_sections(&merged);
        assert!(
            result.is_err(),
            "require_sections should fail when Test is missing from the merged result"
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .to_lowercase()
                .contains("test"),
            "error message should mention 'Test'"
        );
    }

    #[test]
    fn require_sections_accepts_partial_ancestor_when_leaf_supplies_required() {
        // Root layer carries only repo-global sections (no Build/Lint/Test).
        let mut root = BTreeMap::new();
        root.insert("PR conventions".to_string(), "main".to_string());
        root.insert("Notes".to_string(), "no ci".to_string());

        // Leaf layer supplies the required sections.
        let mut leaf = BTreeMap::new();
        leaf.insert("Build".to_string(), "cargo build".to_string());
        leaf.insert("Lint".to_string(), "cargo clippy".to_string());
        leaf.insert("Test".to_string(), "cargo test".to_string());

        let merged = merge(&[root, leaf]);
        assert!(
            require_sections(&merged).is_ok(),
            "merged chain supplying all required sections should be accepted"
        );
    }

    #[test]
    fn rejects_malformed_heading_with_line_number() {
        let fixture = r#"# Validation

## Build
None, this is a plugin.

## Lint
None, no linter configured.

###Test:"#;

        let result = parse_sections(fixture);
        assert!(
            result.is_err(),
            "parse_sections should reject malformed heading"
        );
        assert!(
            result.unwrap_err().to_string().contains("9"),
            "error message should include line number 9"
        );
    }

    #[test]
    fn leaf_replaces_whole_sections() {
        let root_fixture = r#"# Validation

## Build
None, this is a plugin.

## Lint
None, no linter configured.

## Test
No automated test suite.

## Docs
Keep README in sync.

## PR conventions
Main branch: main

## Notes
No CI configured."#;

        let root_map = parse_sections(root_fixture).expect("root fixture should parse");

        let mut leaf_map = BTreeMap::new();
        leaf_map.insert("Build".to_string(), "Custom build command.".to_string());
        leaf_map.insert("Lint".to_string(), "Custom linter config.".to_string());
        leaf_map.insert("Test".to_string(), "Custom test runner.".to_string());

        let result = merge(&[root_map, leaf_map]);

        assert_eq!(result["Build"], "Custom build command.");
        assert_eq!(result["Lint"], "Custom linter config.");
        assert_eq!(result["Test"], "Custom test runner.");
        assert_eq!(result["Docs"].trim(), "Keep README in sync.");
        assert_eq!(result["PR conventions"].trim(), "Main branch: main");
        assert_eq!(result["Notes"].trim(), "No CI configured.");
    }

    #[test]
    fn three_level_merge_applies_nearest_override_per_section() {
        let root_fixture = r#"# Validation

## Build
root build

## Lint
root lint

## Test
root test

## Docs
root docs

## PR conventions
root pr conventions

## Notes
root notes"#;

        let root_map = parse_sections(root_fixture).expect("root fixture should parse");

        let mut middle_map = BTreeMap::new();
        middle_map.insert("Test".to_string(), "middle test".to_string());

        let mut leaf_map = BTreeMap::new();
        leaf_map.insert("Build".to_string(), "leaf build".to_string());

        let result = merge(&[root_map, middle_map, leaf_map]);

        assert_eq!(result["Build"], "leaf build");
        assert_eq!(result["Test"].trim(), "middle test");
        assert_eq!(result["Lint"].trim(), "root lint");
        assert_eq!(result["Docs"].trim(), "root docs");
        assert_eq!(result["PR conventions"].trim(), "root pr conventions");
        assert_eq!(result["Notes"].trim(), "root notes");
    }

    #[test]
    fn validation_dirs_rejects_relative_path() {
        let result = validation_dirs(Path::new("relative/file.md"), Path::new("/tmp"));
        assert!(result.is_err(), "should reject a relative path");
        match result {
            Err(ValidationError::PathNotAbsolute { path }) => {
                assert_eq!(path, Path::new("relative/file.md"));
            }
            other => panic!("expected PathNotAbsolute error, got: {:?}", other),
        }
    }

    #[test]
    fn validation_dirs_rejects_path_outside_repo_root() {
        let outer_temp = tempfile::TempDir::new().expect("should create outer temp dir");
        let repo_root = outer_temp.path().join("repo_root").join("actual_repo");
        std::fs::create_dir_all(&repo_root).expect("should create repo_root");
        let outside_dir = outer_temp.path().join("outside");
        std::fs::create_dir_all(&outside_dir).expect("should create outside_dir");

        let repo_root_canonical = repo_root
            .canonicalize()
            .expect("repo_root should canonicalize");
        let outside_file = outside_dir.join("outside.rs");

        let result = validation_dirs(&outside_file, &repo_root_canonical);
        assert!(result.is_err(), "should reject path outside repo root");

        if let Err(ValidationError::PathOutsideProject {
            requested,
            project_root,
        }) = result
        {
            assert_eq!(requested, outside_file);
            assert_eq!(project_root, repo_root_canonical);
        } else {
            panic!("expected PathOutsideProject error, got: {:?}", result);
        }
    }

    #[test]
    fn validation_dirs_resolves_absolute_path_within_repo_root() {
        let temp_dir = tempfile::TempDir::new().expect("should create temp dir");
        let repo_root = temp_dir.path().join("repo");
        std::fs::create_dir_all(&repo_root).expect("should create repo");
        let subdir = repo_root.join("subdir");
        std::fs::create_dir_all(&subdir).expect("should create subdir");
        let file_path = subdir.join("test.md");
        std::fs::write(&file_path, "content").expect("should write test.md");

        let repo_root_canonical = repo_root
            .canonicalize()
            .expect("repo_root should canonicalize");

        let result = validation_dirs(&file_path, &repo_root_canonical);
        assert!(
            result.is_ok(),
            "should resolve absolute path within repo root: {:?}",
            result.err()
        );

        let dirs = result.unwrap();
        assert!(!dirs.is_empty(), "should return at least repo_root");
        assert_eq!(dirs[0], repo_root_canonical);
    }

    #[test]
    fn validation_dirs_canonicalizes_not_yet_existing_target_via_parent() {
        let temp_dir = tempfile::TempDir::new().expect("should create temp dir");
        let repo_root = temp_dir.path().join("repo");
        std::fs::create_dir_all(&repo_root).expect("should create repo");

        let repo_root_canonical = repo_root
            .canonicalize()
            .expect("repo_root should canonicalize");

        // Path points to a file that doesn't exist yet, but its parent (repo_root) does.
        let nonexistent_path = repo_root.join("nonexistent.md");

        let result = validation_dirs(&nonexistent_path, &repo_root_canonical);
        assert!(
            result.is_ok(),
            "should succeed even if target file doesn't exist yet, as long as its parent exists"
        );
    }

    #[test]
    fn validation_dirs_errors_when_parent_of_nonexistent_target_is_missing() {
        let temp_dir = tempfile::TempDir::new().expect("should create temp dir");
        let repo_root = temp_dir.path().join("repo");
        std::fs::create_dir_all(&repo_root).expect("should create repo");

        let repo_root_canonical = repo_root
            .canonicalize()
            .expect("repo_root should canonicalize");

        // Neither `missing-dir` nor the file inside it exist.
        let nonexistent_path = repo_root.join("missing-dir").join("nonexistent.md");

        let result = validation_dirs(&nonexistent_path, &repo_root_canonical);
        assert!(
            result.is_err(),
            "should error when the immediate parent directory doesn't exist"
        );
    }
}

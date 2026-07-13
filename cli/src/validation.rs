use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

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

impl std::error::Error for ParseError {}

pub fn parse_sections(text: &str) -> Result<BTreeMap<String, String>, ParseError> {
    let mut sections = BTreeMap::new();
    let lines: Vec<&str> = text.lines().collect();

    // Canonical section names (for normalization)
    let canonical_names = vec!["Build", "Lint", "Test", "Docs", "PR conventions", "Notes"];

    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let line_number = i + 1; // 1-based line number

        // Check if line looks like a heading attempt (starts with ## - section heading)
        if line.starts_with("##") {
            // Valid heading pattern: must be "## " (with space after ##)
            if !line.starts_with("## ") {
                // Malformed heading
                return Err(ParseError::new(format!(
                    "malformed heading on line {}: {}",
                    line_number, line
                )));
            }

            let raw_heading = line[3..].trim().to_string();
            let heading_lower = raw_heading.to_lowercase();

            // Normalize heading by matching against canonical names (case-insensitive)
            let normalized_heading = canonical_names
                .iter()
                .find(|canonical| canonical.to_lowercase() == heading_lower)
                .map(|canonical| canonical.to_string())
                .unwrap_or(raw_heading);

            // Collect following lines until next heading or end of file
            let mut body_lines = Vec::new();
            i += 1;

            while i < lines.len() && !lines[i].starts_with("## ") {
                let body_line = lines[i];
                let body_line_number = i + 1; // 1-based line number for body lines

                // Check if this line looks like a malformed heading (starts with ## but not ## )
                if body_line.starts_with("##") && !body_line.starts_with("## ") {
                    return Err(ParseError::new(format!(
                        "malformed heading on line {}: {}",
                        body_line_number, body_line
                    )));
                }

                // Normalize bullet markers: trim and treat - and * as equivalent
                let normalized_line = if body_line.trim_start().starts_with('-') || body_line.trim_start().starts_with('*') {
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

            // Join body lines and trim
            let body = body_lines.join("\n").trim().to_string();
            sections.insert(normalized_heading, body);
        } else {
            i += 1;
        }
    }

    // Check that all required sections are present
    let required_sections = vec!["Build", "Lint", "Test"];
    let missing_sections: Vec<&str> = required_sections
        .iter()
        .filter(|&&section| !sections.contains_key(section))
        .copied()
        .collect();

    if !missing_sections.is_empty() {
        return Err(ParseError::new(format!(
            "missing required section(s): {}",
            missing_sections.join(", ")
        )));
    }

    Ok(sections)
}

pub fn merge(layers: &[BTreeMap<String, String>]) -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
    for layer in layers {
        for (key, value) in layer {
            result.insert(key.clone(), value.clone());
        }
    }
    result
}

/// Find the git repository root by running `git rev-parse --show-toplevel`.
pub(crate) fn find_repo_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let output = std::process::Command::new("git")
        .args(&["rev-parse", "--show-toplevel"])
        .output()?;

    if !output.status.success() {
        return Err("git rev-parse --show-toplevel failed".into());
    }

    let root_path = String::from_utf8(output.stdout)?
        .trim()
        .to_string();

    Ok(PathBuf::from(root_path))
}

/// Resolve validation for a given path, returning the merged sections and the scope directory.
///
/// Walks from repo root down to the directory containing the path,
/// collecting and merging all validation.md files found along the way.
/// Returns (merged_sections, scope_dir_relative_to_repo_root).
fn resolve_validation_with_scope(
    path: &Path,
    repo_root: &Path,
) -> Result<(BTreeMap<String, String>, PathBuf), Box<dyn std::error::Error>> {
    // Canonicalize the path relative to repo root
    let target_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
    };

    // Get the directory containing the target path
    let target_dir = target_path.parent().unwrap_or_else(|| Path::new("."));

    // Collect all validation.md files from repo root down to target_dir
    let mut validation_files = Vec::new();
    let mut scope_dir = PathBuf::from(".");

    // Start from repo root and walk down to target_dir
    let mut current = repo_root.to_path_buf();
    validation_files.push(current.join("validation.md"));

    // Walk down the directory tree
    if let Ok(rel_path) = target_dir.strip_prefix(&repo_root) {
        for component in rel_path.components() {
            current.push(component);
            validation_files.push(current.join("validation.md"));
        }
    }

    // Parse each validation.md that exists and track the deepest one found
    let mut layers = Vec::new();
    for validation_file in &validation_files {
        if validation_file.exists() {
            let text = std::fs::read_to_string(&validation_file)?;
            let sections = parse_sections(&text)?;
            layers.push(sections);

            // Track the deepest directory that has a validation.md
            if let Some(parent) = validation_file.parent() {
                if let Ok(rel) = parent.strip_prefix(repo_root) {
                    scope_dir = rel.to_path_buf();
                }
            }
        }
    }

    if layers.is_empty() {
        return Err("no validation.md files found".into());
    }

    // Merge all layers
    let merged = merge(&layers);

    Ok((merged, scope_dir))
}

/// Resolve validation for a given path.
///
/// Walks from repo root down to the directory containing the path,
/// collecting and merging all validation.md files found along the way.
/// Returns the merged sections in fixed order: Build, Lint, Test, Docs, PR conventions, Notes.
pub(crate) fn resolve_validation(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let repo_root = find_repo_root()?;
    let (merged, _scope_dir) = resolve_validation_with_scope(path, &repo_root)?;

    // Print in fixed order: Build, Lint, Test, Docs, PR conventions, Notes
    let mut output = String::new();
    let order = vec!["Build", "Lint", "Test", "Docs", "PR conventions", "Notes"];
    for section_name in order {
        if let Some(body) = merged.get(section_name) {
            output.push_str(&format!("## {}\n{}\n\n", section_name, body));
        }
    }

    // Remove trailing newline added by the loop
    if output.ends_with("\n\n") {
        output.pop();
        output.pop();
    }
    output.push('\n');

    Ok(output)
}

/// Resolve validation for multiple paths, deduping by scope.
///
/// For each path, resolves validation and tracks the scope directory.
/// Paths that resolve to the same scope have their sections deduplicated.
/// Returns output with labeled blocks for each distinct scope.
pub(crate) fn resolve_validations(paths: &[PathBuf]) -> Result<String, Box<dyn std::error::Error>> {
    let repo_root = find_repo_root()?;

    // Resolve each path and group by scope
    let mut scope_to_sections: std::collections::BTreeMap<PathBuf, BTreeMap<String, String>> =
        std::collections::BTreeMap::new();

    for path in paths {
        let (merged, scope_dir) = resolve_validation_with_scope(path, &repo_root)?;
        scope_to_sections.insert(scope_dir, merged);
    }

    // Generate output for each unique scope
    let mut output = String::new();
    let order = vec!["Build", "Lint", "Test", "Docs", "PR conventions", "Notes"];

    for (scope_dir, merged) in scope_to_sections {
        // Format scope label (use "." for repo root)
        let scope_label = if scope_dir.as_os_str().is_empty() || scope_dir == PathBuf::from(".") {
            ".".to_string()
        } else {
            scope_dir.to_string_lossy().to_string()
        };

        // Add scope label comment
        output.push_str(&format!("### {}\n\n", scope_label));

        for section_name in &order {
            if let Some(body) = merged.get(*section_name) {
                output.push_str(&format!("## {}\n{}\n\n", section_name, body));
            }
        }
    }

    // Clean up trailing whitespace
    while output.ends_with("\n\n\n") {
        output.pop();
    }
    if !output.ends_with('\n') {
        output.push('\n');
    }

    Ok(output)
}

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

        // Check that we have exactly 6 sections
        assert_eq!(sections.len(), 6, "should have exactly 6 sections, got: {:?}", sections.keys().collect::<Vec<_>>());

        // Check all expected keys exist
        assert!(sections.contains_key("Build"), "should have Build section");
        assert!(sections.contains_key("Lint"), "should have Lint section");
        assert!(sections.contains_key("Test"), "should have Test section");
        assert!(sections.contains_key("Docs"), "should have Docs section");
        assert!(sections.contains_key("PR conventions"), "should have PR conventions section");
        assert!(sections.contains_key("Notes"), "should have Notes section");

        // Check body text is trimmed and matches
        assert_eq!(sections["Build"].trim(), "None, this is a plugin.");
        assert_eq!(sections["Lint"].trim(), "None, no linter configured.");
        assert_eq!(sections["Test"].trim(), "No automated test suite.");
        assert_eq!(sections["Docs"].trim(), "Keep README in sync.");
        assert_eq!(sections["PR conventions"].trim(), "Main branch: main");
        assert_eq!(sections["Notes"].trim(), "No CI configured.");
    }

    #[test]
    fn tolerates_case_whitespace_and_bullet_variance() {
        // Canonical fixture from Step 25
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

        // Mutated fixture with:
        // - lowercase headings (## build)
        // - extra leading/trailing spaces on heading lines
        // - bullets switched from - to * (and different forms of whitespace)
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

        let canonical_result = parse_sections(canonical);
        assert!(canonical_result.is_ok(), "canonical should parse");
        let canonical_sections = canonical_result.unwrap();

        let mutated_result = parse_sections(mutated);
        assert!(mutated_result.is_ok(), "mutated should parse");
        let mutated_sections = mutated_result.unwrap();

        // Keys should be identical after normalization (both should use canonical casing)
        assert_eq!(
            canonical_sections.keys().collect::<Vec<_>>(),
            mutated_sections.keys().collect::<Vec<_>>(),
            "keys should be identical after normalization"
        );

        // Body content should be identical
        for (key, canonical_body) in &canonical_sections {
            let mutated_body = mutated_sections.get(key)
                .expect(&format!("mutated should have key '{}'", key));

            // Normalize bullets: replace * with - for comparison
            let canonical_normalized = canonical_body.replace('*', "-");
            let mutated_normalized = mutated_body.replace('*', "-");

            assert_eq!(
                canonical_normalized,
                mutated_normalized,
                "body content should be identical for section '{}'",
                key
            );
        }
    }

    #[test]
    fn rejects_missing_required_section() {
        // Fixture identical to Step 25's canonical but with ## Test section deleted
        let fixture_without_test = r#"# Validation

## Build
None, this is a plugin.

## Lint
None, no linter configured.

## Docs
Keep README in sync.

## PR conventions
Main branch: main

## Notes
No CI configured."#;

        let result = parse_sections(fixture_without_test);

        // Should be Err, not Ok
        assert!(result.is_err(), "parse_sections should fail when Test section is missing");

        let error = result.unwrap_err();
        let error_msg = error.to_string();

        // Error message should mention "Test"
        assert!(error_msg.to_lowercase().contains("test"),
                "error message should mention 'Test', got: {}", error_msg);
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

        assert!(result.is_err(), "parse_sections should reject malformed heading");

        let error = result.unwrap_err();
        let error_msg = error.to_string();

        // Error message should include line number 9
        assert!(error_msg.contains("9"),
                "error message should include line number 9, got: {}", error_msg);
    }

    #[test]
    fn leaf_replaces_whole_sections() {
        // Root fixture (all 6 sections)
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

        let root_map = parse_sections(root_fixture)
            .expect("root fixture should parse");

        // Leaf fixture (only Build, Lint, Test with different content)
        let leaf_fixture = r#"## Build
Custom build command.

## Lint
Custom linter config.

## Test
Custom test runner."#;

        let mut leaf_map = BTreeMap::new();
        leaf_map.insert("Build".to_string(), "Custom build command.".to_string());
        leaf_map.insert("Lint".to_string(), "Custom linter config.".to_string());
        leaf_map.insert("Test".to_string(), "Custom test runner.".to_string());

        // Call merge with root then leaf
        let result = merge(&[root_map, leaf_map.clone()]);

        // Build, Lint, Test should equal leaf's values verbatim
        assert_eq!(result["Build"], "Custom build command.");
        assert_eq!(result["Lint"], "Custom linter config.");
        assert_eq!(result["Test"], "Custom test runner.");

        // Docs, PR conventions, Notes should equal root's values verbatim
        assert_eq!(result["Docs"].trim(), "Keep README in sync.");
        assert_eq!(result["PR conventions"].trim(), "Main branch: main");
        assert_eq!(result["Notes"].trim(), "No CI configured.");
    }

    #[test]
    fn three_level_merge_applies_nearest_override_per_section() {
        // Root fixture with all 6 sections
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

        let root_map = parse_sections(root_fixture)
            .expect("root fixture should parse");

        // Middle layer: only override Test
        let mut middle_map = BTreeMap::new();
        middle_map.insert("Test".to_string(), "middle test".to_string());

        // Leaf layer: only override Build
        let mut leaf_map = BTreeMap::new();
        leaf_map.insert("Build".to_string(), "leaf build".to_string());

        // Call merge with root, middle, leaf
        let result = merge(&[root_map, middle_map, leaf_map]);

        // Build should come from leaf
        assert_eq!(result["Build"], "leaf build");

        // Test should come from middle (nearest override for Test)
        assert_eq!(result["Test"].trim(), "middle test");

        // Everything else should come from root
        assert_eq!(result["Lint"].trim(), "root lint");
        assert_eq!(result["Docs"].trim(), "root docs");
        assert_eq!(result["PR conventions"].trim(), "root pr conventions");
        assert_eq!(result["Notes"].trim(), "root notes");
    }
}

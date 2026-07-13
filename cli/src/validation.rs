use std::collections::BTreeMap;
use std::fmt;

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

        // Check if line is a section heading (starts with "## ")
        if line.starts_with("## ") {
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
}

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

    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];

        // Check if line is a section heading (starts with "## ")
        if line.starts_with("## ") {
            let heading = line[3..].trim().to_string();

            // Collect following lines until next heading or end of file
            let mut body_lines = Vec::new();
            i += 1;

            while i < lines.len() && !lines[i].starts_with("## ") {
                body_lines.push(lines[i]);
                i += 1;
            }

            // Join body lines and trim
            let body = body_lines.join("\n").trim().to_string();
            sections.insert(heading, body);
        } else {
            i += 1;
        }
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
}

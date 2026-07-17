use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageType {
    Rust,
    Programming,
    Prose,
    Markup,
    Data,
    Unknown,
}

impl LanguageType {
    pub fn is_programming(&self) -> bool {
        matches!(self, LanguageType::Rust | LanguageType::Programming)
    }

    pub fn is_reviewable_source(&self) -> bool {
        matches!(
            self,
            LanguageType::Rust
                | LanguageType::Programming
                | LanguageType::Prose
                | LanguageType::Markup
        )
    }
}

/// Hand-maintained extension/filename -> LanguageType map, scoped to the
/// file kinds this repo actually contains. Deliberately not backed by a
/// third-party crate: see blueprint.md Open Risk on taxonomy staleness.
/// Extend this map (not a dependency) when a new extension needs a lane.
pub fn classify(path: &Path) -> LanguageType {
    if let Some("LICENSE" | "LICENSE.md" | "LICENSE.txt" | ".gitignore" | "Cargo.lock") =
        path.file_name().and_then(|n| n.to_str())
    {
        return LanguageType::Data;
    }

    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => LanguageType::Rust,
        Some("sh") | Some("py") | Some("js") | Some("ts") => LanguageType::Programming,
        Some("md") | Some("mdx") | Some("txt") => LanguageType::Prose,
        Some("html") | Some("xml") | Some("svg") => LanguageType::Markup,
        Some("yaml") | Some("yml") | Some("json") | Some("toml") | Some("lock") => {
            LanguageType::Data
        }
        _ => LanguageType::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_rust_source_as_rust() {
        assert_eq!(
            classify(Path::new("cli/src/domain/review.rs")),
            LanguageType::Rust
        );
    }

    #[test]
    fn classifies_markdown_as_prose() {
        assert_eq!(
            classify(Path::new("plugin/agents/cleaner.md")),
            LanguageType::Prose
        );
    }

    #[test]
    fn classifies_yaml_json_and_toml_as_data() {
        assert_eq!(classify(Path::new("config.yaml")), LanguageType::Data);
        assert_eq!(classify(Path::new("config.yml")), LanguageType::Data);
        assert_eq!(classify(Path::new("package.json")), LanguageType::Data);
        assert_eq!(classify(Path::new("Cargo.toml")), LanguageType::Data);
    }

    #[test]
    fn classifies_license_and_gitignore_as_data() {
        assert_eq!(classify(Path::new("LICENSE")), LanguageType::Data);
        assert_eq!(classify(Path::new(".gitignore")), LanguageType::Data);
    }

    #[test]
    fn classifies_html_as_markup() {
        assert_eq!(classify(Path::new("index.html")), LanguageType::Markup);
    }

    #[test]
    fn classifies_unknown_extension_as_unknown() {
        assert_eq!(classify(Path::new("archive.xyz")), LanguageType::Unknown);
    }
}

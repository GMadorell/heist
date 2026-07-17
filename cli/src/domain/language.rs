use linguist::{
    detect_language_by_extension, detect_language_by_filename, disambiguate, DetectedLanguage,
};
use linguist_types::LanguageType as LinguistLanguageType;
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

pub fn classify(path: &Path, content: Option<&str>) -> LanguageType {
    resolve(
        &detect_language_by_filename(path).unwrap_or_default(),
        path,
        content,
    )
    .or_else(|| {
        resolve(
            &detect_language_by_extension(path).unwrap_or_default(),
            path,
            content,
        )
    })
    .unwrap_or(LanguageType::Unknown)
}

fn resolve(
    candidates: &[DetectedLanguage],
    path: &Path,
    content: Option<&str>,
) -> Option<LanguageType> {
    match candidates {
        [] => None,
        [single] => Some(to_language_type(single)),
        multiple => {
            let disambiguated = content.and_then(|c| disambiguate(path, c).ok());
            if let Some([single]) = disambiguated.as_deref() {
                return Some(to_language_type(single));
            }
            Some(by_priority(multiple))
        }
    }
}

fn to_language_type(candidate: &DetectedLanguage) -> LanguageType {
    if candidate.name.eq_ignore_ascii_case("rust") {
        return LanguageType::Rust;
    }
    match candidate.definition.language_type {
        LinguistLanguageType::Programming => LanguageType::Programming,
        LinguistLanguageType::Markup => LanguageType::Markup,
        LinguistLanguageType::Prose => LanguageType::Prose,
        LinguistLanguageType::Data => LanguageType::Data,
    }
}

fn by_priority(candidates: &[DetectedLanguage]) -> LanguageType {
    if candidates.iter().any(|c| c.name == "Rust") {
        return LanguageType::Rust;
    }
    for language_type in [
        LinguistLanguageType::Programming,
        LinguistLanguageType::Markup,
        LinguistLanguageType::Prose,
        LinguistLanguageType::Data,
    ] {
        if candidates
            .iter()
            .any(|c| c.definition.language_type == language_type)
        {
            return to_language_type(
                candidates
                    .iter()
                    .find(|c| c.definition.language_type == language_type)
                    .expect("just matched by any()"),
            );
        }
    }
    LanguageType::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_rust_source_as_rust() {
        assert_eq!(
            classify(Path::new("cli/src/domain/review.rs"), None),
            LanguageType::Rust
        );
    }

    #[test]
    fn classifies_unambiguous_extension_without_content() {
        // `.rs` has only one Linguist match, so no content is needed to resolve it.
        assert_eq!(classify(Path::new("build.rs"), None), LanguageType::Rust);
    }

    #[test]
    fn disambiguates_markdown_from_gcc_machine_description_via_content() {
        // `.md` is ambiguous (Markdown vs. the programming-type "GCC Machine
        // Description"); ordinary prose content resolves it to Markdown.
        let content = "# Title\n\nSome prose about a project.\n";
        assert_eq!(
            classify(Path::new("plugin/agents/cleaner.md"), Some(content)),
            LanguageType::Prose
        );
    }

    #[test]
    fn ambiguous_extension_without_content_falls_back_to_programming() {
        // Without content to disambiguate, `.md` falls back to the
        // programming-biased priority order rather than guessing prose.
        assert_eq!(
            classify(Path::new("plugin/agents/cleaner.md"), None),
            LanguageType::Programming
        );
    }

    #[test]
    fn classifies_yaml_json_and_toml_as_data() {
        assert_eq!(classify(Path::new("config.yaml"), None), LanguageType::Data);
        assert_eq!(classify(Path::new("config.yml"), None), LanguageType::Data);
        assert_eq!(
            classify(Path::new("package.json"), None),
            LanguageType::Data
        );
        assert_eq!(classify(Path::new("Cargo.toml"), None), LanguageType::Data);
    }

    #[test]
    fn classifies_gitignore_as_data() {
        assert_eq!(classify(Path::new(".gitignore"), None), LanguageType::Data);
    }

    #[test]
    fn classifies_license_as_prose() {
        // Linguist has no dedicated "License" language; a bare `LICENSE`
        // filename matches "Text", which is prose type.
        assert_eq!(classify(Path::new("LICENSE"), None), LanguageType::Prose);
    }

    #[test]
    fn classifies_html_as_markup() {
        assert_eq!(
            classify(Path::new("index.html"), None),
            LanguageType::Markup
        );
    }

    #[test]
    fn classifies_unknown_extension_as_unknown() {
        assert_eq!(
            classify(Path::new("archive.xyz"), None),
            LanguageType::Unknown
        );
    }
}

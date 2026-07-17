use crate::domain::language::LanguageType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lane {
    Intent,
    Coverage,
    Quality,
    Simplicity,
    Rust,
}

impl Lane {
    pub fn as_str(&self) -> &'static str {
        match self {
            Lane::Intent => "intent",
            Lane::Coverage => "coverage",
            Lane::Quality => "quality",
            Lane::Simplicity => "simplicity",
            Lane::Rust => "rust",
        }
    }
}

pub fn select_lanes(language_types: &[LanguageType]) -> Vec<Lane> {
    let mut lanes = vec![Lane::Intent];

    if language_types.iter().any(|lt| lt.is_programming()) {
        lanes.push(Lane::Coverage);
    }
    if language_types.iter().any(|lt| lt.is_reviewable_source()) {
        lanes.push(Lane::Quality);
        lanes.push(Lane::Simplicity);
    }
    if language_types
        .iter()
        .any(|lt| matches!(lt, LanguageType::Rust))
    {
        lanes.push(Lane::Rust);
    }

    lanes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::language::LanguageType;

    #[test]
    fn empty_diff_selects_intent_only() {
        assert_eq!(select_lanes(&[]), vec![Lane::Intent]);
    }

    #[test]
    fn data_only_diff_selects_intent_only() {
        assert_eq!(select_lanes(&[LanguageType::Data]), vec![Lane::Intent]);
    }

    #[test]
    fn programming_file_adds_coverage_quality_simplicity() {
        assert_eq!(
            select_lanes(&[LanguageType::Programming]),
            vec![
                Lane::Intent,
                Lane::Coverage,
                Lane::Quality,
                Lane::Simplicity
            ]
        );
    }

    #[test]
    fn prose_only_diff_adds_quality_and_simplicity_but_not_coverage() {
        assert_eq!(
            select_lanes(&[LanguageType::Prose]),
            vec![Lane::Intent, Lane::Quality, Lane::Simplicity]
        );
    }

    #[test]
    fn rust_file_adds_all_gated_lanes_plus_rust() {
        assert_eq!(
            select_lanes(&[LanguageType::Rust]),
            vec![
                Lane::Intent,
                Lane::Coverage,
                Lane::Quality,
                Lane::Simplicity,
                Lane::Rust
            ]
        );
    }

    #[test]
    fn mixed_data_and_prose_skips_coverage_and_rust() {
        assert_eq!(
            select_lanes(&[LanguageType::Data, LanguageType::Prose]),
            vec![Lane::Intent, Lane::Quality, Lane::Simplicity]
        );
    }

    #[test]
    fn lane_as_str_matches_cli_output_names() {
        assert_eq!(Lane::Intent.as_str(), "intent");
        assert_eq!(Lane::Coverage.as_str(), "coverage");
        assert_eq!(Lane::Quality.as_str(), "quality");
        assert_eq!(Lane::Simplicity.as_str(), "simplicity");
        assert_eq!(Lane::Rust.as_str(), "rust");
    }
}

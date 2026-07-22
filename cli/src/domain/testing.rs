use crate::domain::value::{BranchValue, DateValue, NonBlankValue, RefValue, SlugValue};

/// Parse-or-panic constructors for value objects in test fixtures, where the
/// input is a hardcoded literal known to be valid and a parse failure means
/// the test itself is broken.
pub mod valid {
    use super::{BranchValue, DateValue, NonBlankValue, RefValue, SlugValue};

    pub fn slug(s: &str) -> SlugValue {
        SlugValue::parse(s).expect("valid slug")
    }

    pub fn date(s: &str) -> DateValue {
        DateValue::parse("date", s).expect("valid date")
    }

    pub fn branch(s: &str) -> NonBlankValue {
        NonBlankValue::parse("branch", s).expect("valid branch")
    }

    pub fn base(s: &str) -> NonBlankValue {
        NonBlankValue::parse("base", s).expect("valid base")
    }

    pub fn worktree(s: &str) -> NonBlankValue {
        NonBlankValue::parse("worktree", s).expect("valid worktree")
    }

    pub fn branch_value(s: &str) -> BranchValue {
        BranchValue::try_from_raw("branch", s).expect("valid branch")
    }

    pub fn ref_value(s: &str) -> RefValue {
        RefValue::try_from_raw(s).expect("valid ref")
    }
}

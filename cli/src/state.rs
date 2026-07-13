use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Casing,
    Planning,
    FenceReview,
    HumanReview,
    Forging,
    Safehouse,
    Implementing,
    Cleaning,
    Done,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn stage_serializes_to_snake_case() {
        // Test Casing
        let casing_json = serde_json::to_value(Stage::Casing).unwrap();
        assert_eq!(casing_json, json!("casing"));

        // Test Planning
        let planning_json = serde_json::to_value(Stage::Planning).unwrap();
        assert_eq!(planning_json, json!("planning"));

        // Test FenceReview
        let fence_review_json = serde_json::to_value(Stage::FenceReview).unwrap();
        assert_eq!(fence_review_json, json!("fence_review"));

        // Test HumanReview
        let human_review_json = serde_json::to_value(Stage::HumanReview).unwrap();
        assert_eq!(human_review_json, json!("human_review"));

        // Test Forging
        let forging_json = serde_json::to_value(Stage::Forging).unwrap();
        assert_eq!(forging_json, json!("forging"));

        // Test Safehouse
        let safehouse_json = serde_json::to_value(Stage::Safehouse).unwrap();
        assert_eq!(safehouse_json, json!("safehouse"));

        // Test Implementing
        let implementing_json = serde_json::to_value(Stage::Implementing).unwrap();
        assert_eq!(implementing_json, json!("implementing"));

        // Test Cleaning
        let cleaning_json = serde_json::to_value(Stage::Cleaning).unwrap();
        assert_eq!(cleaning_json, json!("cleaning"));

        // Test Done
        let done_json = serde_json::to_value(Stage::Done).unwrap();
        assert_eq!(done_json, json!("done"));
    }
}

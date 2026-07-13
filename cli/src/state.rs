use serde::{Deserialize, Serialize};

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub schema_version: u32,
    pub slug: String,
    pub stage: Stage,
    pub worktree: Option<String>,
    pub branch: Option<String>,
    pub score_step: u32,
    pub score_steps_total: u32,
    pub fence_rounds: u32,
    pub created: String,
    pub updated: String,
}

impl State {
    pub fn new(slug: &str) -> Self {
        let today = get_today_date();
        State {
            schema_version: 1,
            slug: slug.to_string(),
            stage: Stage::Casing,
            worktree: None,
            branch: None,
            score_step: 0,
            score_steps_total: 0,
            fence_rounds: 0,
            created: today.clone(),
            updated: today,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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

pub fn get_today_date() -> String {
    let output = std::process::Command::new("date")
        .arg("+%Y-%m-%d")
        .output()
        .expect("failed to get date");
    String::from_utf8(output.stdout)
        .expect("invalid utf8 from date command")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn new_state_has_expected_defaults() {
        let today = get_today_date();
        let state = State::new("my-slug");
        let json = serde_json::to_value(&state).expect("failed to serialize");

        // Check individual fields
        assert_eq!(json["schema_version"], 1, "schema_version should be 1");
        assert_eq!(json["slug"], "my-slug", "slug should be 'my-slug'");
        assert_eq!(json["stage"], "casing", "stage should be 'casing'");
        assert_eq!(json["worktree"], serde_json::Value::Null, "worktree should be null");
        assert_eq!(json["branch"], serde_json::Value::Null, "branch should be null");
        assert_eq!(json["score_step"], 0, "score_step should be 0");
        assert_eq!(json["score_steps_total"], 0, "score_steps_total should be 0");
        assert_eq!(json["fence_rounds"], 0, "fence_rounds should be 0");
        assert_eq!(json["created"], today, "created should be today's date");
        assert_eq!(json["updated"], today, "updated should be today's date");

        // Verify no "stages" key exists
        assert!(!json.get("stages").is_some(), "should not have 'stages' key");

        // Verify the object has exactly the expected keys
        let obj = json.as_object().expect("should be an object");
        let expected_keys = vec![
            "schema_version", "slug", "stage", "worktree", "branch",
            "score_step", "score_steps_total", "fence_rounds", "created", "updated"
        ];
        assert_eq!(obj.len(), expected_keys.len(), "object should have exactly {} keys, got: {:?}", expected_keys.len(), obj.keys().collect::<Vec<_>>());
    }

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

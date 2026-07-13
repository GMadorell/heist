use crate::state::Stage;

pub fn next_step(stage: Stage) -> u32 {
    match stage {
        Stage::Casing => 1,
        Stage::Planning => 2,
        Stage::FenceReview => 3,
        Stage::HumanReview => 4,
        Stage::Forging => 5,
        Stage::Safehouse => 6,
        Stage::Implementing => 6,
        Stage::Cleaning => 7,
        Stage::Done => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_every_stage_to_its_pipeline_step() {
        assert_eq!(next_step(Stage::Casing), 1);
        assert_eq!(next_step(Stage::Planning), 2);
        assert_eq!(next_step(Stage::FenceReview), 3);
        assert_eq!(next_step(Stage::HumanReview), 4);
        assert_eq!(next_step(Stage::Forging), 5);
        assert_eq!(next_step(Stage::Safehouse), 6);
        assert_eq!(next_step(Stage::Implementing), 6);
        assert_eq!(next_step(Stage::Cleaning), 7);
        assert_eq!(next_step(Stage::Done), 0);
    }
}

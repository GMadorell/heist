use crate::ports::tool_probe::ToolProbe;

const TOOLS: [&str; 3] = ["git", "gh", "crit"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolStatus {
    pub tool: &'static str,
    pub available: bool,
}

pub fn doctor(probe: &dyn ToolProbe) -> Vec<ToolStatus> {
    TOOLS
        .iter()
        .map(|&tool| ToolStatus {
            tool,
            available: probe.is_available(tool),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{doctor, ToolStatus};
    use crate::adapters::testing::FakeToolProbe;

    #[test]
    fn doctor_reports_each_tool_in_order() {
        let probe = FakeToolProbe::new()
            .with_available("git")
            .with_available("crit");
        let result = doctor(&probe);
        assert_eq!(
            result,
            vec![
                ToolStatus {
                    tool: "git",
                    available: true
                },
                ToolStatus {
                    tool: "gh",
                    available: false
                },
                ToolStatus {
                    tool: "crit",
                    available: true
                },
            ]
        );
    }
}

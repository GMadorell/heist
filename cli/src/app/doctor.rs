use crate::domain::tool::Tool;
use crate::ports::tool_probe::ToolProbe;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolStatus {
    pub tool: Tool,
    pub available: bool,
}

pub fn doctor(probe: &dyn ToolProbe) -> Vec<ToolStatus> {
    Tool::ALL
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
    use crate::domain::tool::Tool;

    #[test]
    fn doctor_reports_each_tool_in_order() {
        let probe = FakeToolProbe::new()
            .with_available(Tool::Git)
            .with_available(Tool::Crit);
        let result = doctor(&probe);
        assert_eq!(
            result,
            vec![
                ToolStatus {
                    tool: Tool::Git,
                    available: true
                },
                ToolStatus {
                    tool: Tool::Gh,
                    available: false
                },
                ToolStatus {
                    tool: Tool::Crit,
                    available: true
                },
            ]
        );
    }
}

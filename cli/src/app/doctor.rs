use crate::ports::tool_probe::ToolProbe;

const TOOLS: [&str; 3] = ["git", "gh", "crit"];

pub fn run_doctor(probe: &dyn ToolProbe) -> Vec<(&'static str, bool)> {
    TOOLS
        .iter()
        .map(|&tool| (tool, probe.is_available(tool)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::run_doctor;
    use crate::adapters::testing::FakeToolProbe;

    #[test]
    fn run_doctor_reports_each_tool_in_order() {
        let probe = FakeToolProbe::new()
            .with_available("git")
            .with_available("crit");
        let result = run_doctor(&probe);
        assert_eq!(result, vec![("git", true), ("gh", false), ("crit", true)]);
    }
}

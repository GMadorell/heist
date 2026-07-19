use crate::ports::tool_probe::ToolProbe;

pub struct RealToolProbe;

impl ToolProbe for RealToolProbe {
    fn is_available(&self, tool: &str) -> bool {
        std::env::var_os("PATH")
            .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(tool).is_file()))
            .unwrap_or(false)
    }
}

pub trait ToolProbe {
    fn is_available(&self, tool: &str) -> bool;
}

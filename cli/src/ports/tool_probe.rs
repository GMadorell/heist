use crate::domain::tool::Tool;

pub trait ToolProbe {
    fn is_available(&self, tool: Tool) -> bool;
}

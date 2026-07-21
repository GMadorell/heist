use crate::domain::tool::Tool;
use crate::ports::tool_probe::ToolProbe;

pub struct RealToolProbe;

impl ToolProbe for RealToolProbe {
    fn is_available(&self, tool: Tool) -> bool {
        found_on_path(tool.binary_name())
    }
}

fn found_on_path(binary_name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(binary_name).is_file()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // Both cases below mutate the process-global `PATH` env var, so they run
    // as a single test to avoid racing with other tests / each other under
    // cargo test's default parallel threads.
    #[test]
    fn is_available_reflects_path_contents() {
        let dir =
            std::env::temp_dir().join(format!("heist-real-tool-probe-test-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("failed to create temp dir");
        let tool_path = dir.join("my-fake-tool");
        fs::write(&tool_path, "#!/bin/sh\n").expect("failed to write fake tool");

        let original_path = std::env::var_os("PATH");

        std::env::set_var("PATH", &dir);
        let found = found_on_path("my-fake-tool");
        let missing = found_on_path("definitely-not-a-real-tool");

        std::env::remove_var("PATH");
        let found_without_path = found_on_path("my-fake-tool");

        if let Some(path) = original_path {
            std::env::set_var("PATH", path);
        } else {
            std::env::remove_var("PATH");
        }
        fs::remove_dir_all(&dir).ok();

        assert!(found, "expected my-fake-tool to be found on PATH");
        assert!(
            !missing,
            "expected a nonexistent tool to be reported missing"
        );
        assert!(
            !found_without_path,
            "expected no PATH to mean nothing is available"
        );
    }
}

use super::{ProcessGuard, Tool, ToolError, ToolInvocation, ToolInvocationResult, ToolResult};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

#[derive(Clone)]
pub struct ToolRegistry {
    tools: HashMap<String, Tool>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Tool) -> Result<(), ToolError> {
        if self.tools.contains_key(&tool.name) {
            return Err(ToolError::AlreadyRegistered(tool.name.clone()));
        }
        self.tools.insert(tool.name.clone(), tool);
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&Tool> {
        self.tools.get(name)
    }

    pub async fn invoke(
        &self,
        invocation: &ToolInvocation,
    ) -> Result<ToolInvocationResult, ToolError> {
        let tool = self
            .get(&invocation.tool_name)
            .ok_or_else(|| ToolError::ToolNotFound(invocation.tool_name.clone()))?;

        let mut cmd = tokio::process::Command::new(&tool.executable);
        cmd.args(&tool.default_args);
        cmd.args(&invocation.args);

        if let Some(ref working_dir) = invocation.working_dir {
            cmd.current_dir(working_dir);
        }

        for (key, value) in &invocation.env {
            cmd.env(key, value);
        }

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let child = cmd
            .spawn()
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to spawn: {}", e)))?;

        let pid = child
            .id()
            .ok_or_else(|| ToolError::ExecutionFailed("Failed to get process ID".to_string()))?;

        let guard = ProcessGuard::new(pid, &tool.name);

        let timeout_duration = Duration::from_secs(30);

        let output = match tokio::time::timeout(timeout_duration, child.wait_with_output()).await {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => {
                return Err(ToolError::ExecutionFailed(format!(
                    "Failed to wait for process: {}",
                    e
                )))
            }
            Err(_) => {
                let _ = guard.terminate();
                return Err(ToolError::Timeout(format!(
                    "Tool {} timed out after {:?}",
                    invocation.tool_name, timeout_duration
                )));
            }
        };

        guard
            .terminated
            .store(true, std::sync::atomic::Ordering::SeqCst);

        let exit_code = output.status.code();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let result = ToolResult::new(exit_code, stdout, stderr);

        Ok(ToolInvocationResult::completed(result))
    }

    pub fn list_tools(&self) -> Vec<&str> {
        self.tools.keys().map(|k| k.as_str()).collect()
    }

    pub fn is_registered(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    pub fn with_standard_tools() -> Self {
        let mut registry = Self::new();

        let find_tool = |name: &str| -> Option<PathBuf> {
            match Command::new("which").arg(name).output() {
                Ok(output) => {
                    if output.status.success() {
                        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        Some(PathBuf::from(path))
                    } else {
                        None
                    }
                }
                Err(_) => None,
            }
        };

        if let Some(path) = find_tool("magellan") {
            let tool = Tool::new("magellan", path).description("Graph-based code indexer");
            if registry.register(tool).is_ok() {
                eprintln!("Registered standard tool: magellan");
            }
        } else {
            eprintln!("Warning: magellan not found in PATH");
        }

        if let Some(path) = find_tool("cargo") {
            let tool = Tool::new("cargo", path).description("Rust package manager");
            if registry.register(tool).is_ok() {
                eprintln!("Registered standard tool: cargo");
            }
        } else {
            eprintln!("Warning: cargo not found in PATH");
        }

        if let Some(path) = find_tool("splice") {
            let tool = Tool::new("splice", path).description("Precision code editor");
            if registry.register(tool).is_ok() {
                eprintln!("Registered standard tool: splice");
            }
        } else {
            eprintln!("Warning: splice not found in PATH");
        }

        registry
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::with_standard_tools()
    }
}

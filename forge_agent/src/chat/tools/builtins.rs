use super::registry::AsyncTool;
use super::types::ToolDef;
use async_trait::async_trait;

fn validate_path(working_dir: &std::path::Path, path: &str) -> Result<std::path::PathBuf, String> {
    if path.contains('\0') {
        return Err(format!("Path contains null bytes: {path}"));
    }
    if path.starts_with('/') || path.starts_with('\\') {
        return Err(format!("Absolute paths not allowed: {path}"));
    }
    for component in std::path::Path::new(path).components() {
        match component {
            std::path::Component::ParentDir => {
                return Err(format!("Path traversal not allowed: {path}"));
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                return Err(format!("Absolute paths not allowed: {path}"));
            }
            std::path::Component::CurDir | std::path::Component::Normal(_) => {}
        }
    }
    let full = working_dir.join(path);
    let canonical_working = working_dir
        .canonicalize()
        .unwrap_or_else(|_| working_dir.to_path_buf());
    if let Ok(canonical) = full.canonicalize() {
        if !canonical.starts_with(&canonical_working) {
            return Err(format!(
                "Path escapes working directory: {} (resolved to {})",
                path,
                canonical.display()
            ));
        }
    }
    Ok(full)
}

pub struct FileReadTool {
    working_dir: std::path::PathBuf,
}

impl FileReadTool {
    pub fn new(working_dir: impl Into<std::path::PathBuf>) -> Self {
        FileReadTool {
            working_dir: working_dir.into(),
        }
    }
}

#[async_trait]
impl AsyncTool for FileReadTool {
    async fn call(&self, arguments: serde_json::Value) -> Result<String, String> {
        let path = arguments["path"]
            .as_str()
            .ok_or_else(|| "Missing 'path' parameter".to_string())?;
        let full = validate_path(&self.working_dir, path)?;
        tokio::fs::read_to_string(&full)
            .await
            .map_err(|e| format!("Failed to read {}: {e}", full.display()))
    }

    fn definition(&self) -> ToolDef {
        ToolDef::new(
            "file_read",
            "Read the contents of a file. Path is relative to the project root.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path to the file"
                    }
                },
                "required": ["path"]
            }),
        )
    }
}

pub struct FileWriteTool {
    working_dir: std::path::PathBuf,
}

impl FileWriteTool {
    pub fn new(working_dir: impl Into<std::path::PathBuf>) -> Self {
        FileWriteTool {
            working_dir: working_dir.into(),
        }
    }
}

#[async_trait]
impl AsyncTool for FileWriteTool {
    async fn call(&self, arguments: serde_json::Value) -> Result<String, String> {
        let path = arguments["path"]
            .as_str()
            .ok_or_else(|| "Missing 'path' parameter".to_string())?;
        let content = arguments["content"]
            .as_str()
            .ok_or_else(|| "Missing 'content' parameter".to_string())?;
        let full = validate_path(&self.working_dir, path)?;
        if let Some(parent) = full.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("Failed to create directory: {e}"))?;
        }
        tokio::fs::write(&full, content)
            .await
            .map_err(|e| format!("Failed to write {}: {e}", full.display()))?;
        Ok(format!("Wrote {} bytes to {}", content.len(), path))
    }

    fn definition(&self) -> ToolDef {
        ToolDef::new(
            "file_write",
            "Write content to a file. Creates parent directories if needed. Path is relative to project root.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path to the file"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to write"
                    }
                },
                "required": ["path", "content"]
            }),
        )
    }
}

pub struct ShellExecTool {
    working_dir: std::path::PathBuf,
    timeout_secs: u64,
}

impl ShellExecTool {
    pub fn new(working_dir: impl Into<std::path::PathBuf>) -> Self {
        ShellExecTool {
            working_dir: working_dir.into(),
            timeout_secs: 30,
        }
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }
}

#[async_trait]
impl AsyncTool for ShellExecTool {
    async fn call(&self, arguments: serde_json::Value) -> Result<String, String> {
        let command = arguments["command"]
            .as_str()
            .ok_or_else(|| "Missing 'command' parameter".to_string())?;
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(self.timeout_secs),
            tokio::process::Command::new("sh")
                .arg("-c")
                .arg(command)
                .current_dir(&self.working_dir)
                .output(),
        )
        .await
        .map_err(|_| format!("Command timed out after {}s", self.timeout_secs))?
        .map_err(|e| format!("Failed to execute command: {e}"))?;

        let stdout = String::from_utf8_lossy(&result.stdout);
        let stderr = String::from_utf8_lossy(&result.stderr);
        if result.status.success() {
            Ok(stdout.to_string())
        } else {
            Err(format!(
                "Exit code {}: {}{}",
                result.status.code().unwrap_or(-1),
                stdout,
                if stderr.is_empty() {
                    String::new()
                } else {
                    format!("\nstderr: {stderr}")
                }
            ))
        }
    }

    fn definition(&self) -> ToolDef {
        ToolDef::new(
            "shell_exec",
            "Execute a shell command in the project directory. Returns stdout on success, error message on failure.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Shell command to execute"
                    }
                },
                "required": ["command"]
            }),
        )
    }
}

pub fn default_builtin_tools(
    working_dir: impl Into<std::path::PathBuf>,
) -> Vec<Box<dyn AsyncTool>> {
    let dir = working_dir.into();
    vec![
        Box::new(FileReadTool::new(dir.clone())),
        Box::new(FileWriteTool::new(dir.clone())),
        Box::new(ShellExecTool::new(dir)),
    ]
}

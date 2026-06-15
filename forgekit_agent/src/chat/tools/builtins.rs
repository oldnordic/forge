use super::registry::AsyncTool;
use super::types::ToolDef;
use crate::chat::sandbox::SharedSandbox;
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
    sandbox: SharedSandbox,
}

impl FileReadTool {
    pub fn new(working_dir: impl Into<std::path::PathBuf>) -> Self {
        FileReadTool {
            working_dir: working_dir.into(),
            sandbox: crate::chat::sandbox::shared_sandbox(None),
        }
    }

    pub fn with_sandbox(mut self, sandbox: SharedSandbox) -> Self {
        self.sandbox = sandbox;
        self
    }
}

#[async_trait]
impl AsyncTool for FileReadTool {
    async fn call(&self, arguments: serde_json::Value) -> Result<String, String> {
        let path = arguments["path"]
            .as_str()
            .ok_or_else(|| "Missing 'path' parameter".to_string())?;
        if let Some(ref sandbox) = *self.sandbox.lock() {
            sandbox.is_path_allowed(path)?;
        }
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
    sandbox: SharedSandbox,
}

impl FileWriteTool {
    pub fn new(working_dir: impl Into<std::path::PathBuf>) -> Self {
        FileWriteTool {
            working_dir: working_dir.into(),
            sandbox: crate::chat::sandbox::shared_sandbox(None),
        }
    }

    pub fn with_sandbox(mut self, sandbox: SharedSandbox) -> Self {
        self.sandbox = sandbox;
        self
    }
}

#[async_trait]
impl AsyncTool for FileWriteTool {
    async fn call(&self, arguments: serde_json::Value) -> Result<String, String> {
        let path = arguments["path"]
            .as_str()
            .ok_or_else(|| "Missing 'path' parameter".to_string())?;
        if let Some(ref sandbox) = *self.sandbox.lock() {
            sandbox.is_path_allowed(path)?;
        }
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
    sandbox: SharedSandbox,
}

impl ShellExecTool {
    pub fn new(working_dir: impl Into<std::path::PathBuf>) -> Self {
        ShellExecTool {
            working_dir: working_dir.into(),
            timeout_secs: 30,
            sandbox: crate::chat::sandbox::shared_sandbox(None),
        }
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    pub fn with_sandbox(mut self, sandbox: SharedSandbox) -> Self {
        self.sandbox = sandbox;
        self
    }
}

#[async_trait]
impl AsyncTool for ShellExecTool {
    async fn call(&self, arguments: serde_json::Value) -> Result<String, String> {
        let command = arguments["command"]
            .as_str()
            .ok_or_else(|| "Missing 'command' parameter".to_string())?;
        if let Some(ref sandbox) = *self.sandbox.lock() {
            sandbox.is_command_allowed(command)?;
        }
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

pub struct GraphQueryTool {
    forge: forgekit_core::Forge,
}

impl GraphQueryTool {
    pub fn new(forge: forgekit_core::Forge) -> Self {
        GraphQueryTool { forge }
    }
}

#[async_trait]
impl AsyncTool for GraphQueryTool {
    async fn call(&self, arguments: serde_json::Value) -> Result<String, String> {
        let command = arguments["command"]
            .as_str()
            .ok_or_else(|| "Missing 'command' parameter".to_string())?;

        match command {
            "find_symbol" => {
                let name = arguments["name"]
                    .as_str()
                    .ok_or_else(|| "Missing 'name' parameter for find_symbol".to_string())?;
                let symbols = self
                    .forge
                    .graph()
                    .find_symbol(name)
                    .await
                    .map_err(|e| format!("find_symbol failed: {e}"))?;
                Ok(format_symbols(&symbols))
            }
            "callers_of" => {
                let name = arguments["name"]
                    .as_str()
                    .ok_or_else(|| "Missing 'name' parameter for callers_of".to_string())?;
                let callers = self
                    .forge
                    .graph()
                    .callers_of(name)
                    .await
                    .map_err(|e| format!("callers_of failed: {e}"))?;
                Ok(format_references(&callers))
            }
            "references" => {
                let name = arguments["name"]
                    .as_str()
                    .ok_or_else(|| "Missing 'name' parameter for references".to_string())?;
                let refs = self
                    .forge
                    .graph()
                    .references(name)
                    .await
                    .map_err(|e| format!("references failed: {e}"))?;
                Ok(format_references(&refs))
            }
            "cycles" => {
                let cycles = self
                    .forge
                    .graph()
                    .cycles()
                    .await
                    .map_err(|e| format!("cycles failed: {e}"))?;
                Ok(format_cycles(&cycles))
            }
            "impact_analysis" => {
                let name = arguments["name"]
                    .as_str()
                    .ok_or_else(|| "Missing 'name' parameter for impact_analysis".to_string())?;
                let max_hops = arguments["max_hops"].as_u64().map(|h| h as u32);
                let impacted = self
                    .forge
                    .graph()
                    .impact_analysis(name, max_hops)
                    .await
                    .map_err(|e| format!("impact_analysis failed: {e}"))?;
                Ok(format_impacted(&impacted))
            }
            _ => Err(format!(
                "Unknown graph command: '{command}'. Available: find_symbol, callers_of, references, cycles, impact_analysis"
            )),
        }
    }

    fn definition(&self) -> ToolDef {
        ToolDef::new(
            "graph_query",
            "Query the code graph for symbol information. Commands: find_symbol (find symbols by name), callers_of (find who calls a symbol), references (find all cross-file references), cycles (detect call-graph cycles), impact_analysis (find symbols affected by changing a symbol).",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "enum": ["find_symbol", "callers_of", "references", "cycles", "impact_analysis"],
                        "description": "The graph query to execute"
                    },
                    "name": {
                        "type": "string",
                        "description": "Symbol name (required for find_symbol, callers_of, references, impact_analysis)"
                    },
                    "max_hops": {
                        "type": "integer",
                        "description": "Maximum traversal depth for impact_analysis (default: 2)"
                    }
                },
                "required": ["command"]
            }),
        )
    }
}

fn format_symbols(symbols: &[forgekit_core::types::Symbol]) -> String {
    if symbols.is_empty() {
        return "No symbols found.".to_string();
    }
    let lines: Vec<String> = symbols
        .iter()
        .map(|s| {
            format!(
                "- {} ({}): {}:{}  kind={:?}",
                s.name,
                s.fully_qualified_name,
                s.location.file_path.display(),
                s.location.line_number,
                s.kind,
            )
        })
        .collect();
    format!("Found {} symbol(s):\n{}", symbols.len(), lines.join("\n"))
}

fn format_references(refs: &[forgekit_core::types::Reference]) -> String {
    if refs.is_empty() {
        return "No references found.".to_string();
    }
    let lines: Vec<String> = refs
        .iter()
        .map(|r| {
            let from = r.from_name.as_deref().unwrap_or("<unknown>");
            let to = r.to_name.as_deref().unwrap_or("<unknown>");
            format!(
                "- {from} -> {to} at {}:{} ({:?})",
                r.location.file_path.display(),
                r.location.line_number,
                r.kind,
            )
        })
        .collect();
    format!("Found {} reference(s):\n{}", refs.len(), lines.join("\n"))
}

fn format_cycles(cycles: &[forgekit_core::types::Cycle]) -> String {
    if cycles.is_empty() {
        return "No cycles detected.".to_string();
    }
    let lines: Vec<String> = cycles
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let members: Vec<String> = c
                .members
                .iter()
                .map(|m| {
                    let fqn = m.fqn.as_deref().unwrap_or("<unknown>");
                    format!("{fqn} ({}:{})", m.file_path, m.kind)
                })
                .collect();
            format!("Cycle {}: {}", i + 1, members.join(" <-> "))
        })
        .collect();
    format!("Found {} cycle(s):\n{}", cycles.len(), lines.join("\n"))
}

fn format_impacted(impacted: &[forgekit_core::graph::ImpactedSymbol]) -> String {
    if impacted.is_empty() {
        return "No impacted symbols found.".to_string();
    }
    let lines: Vec<String> = impacted
        .iter()
        .map(|s| {
            format!(
                "- {} ({}): {}  hop={} edge={}",
                s.name, s.kind, s.file_path, s.hop_distance, s.edge_type,
            )
        })
        .collect();
    format!(
        "Found {} impacted symbol(s):\n{}",
        impacted.len(),
        lines.join("\n")
    )
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

pub fn default_builtin_tools_sandboxed(
    working_dir: impl Into<std::path::PathBuf>,
    sandbox: SharedSandbox,
) -> Vec<Box<dyn AsyncTool>> {
    let dir = working_dir.into();
    vec![
        Box::new(FileReadTool::new(dir.clone()).with_sandbox(sandbox.clone())),
        Box::new(FileWriteTool::new(dir.clone()).with_sandbox(sandbox.clone())),
        Box::new(ShellExecTool::new(dir).with_sandbox(sandbox)),
    ]
}

pub fn default_builtin_tools_with_graph(
    working_dir: impl Into<std::path::PathBuf>,
    forge: forgekit_core::Forge,
) -> Vec<Box<dyn AsyncTool>> {
    let dir = working_dir.into();
    vec![
        Box::new(FileReadTool::new(dir.clone())),
        Box::new(FileWriteTool::new(dir.clone())),
        Box::new(ShellExecTool::new(dir)),
        Box::new(GraphQueryTool::new(forge)),
    ]
}

pub fn default_builtin_tools_with_graph_sandboxed(
    working_dir: impl Into<std::path::PathBuf>,
    forge: forgekit_core::Forge,
    sandbox: SharedSandbox,
) -> Vec<Box<dyn AsyncTool>> {
    let dir = working_dir.into();
    vec![
        Box::new(FileReadTool::new(dir.clone()).with_sandbox(sandbox.clone())),
        Box::new(FileWriteTool::new(dir.clone()).with_sandbox(sandbox.clone())),
        Box::new(ShellExecTool::new(dir).with_sandbox(sandbox)),
        Box::new(GraphQueryTool::new(forge)),
    ]
}

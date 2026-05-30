use crate::chat::tools::builtins::{FileReadTool, FileWriteTool, ShellExecTool};
use crate::chat::tools::registry::{AsyncTool, BuiltinToolRegistry, ToolRegistry};
use crate::chat::tools::types::{ToolCall, ToolDef, ToolOutput};

#[test]
fn tool_def_construction() {
    let def = ToolDef::new(
        "my_tool",
        "Does something useful",
        serde_json::json!({"type": "object", "properties": {"x": {"type": "integer"}}}),
    );
    assert_eq!(def.name, "my_tool");
    assert_eq!(def.description, "Does something useful");
    assert_eq!(def.parameters["properties"]["x"]["type"], "integer");
}

#[test]
fn tool_def_empty_has_empty_properties() {
    let def = ToolDef::empty("noop", "Does nothing");
    assert_eq!(def.parameters["properties"].as_object().unwrap().len(), 0);
}

#[test]
fn tool_call_construction() {
    let call = ToolCall::new("c1", "file_read", serde_json::json!({"path": "a.rs"}));
    assert_eq!(call.id, "c1");
    assert_eq!(call.name, "file_read");
    assert_eq!(call.arguments["path"], "a.rs");
}

#[test]
fn tool_output_success() {
    let out = ToolOutput::success("c1", "file contents");
    assert!(!out.is_error);
    assert_eq!(out.content, "file contents");
    assert_eq!(out.tool_call_id, "c1");
}

#[test]
fn tool_output_error() {
    let out = ToolOutput::error("c1", "file not found");
    assert!(out.is_error);
    assert_eq!(out.content, "file not found");
}

#[test]
fn tool_def_serde_roundtrip() {
    let def = ToolDef::new("t", "desc", serde_json::json!({"type": "object"}));
    let json = serde_json::to_string(&def).expect("serialize");
    let back: ToolDef = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(def, back);
}

#[tokio::test]
async fn registry_unknown_tool_returns_error() {
    let registry = BuiltinToolRegistry::new();
    let call = ToolCall::new("c1", "nonexistent", serde_json::json!({}));
    let output = registry.execute(&call).await;
    assert!(output.is_error);
    assert!(output.content.contains("Unknown tool"));
}

#[tokio::test]
async fn registry_has_tool() {
    let mut registry = BuiltinToolRegistry::new();
    assert!(!registry.has_tool("echo"));
    registry.register(Box::new(EchoTool));
    assert!(registry.has_tool("echo"));
}

#[tokio::test]
async fn registry_definitions() {
    let mut registry = BuiltinToolRegistry::new();
    registry.register(Box::new(EchoTool));
    let defs = registry.definitions();
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].name, "echo");
}

#[tokio::test]
async fn registry_execute_known_tool() {
    let mut registry = BuiltinToolRegistry::new();
    registry.register(Box::new(EchoTool));
    let call = ToolCall::new("c1", "echo", serde_json::json!({"message": "hello"}));
    let output = registry.execute(&call).await;
    assert!(!output.is_error);
    assert_eq!(output.content, "hello");
}

#[tokio::test]
async fn registry_execute_tool_error() {
    let mut registry = BuiltinToolRegistry::new();
    registry.register(Box::new(EchoTool));
    let call = ToolCall::new("c1", "echo", serde_json::json!({}));
    let output = registry.execute(&call).await;
    assert!(output.is_error);
    assert!(output.content.contains("missing required argument"));
}

#[tokio::test]
async fn file_read_tool_writes_and_reads() {
    let dir = tempfile::tempdir().expect("tempdir");
    let write_tool = FileWriteTool::new(dir.path());
    let read_tool = FileReadTool::new(dir.path());

    write_tool
        .call(serde_json::json!({"path": "test.txt", "content": "hello world"}))
        .await
        .expect("write");

    let content = read_tool
        .call(serde_json::json!({"path": "test.txt"}))
        .await
        .expect("read");
    assert_eq!(content, "hello world");
}

#[tokio::test]
async fn file_read_tool_path_escape_blocked() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tool = FileReadTool::new(dir.path());
    let result = tool
        .call(serde_json::json!({"path": "../../etc/passwd"}))
        .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("traversal"));
}

#[tokio::test]
async fn file_write_tool_path_escape_blocked() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tool = FileWriteTool::new(dir.path());
    let result = tool
        .call(serde_json::json!({"path": "/tmp/evil", "content": "pwned"}))
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn shell_exec_tool_runs_command() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tool = ShellExecTool::new(dir.path());
    let output = tool
        .call(serde_json::json!({"command": "echo hello"}))
        .await
        .expect("exec");
    assert_eq!(output.trim(), "hello");
}

#[tokio::test]
async fn shell_exec_tool_captures_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tool = ShellExecTool::new(dir.path());
    let result = tool.call(serde_json::json!({"command": "exit 1"})).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Exit code 1"));
}

struct EchoTool;

#[async_trait::async_trait]
impl AsyncTool for EchoTool {
    async fn call(&self, arguments: serde_json::Value) -> Result<String, String> {
        arguments["message"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "Missing 'message' parameter".to_string())
    }

    fn definition(&self) -> ToolDef {
        ToolDef::new(
            "echo",
            "Echoes back the message parameter",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "message": {"type": "string"}
                },
                "required": ["message"]
            }),
        )
    }
}

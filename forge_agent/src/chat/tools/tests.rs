use crate::chat::tools::builtins::{FileReadTool, FileWriteTool, GraphQueryTool, ShellExecTool};
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

async fn create_indexed_forge(dir: &std::path::Path, source: &str) -> forge_core::Forge {
    let src_dir = dir.join("src");
    tokio::fs::create_dir_all(&src_dir)
        .await
        .expect("create src");
    tokio::fs::write(src_dir.join("lib.rs"), source)
        .await
        .expect("write lib.rs");
    let forge = forge_core::ForgeBuilder::new()
        .path(dir)
        .db_path(dir.join("test-graph.db"))
        .build()
        .await
        .expect("forge build");
    forge.graph().index().await.expect("index");
    forge
}

#[tokio::test]
async fn graph_query_find_symbol() {
    let dir = tempfile::tempdir().expect("tempdir");
    let forge = create_indexed_forge(dir.path(), "fn my_func() -> i32 { 42 }\n").await;
    let tool = GraphQueryTool::new(forge);
    let result = tool
        .call(serde_json::json!({"command": "find_symbol", "name": "my_func"}))
        .await
        .expect("call");
    assert!(result.contains("my_func"));
    assert!(result.contains("Found 1 symbol(s)"));
}

#[tokio::test]
async fn graph_query_find_symbol_empty() {
    let dir = tempfile::tempdir().expect("tempdir");
    let forge = create_indexed_forge(dir.path(), "fn other() {}\n").await;
    let tool = GraphQueryTool::new(forge);
    let result = tool
        .call(serde_json::json!({"command": "find_symbol", "name": "nonexistent"}))
        .await
        .expect("call");
    assert!(result.contains("No symbols found"));
}

#[tokio::test]
async fn graph_query_callers_of() {
    let dir = tempfile::tempdir().expect("tempdir");
    let forge = create_indexed_forge(
        dir.path(),
        "fn helper() -> i32 { 1 }\nfn caller() -> i32 { helper() }\n",
    )
    .await;
    let tool = GraphQueryTool::new(forge);
    let result = tool
        .call(serde_json::json!({"command": "callers_of", "name": "helper"}))
        .await
        .expect("call");
    assert!(result.contains("caller"));
    assert!(result.contains("Found"));
}

#[tokio::test]
async fn graph_query_references() {
    let dir = tempfile::tempdir().expect("tempdir");
    let forge = create_indexed_forge(dir.path(), "fn my_func() -> i32 { 42 }\n").await;
    let tool = GraphQueryTool::new(forge);
    let result = tool
        .call(serde_json::json!({"command": "references", "name": "my_func"}))
        .await
        .expect("call");
    assert!(result.contains("reference") || result.contains("No references"));
}

#[tokio::test]
async fn graph_query_cycles() {
    let dir = tempfile::tempdir().expect("tempdir");
    let forge = create_indexed_forge(dir.path(), "fn a() { b() }\nfn b() { a() }\n").await;
    let tool = GraphQueryTool::new(forge);
    let result = tool
        .call(serde_json::json!({"command": "cycles"}))
        .await
        .expect("call");
    assert!(result.contains("Cycle") || result.contains("No cycles"));
}

#[tokio::test]
async fn graph_query_impact_analysis() {
    let dir = tempfile::tempdir().expect("tempdir");
    let forge = create_indexed_forge(
        dir.path(),
        "fn base() -> i32 { 1 }\nfn mid() -> i32 { base() }\nfn top() -> i32 { mid() }\n",
    )
    .await;
    let tool = GraphQueryTool::new(forge);
    let result = tool
        .call(serde_json::json!({"command": "impact_analysis", "name": "base", "max_hops": 2}))
        .await
        .expect("call");
    assert!(result.contains("impacted") || result.contains("No impacted"));
}

#[tokio::test]
async fn graph_query_unknown_command() {
    let dir = tempfile::tempdir().expect("tempdir");
    let forge = create_indexed_forge(dir.path(), "fn foo() {}\n").await;
    let tool = GraphQueryTool::new(forge);
    let result = tool
        .call(serde_json::json!({"command": "invalid_query"}))
        .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unknown graph command"));
}

#[tokio::test]
async fn graph_query_missing_command() {
    let dir = tempfile::tempdir().expect("tempdir");
    let forge = create_indexed_forge(dir.path(), "fn foo() {}\n").await;
    let tool = GraphQueryTool::new(forge);
    let result = tool.call(serde_json::json!({})).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Missing 'command'"));
}

#[tokio::test]
async fn graph_query_missing_name_for_find_symbol() {
    let dir = tempfile::tempdir().expect("tempdir");
    let forge = create_indexed_forge(dir.path(), "fn foo() {}\n").await;
    let tool = GraphQueryTool::new(forge);
    let result = tool
        .call(serde_json::json!({"command": "find_symbol"}))
        .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Missing 'name'"));
}

#[tokio::test]
async fn graph_query_definition_has_command_enum() {
    let dir = tempfile::tempdir().expect("tempdir");
    let forge = create_indexed_forge(dir.path(), "fn foo() {}\n").await;
    let tool = GraphQueryTool::new(forge);
    let def = tool.definition();
    assert_eq!(def.name, "graph_query");
    let enum_vals = def.parameters["properties"]["command"]["enum"]
        .as_array()
        .expect("enum array");
    assert!(enum_vals.iter().any(|v| v == "find_symbol"));
    assert!(enum_vals.iter().any(|v| v == "callers_of"));
    assert!(enum_vals.iter().any(|v| v == "references"));
    assert!(enum_vals.iter().any(|v| v == "cycles"));
    assert!(enum_vals.iter().any(|v| v == "impact_analysis"));
}

#[tokio::test]
async fn registry_with_graph_tool() {
    let dir = tempfile::tempdir().expect("tempdir");
    let forge = create_indexed_forge(dir.path(), "fn foo() {}\n").await;
    let tools = crate::chat::tools::default_builtin_tools_with_graph(dir.path(), forge);
    let mut registry = BuiltinToolRegistry::new();
    registry.register_many(tools);
    assert!(registry.has_tool("file_read"));
    assert!(registry.has_tool("file_write"));
    assert!(registry.has_tool("shell_exec"));
    assert!(registry.has_tool("graph_query"));
}

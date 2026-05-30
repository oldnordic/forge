use crate::chat::providers::mock::MockChatProvider;
use crate::chat::react::{AgentError, ReActLoop};
use crate::chat::tools::registry::BuiltinToolRegistry;
use crate::chat::tools::types::ToolDef;
use crate::llm::LlmConfig;
use async_trait::async_trait;
struct EchoTool;

#[async_trait]
impl crate::chat::tools::registry::AsyncTool for EchoTool {
    async fn call(&self, args: serde_json::Value) -> Result<String, String> {
        let msg = args["msg"].as_str().unwrap_or("");
        Ok(format!("Echo: {msg}"))
    }

    fn definition(&self) -> ToolDef {
        ToolDef::new(
            "echo",
            "Echo back the message",
            serde_json::json!({"type": "object", "properties": {"msg": {"type": "string"}}, "required": ["msg"]}),
        )
    }
}

fn echo_registry() -> BuiltinToolRegistry {
    let mut reg = BuiltinToolRegistry::new();
    reg.register(Box::new(EchoTool));
    reg
}

#[tokio::test]
async fn react_text_only_no_tools() {
    let provider = MockChatProvider::from_text("Hello world");
    let registry = BuiltinToolRegistry::new();
    let config = LlmConfig::new("test-model");

    let react = ReActLoop::new(Box::new(provider), registry, config);
    let result = react.run("Say hello").await;

    assert_eq!(result.unwrap(), "Hello world");
}

#[tokio::test]
async fn react_tool_call_then_answer() {
    let provider = MockChatProvider::from_text("final answer")
        .with_tool_call("echo", serde_json::json!({"msg": "hi"}));
    let registry = echo_registry();
    let config = LlmConfig::new("test-model");

    let react = ReActLoop::new(Box::new(provider), registry, config);
    let result = react.run("Echo hi").await;

    assert_eq!(result.unwrap(), "final answer");
}

#[tokio::test]
async fn react_multi_step_tool_calls() {
    let provider = MockChatProvider::from_text("done")
        .with_tool_call("echo", serde_json::json!({"msg": "step 1"}))
        .with_tool_call("echo", serde_json::json!({"msg": "step 2"}));
    let registry = echo_registry();
    let config = LlmConfig::new("test-model");

    let react = ReActLoop::new(Box::new(provider), registry, config);
    let result = react.run("Multi-step").await;

    assert_eq!(result.unwrap(), "done");
}

#[tokio::test]
async fn react_max_iterations_exceeded() {
    let provider = MockChatProvider::from_text("never finish")
        .with_tool_call("echo", serde_json::json!({"msg": "loop"}))
        .with_tool_call("echo", serde_json::json!({"msg": "loop"}))
        .with_tool_call("echo", serde_json::json!({"msg": "loop"}));
    let registry = echo_registry();
    let config = LlmConfig::new("test-model");

    let react = ReActLoop::new(Box::new(provider), registry, config).with_max_iterations(2);
    let result = react.run("Infinite loop").await;

    match result.unwrap_err() {
        AgentError::MaxIterations => {}
        other => panic!("expected MaxIterations, got {other}"),
    }
}

#[tokio::test]
async fn react_provider_error_propagates() {
    let provider = MockChatProvider::from_text("ok").with_error(
        crate::chat::types::LlmError::Http("connection failed".to_string()),
    );
    let registry = BuiltinToolRegistry::new();
    let config = LlmConfig::new("test-model");

    let react = ReActLoop::new(Box::new(provider), registry, config);
    let result = react.run("Test").await;

    match result.unwrap_err() {
        AgentError::Provider(err) => {
            assert!(err.to_string().contains("connection failed"));
        }
        other => panic!("expected Provider error, got {other}"),
    }
}

#[tokio::test]
async fn react_tool_error_feeds_back() {
    struct FailTool;

    #[async_trait]
    impl crate::chat::tools::registry::AsyncTool for FailTool {
        async fn call(&self, _args: serde_json::Value) -> Result<String, String> {
            Err("tool exploded".to_string())
        }

        fn definition(&self) -> ToolDef {
            ToolDef::empty("fail", "Always fails")
        }
    }

    let mut reg = BuiltinToolRegistry::new();
    reg.register(Box::new(FailTool));

    let provider = MockChatProvider::from_text("I see the error")
        .with_tool_call("fail", serde_json::json!({}));
    let config = LlmConfig::new("test-model");

    let react = ReActLoop::new(Box::new(provider), reg, config);
    let result = react.run("Try failing tool").await;

    assert_eq!(result.unwrap(), "I see the error");
}

#[tokio::test]
async fn react_custom_system_prompt() {
    let provider = MockChatProvider::from_text("custom response");
    let registry = BuiltinToolRegistry::new();
    let config = LlmConfig::new("test-model");

    let react = ReActLoop::new(Box::new(provider), registry, config)
        .with_system_prompt("You are a test assistant");
    let result = react.run("Test").await;

    assert_eq!(result.unwrap(), "custom response");
}

#[tokio::test]
async fn react_no_tool_calls_returns_immediately() {
    let provider = MockChatProvider::from_text("quick answer");
    let registry = echo_registry();
    let config = LlmConfig::new("test-model");

    let react = ReActLoop::new(Box::new(provider), registry, config);
    let result = react.run("Direct question").await;

    assert_eq!(result.unwrap(), "quick answer");
}

#[cfg(feature = "llm-ollama")]
#[tokio::test]
async fn react_live_ollama_tool_calling() {
    let client = reqwest::Client::new();
    let resp = client.get("http://localhost:11434/api/tags").send().await;
    if let Err(e) = &resp {
        eprintln!("Skipping live ReAct test — Ollama not available: {e}");
        return;
    }
    let resp = resp.expect("checked above");
    if !resp.status().is_success() {
        eprintln!("Skipping live ReAct test — Ollama tags endpoint failed");
        return;
    }
    let body = resp.text().await.unwrap_or_default();
    if !body.contains("qwen3.5-agent") {
        eprintln!("Skipping live ReAct test — qwen3.5-agent model not found");
        return;
    }

    use crate::chat::providers::ollama::OllamaChatProvider;
    use crate::chat::tools::registry::AsyncTool;

    struct LiveFileReadTool;

    #[async_trait]
    impl AsyncTool for LiveFileReadTool {
        async fn call(&self, args: serde_json::Value) -> Result<String, String> {
            let path = args["path"].as_str().unwrap_or("");
            if path.is_empty() {
                return Err("path is required".to_string());
            }
            tokio::fs::read_to_string(path)
                .await
                .map_err(|e| format!("Failed to read {}: {}", path, e))
        }

        fn definition(&self) -> ToolDef {
            ToolDef::new(
                "file_read",
                "Read the contents of a file at the given path",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "description": "Absolute or relative file path"}
                    },
                    "required": ["path"]
                }),
            )
        }
    }

    let mut registry = BuiltinToolRegistry::new();
    registry.register(Box::new(LiveFileReadTool));

    let provider = OllamaChatProvider::local();
    let config = LlmConfig::new("qwen3.5-agent:latest").with_temperature(0.1);

    let react = ReActLoop::new(Box::new(provider), registry, config)
        .with_max_iterations(10)
        .with_system_prompt("You are a helpful assistant. Use tools when asked. After getting tool results, give a brief text answer. Do not make additional tool calls after you have the answer.");

    let result = react
        .run("Read the file /home/feanor/Projects/forge/Cargo.toml and tell me what the workspace members are. Reply in one short sentence.")
        .await;

    match result {
        Ok(text) => {
            let lower = text.to_lowercase();
            assert!(
                lower.contains("forge") || lower.contains("member") || lower.contains("workspace"),
                "Expected response about workspace members, got: {text}"
            );
            eprintln!("Live ReAct SUCCESS: {text}");
        }
        Err(AgentError::MaxIterations) => {
            panic!("Live ReAct hit max iterations — tool results may not be reaching the model. Check convert_message for ToolResult handling.");
        }
        Err(e) => {
            panic!("Live ReAct unexpected error: {e}");
        }
    }
}

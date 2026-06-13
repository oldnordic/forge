use std::sync::Arc;

use crate::chat::providers::mock::MockChatProvider;
use crate::chat::react::{AgentError, ReActLoop};
use crate::chat::retrieval::{CodeRetriever, CodeSnippet, RetrievalSource};
use crate::chat::stream::ReactStreamEvent;
use crate::chat::tools::registry::BuiltinToolRegistry;
use crate::chat::tools::types::ToolDef;
use crate::llm::LlmConfig;
use async_trait::async_trait;
use futures::StreamExt;
use std::path::PathBuf;
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

    let react = ReActLoop::new(Arc::new(provider), registry, config);
    let result = react.run("Say hello").await;

    assert_eq!(result.unwrap(), "Hello world");
}

#[tokio::test]
async fn react_tool_call_then_answer() {
    let provider = MockChatProvider::from_text("final answer")
        .with_tool_call("echo", serde_json::json!({"msg": "hi"}));
    let registry = echo_registry();
    let config = LlmConfig::new("test-model");

    let react = ReActLoop::new(Arc::new(provider), registry, config);
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

    let react = ReActLoop::new(Arc::new(provider), registry, config);
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

    let react = ReActLoop::new(Arc::new(provider), registry, config).with_max_iterations(2);
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

    let react = ReActLoop::new(Arc::new(provider), registry, config).with_step_retries(0);
    let result = react.run("Test").await;

    match result.unwrap_err() {
        AgentError::Provider(err) => {
            assert!(err.to_string().contains("connection failed"));
        }
        other => panic!("expected Provider error, got {other}"),
    }
}

#[tokio::test]
async fn react_provider_error_retries_then_succeeds() {
    let provider = MockChatProvider::from_text("recovered")
        .with_error(crate::chat::types::LlmError::Http("transient".to_string()));
    let registry = BuiltinToolRegistry::new();
    let config = LlmConfig::new("test-model");

    let react = ReActLoop::new(Arc::new(provider), registry, config).with_step_retries(2);
    let result = react.run("Test retry").await;

    assert_eq!(result.unwrap(), "recovered");
}

#[tokio::test]
async fn react_provider_error_exhausts_retries() {
    let provider = MockChatProvider::from_text("never reached")
        .with_error(crate::chat::types::LlmError::Http("fail 1".to_string()))
        .with_error(crate::chat::types::LlmError::Http("fail 2".to_string()))
        .with_error(crate::chat::types::LlmError::Http("fail 3".to_string()));
    let registry = BuiltinToolRegistry::new();
    let config = LlmConfig::new("test-model");

    let react = ReActLoop::new(Arc::new(provider), registry, config).with_step_retries(2);
    let result = react.run("Test retry limit").await;

    match result.unwrap_err() {
        AgentError::Provider(err) => {
            assert!(err.to_string().contains("fail 3"));
        }
        other => panic!("expected Provider error after retries exhausted, got {other}"),
    }
}

#[tokio::test]
async fn react_verifier_rejects_answer() {
    let provider =
        MockChatProvider::from_text("bad answer").with_text("good answer with magic word");
    let registry = BuiltinToolRegistry::new();
    let config = LlmConfig::new("test-model");

    let verifier: crate::chat::react::VerifierFn = Arc::new(|answer| answer.contains("magic word"));

    let react = ReActLoop::new(Arc::new(provider), registry, config).with_verifier(verifier);
    let result = react.run("Tell me something").await;

    assert_eq!(result.unwrap(), "good answer with magic word");
}

#[tokio::test]
async fn react_verifier_rejects_all_hits_max_iterations() {
    let provider = MockChatProvider::from_text("always bad");
    let registry = BuiltinToolRegistry::new();
    let config = LlmConfig::new("test-model");

    let verifier: crate::chat::react::VerifierFn = Arc::new(|_| false);

    let react = ReActLoop::new(Arc::new(provider), registry, config)
        .with_verifier(verifier)
        .with_max_iterations(3);
    let result = react.run("Never passes").await;

    match result.unwrap_err() {
        AgentError::MaxIterations => {}
        other => panic!("expected MaxIterations, got {other}"),
    }
}

#[tokio::test]
async fn react_verifier_accepts_first_try() {
    let provider = MockChatProvider::from_text("perfect magic answer");
    let registry = BuiltinToolRegistry::new();
    let config = LlmConfig::new("test-model");

    let verifier: crate::chat::react::VerifierFn = Arc::new(|answer| answer.contains("magic"));

    let react = ReActLoop::new(Arc::new(provider), registry, config).with_verifier(verifier);
    let result = react.run("Say magic").await;

    assert_eq!(result.unwrap(), "perfect magic answer");
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

    let react = ReActLoop::new(Arc::new(provider), reg, config);
    let result = react.run("Try failing tool").await;

    assert_eq!(result.unwrap(), "I see the error");
}

#[tokio::test]
async fn react_custom_system_prompt() {
    let provider = MockChatProvider::from_text("custom response");
    let registry = BuiltinToolRegistry::new();
    let config = LlmConfig::new("test-model");

    let react = ReActLoop::new(Arc::new(provider), registry, config)
        .with_system_prompt("You are a test assistant");
    let result = react.run("Test").await;

    assert_eq!(result.unwrap(), "custom response");
}

#[tokio::test]
async fn react_no_tool_calls_returns_immediately() {
    let provider = MockChatProvider::from_text("quick answer");
    let registry = echo_registry();
    let config = LlmConfig::new("test-model");

    let react = ReActLoop::new(Arc::new(provider), registry, config);
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

    let react = ReActLoop::new(Arc::new(provider), registry, config)
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

#[tokio::test]
async fn react_stream_text_only() {
    let provider = MockChatProvider::from_text("Hello streaming");
    let registry = BuiltinToolRegistry::new();
    let config = LlmConfig::new("test-model");

    let react = ReActLoop::new(Arc::new(provider), registry, config);
    let events: Vec<ReactStreamEvent> = react.run_stream("Say hi").collect().await;

    let has_iteration = events
        .iter()
        .any(|e| matches!(e, ReactStreamEvent::IterationStart { .. }));
    assert!(has_iteration, "should have IterationStart event");

    let has_token = events.iter().any(|e| {
        matches!(
            e,
            ReactStreamEvent::LlmEvent(crate::chat::stream::StreamEvent::Token(_))
        )
    });
    assert!(has_token, "should have Token event");

    let answer = events.iter().find_map(|e| match e {
        ReactStreamEvent::Answer(a) => Some(a.clone()),
        _ => None,
    });
    assert_eq!(answer.as_deref(), Some("Hello streaming"));
}

#[tokio::test]
async fn react_stream_tool_call_then_answer() {
    let provider = MockChatProvider::from_text("final stream answer")
        .with_tool_call("echo", serde_json::json!({"msg": "hi"}));
    let registry = echo_registry();
    let config = LlmConfig::new("test-model");

    let react = ReActLoop::new(Arc::new(provider), registry, config);
    let events: Vec<ReactStreamEvent> = react.run_stream("Echo hi").collect().await;

    let tool_executed: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            ReactStreamEvent::ToolExecuted { name, success, .. } => Some((name.clone(), *success)),
            _ => None,
        })
        .collect();
    assert_eq!(tool_executed.len(), 1);
    assert_eq!(tool_executed[0].0, "echo");
    assert!(tool_executed[0].1);

    let answer = events.iter().find_map(|e| match e {
        ReactStreamEvent::Answer(a) => Some(a.clone()),
        _ => None,
    });
    assert_eq!(answer.as_deref(), Some("final stream answer"));
}

#[tokio::test]
async fn react_stream_max_iterations() {
    let provider = MockChatProvider::from_text("never finish")
        .with_tool_call("echo", serde_json::json!({"msg": "loop"}))
        .with_tool_call("echo", serde_json::json!({"msg": "loop"}))
        .with_tool_call("echo", serde_json::json!({"msg": "loop"}));
    let registry = echo_registry();
    let config = LlmConfig::new("test-model");

    let react = ReActLoop::new(Arc::new(provider), registry, config).with_max_iterations(2);
    let events: Vec<ReactStreamEvent> = react.run_stream("Loop").collect().await;

    let has_max = events
        .iter()
        .any(|e| matches!(e, ReactStreamEvent::MaxIterationsReached));
    assert!(has_max, "should emit MaxIterationsReached");

    let has_answer = events
        .iter()
        .any(|e| matches!(e, ReactStreamEvent::Answer(_)));
    assert!(
        !has_answer,
        "should not emit Answer when max iterations reached"
    );
}

#[tokio::test]
async fn react_stream_multiple_iterations() {
    let provider = MockChatProvider::from_text("done")
        .with_tool_call("echo", serde_json::json!({"msg": "step 1"}))
        .with_tool_call("echo", serde_json::json!({"msg": "step 2"}));
    let registry = echo_registry();
    let config = LlmConfig::new("test-model");

    let react = ReActLoop::new(Arc::new(provider), registry, config);
    let events: Vec<ReactStreamEvent> = react.run_stream("Multi-step").collect().await;

    let iterations: Vec<usize> = events
        .iter()
        .filter_map(|e| match e {
            ReactStreamEvent::IterationStart { iteration } => Some(*iteration),
            _ => None,
        })
        .collect();
    assert!(
        iterations.len() >= 3,
        "should have at least 3 iterations (2 tool calls + 1 final), got {}",
        iterations.len()
    );

    let tool_count = events
        .iter()
        .filter(|e| matches!(e, ReactStreamEvent::ToolExecuted { .. }))
        .count();
    assert_eq!(tool_count, 2, "should have 2 tool executions");

    let answer = events.iter().find_map(|e| match e {
        ReactStreamEvent::Answer(a) => Some(a.clone()),
        _ => None,
    });
    assert_eq!(answer.as_deref(), Some("done"));
}

struct FixedRetriever {
    snippets: Vec<CodeSnippet>,
}

#[async_trait]
impl CodeRetriever for FixedRetriever {
    async fn retrieve(&self, _query: &str, top_k: usize) -> Vec<CodeSnippet> {
        self.snippets.iter().take(top_k).cloned().collect()
    }
}

#[tokio::test]
async fn react_with_retriever_injects_context() {
    let snippets = vec![CodeSnippet {
        file: PathBuf::from("src/lib.rs"),
        line: 1,
        content: "pub fn hello() {}".to_string(),
        score: 0.9,
        source: RetrievalSource::File,
    }];
    let retriever = FixedRetriever { snippets };

    let provider = MockChatProvider::from_text("I see the hello function");
    let registry = BuiltinToolRegistry::new();
    let config = LlmConfig::new("test-model");

    let react =
        ReActLoop::new(Arc::new(provider), registry, config).with_retriever(Arc::new(retriever));
    let result = react.run("Find hello").await;

    assert_eq!(result.unwrap(), "I see the hello function");
}

#[tokio::test]
async fn react_with_empty_retriever_still_works() {
    let retriever = FixedRetriever { snippets: vec![] };

    let provider = MockChatProvider::from_text("no context needed");
    let registry = BuiltinToolRegistry::new();
    let config = LlmConfig::new("test-model");

    let react =
        ReActLoop::new(Arc::new(provider), registry, config).with_retriever(Arc::new(retriever));
    let result = react.run("Test").await;

    assert_eq!(result.unwrap(), "no context needed");
}

#[tokio::test]
async fn react_without_retriever_works() {
    let provider = MockChatProvider::from_text("no retrieval");
    let registry = BuiltinToolRegistry::new();
    let config = LlmConfig::new("test-model");

    let react = ReActLoop::new(Arc::new(provider), registry, config);
    let result = react.run("Test").await;

    assert_eq!(result.unwrap(), "no retrieval");
}

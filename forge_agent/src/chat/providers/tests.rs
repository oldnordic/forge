use crate::chat::providers::adapter::LlmProviderAdapter;
use crate::chat::providers::mock::MockChatProvider;
use crate::chat::providers::ChatProvider;
use crate::chat::stream::StreamEvent;
use crate::chat::tools::types::ToolDef;
use crate::chat::types::{ChatMessage, ContentBlock, LlmError, Role};
use crate::llm::{LlmConfig, LlmProvider};

#[tokio::test]
async fn mock_text_response() {
    let mock = MockChatProvider::from_text("Hello from mock");
    let config = LlmConfig::new("test-model");
    let messages = vec![ChatMessage::user("Hi")];

    let response = mock
        .chat(&messages, &[], &config)
        .await
        .expect("chat should succeed");

    assert_eq!(response.message.role, Role::Assistant);
    assert_eq!(response.message.text(), Some("Hello from mock"));
    assert!(!response.message.has_tool_calls());
    assert_eq!(response.model, "mock");
}

#[tokio::test]
async fn mock_tool_call_response() {
    let mock = MockChatProvider::from_text("default")
        .with_tool_call("file_read", serde_json::json!({"path": "main.rs"}));
    let config = LlmConfig::new("test-model");
    let messages = vec![ChatMessage::user("Read main.rs")];
    let tools = vec![ToolDef::new(
        "file_read",
        "Read a file",
        serde_json::json!({"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}),
    )];

    let response = mock
        .chat(&messages, &tools, &config)
        .await
        .expect("chat should succeed");

    assert!(response.message.has_tool_calls());
    let calls = response.message.tool_calls();
    assert_eq!(calls.len(), 1);
    match &calls[0] {
        ContentBlock::ToolCall {
            name, arguments, ..
        } => {
            assert_eq!(name, "file_read");
            assert_eq!(arguments["path"], "main.rs");
        }
        _ => panic!("expected ToolCall"),
    }
}

#[tokio::test]
async fn mock_sequential_responses() {
    let mock = MockChatProvider::from_text("final answer")
        .with_tool_call("echo", serde_json::json!({"msg": "hello"}))
        .with_text("Let me think...");
    let config = LlmConfig::new("test-model");
    let messages = vec![ChatMessage::user("test")];

    let r1 = mock.chat(&messages, &[], &config).await.expect("ok");
    assert!(r1.message.has_tool_calls());

    let r2 = mock.chat(&messages, &[], &config).await.expect("ok");
    assert_eq!(r2.message.text(), Some("Let me think..."));

    let r3 = mock.chat(&messages, &[], &config).await.expect("ok");
    assert_eq!(r3.message.text(), Some("final answer"));
}

#[tokio::test]
async fn mock_error_response() {
    let mock = MockChatProvider::from_text("ok").with_error(LlmError::ContextLengthExceeded);
    let config = LlmConfig::new("test-model");
    let messages = vec![ChatMessage::user("test")];

    let result = mock.chat(&messages, &[], &config).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        LlmError::ContextLengthExceeded => {}
        other => panic!("expected ContextLengthExceeded, got {other}"),
    }
}

struct SimpleMockLlm {
    response: String,
}

impl SimpleMockLlm {
    fn new(response: impl Into<String>) -> Self {
        SimpleMockLlm {
            response: response.into(),
        }
    }
}

#[async_trait::async_trait]
impl LlmProvider for SimpleMockLlm {
    async fn complete(&self, _prompt: &str, _system: Option<&str>) -> Result<String, String> {
        Ok(self.response.clone())
    }
}

#[tokio::test]
async fn adapter_converts_legacy_provider() {
    let legacy = std::sync::Arc::new(SimpleMockLlm::new("legacy response"));
    let adapter = LlmProviderAdapter::new(legacy);
    let config = LlmConfig::new("legacy-model");
    let messages = vec![
        ChatMessage::system("Be helpful"),
        ChatMessage::user("Hello"),
    ];

    let response = adapter
        .chat(&messages, &[], &config)
        .await
        .expect("adapter should succeed");

    assert_eq!(response.message.text(), Some("legacy response"));
    assert_eq!(response.model, "legacy-model");
}

#[tokio::test]
async fn adapter_extracts_system_prompt() {
    let legacy = std::sync::Arc::new(SimpleMockLlm::new("ok"));
    let adapter = LlmProviderAdapter::new(legacy);
    let config = LlmConfig::new("model");
    let messages = vec![
        ChatMessage::system("System instructions"),
        ChatMessage::user("User question"),
        ChatMessage::assistant("Previous answer"),
        ChatMessage::user("Follow up"),
    ];

    let response = adapter.chat(&messages, &[], &config).await;
    assert!(response.is_ok());
}

#[cfg(feature = "llm-ollama")]
#[tokio::test]
async fn ollama_chat_live_tool_calling() {
    let client = reqwest::Client::new();
    let resp = client.get("http://localhost:11434/api/tags").send().await;
    if let Err(e) = &resp {
        eprintln!("Skipping live Ollama test — server not available: {e}");
        return;
    }
    let resp = resp.expect("checked above");
    if !resp.status().is_success() {
        eprintln!("Skipping live Ollama test — tags endpoint failed");
        return;
    }

    let body = resp.text().await.unwrap_or_default();
    if !body.contains("qwen3.5-agent") {
        eprintln!("Skipping live Ollama test — qwen3.5-agent model not found");
        return;
    }

    let provider = crate::chat::providers::ollama::OllamaChatProvider::local();
    let config = LlmConfig::new("qwen3.5-agent:latest");
    let messages = vec![ChatMessage::user("Read the file hello.txt")];
    let tools = vec![ToolDef::new(
        "file_read",
        "Read a file",
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "File path"}
            },
            "required": ["path"]
        }),
    )];

    let response = provider
        .chat(&messages, &tools, &config)
        .await
        .expect("Ollama chat should succeed");

    assert!(response.message.has_tool_calls() || response.message.text().is_some());
    if response.message.has_tool_calls() {
        let calls = response.message.tool_calls();
        assert!(!calls.is_empty());
        match &calls[0] {
            ContentBlock::ToolCall { name, .. } => {
                assert_eq!(name, "file_read");
            }
            _ => panic!("expected ToolCall"),
        }
    }
}

#[cfg(feature = "llm-openai")]
#[tokio::test]
async fn openai_mock_text_response() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/chat/completions")
        .match_header("authorization", "Bearer test-key")
        .with_status(200)
        .with_body(
            serde_json::json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": "Hello from OpenAI"
                    },
                    "finish_reason": "stop"
                }],
                "usage": {
                    "prompt_tokens": 10,
                    "completion_tokens": 5,
                    "total_tokens": 15
                }
            })
            .to_string(),
        )
        .create_async()
        .await;

    let provider =
        crate::chat::providers::openai::OpenAiChatProvider::new(server.url(), "test-key");
    let config = LlmConfig::new("gpt-4");
    let messages = vec![ChatMessage::user("Hi")];

    let response = provider
        .chat(&messages, &[], &config)
        .await
        .expect("OpenAI chat should succeed");

    assert_eq!(response.message.text(), Some("Hello from OpenAI"));
    assert!(!response.message.has_tool_calls());
    assert_eq!(response.usage.prompt_tokens, Some(10));
    assert_eq!(response.usage.completion_tokens, Some(5));
    assert_eq!(response.finish_reason.as_deref(), Some("stop"));
    mock.assert_async().await;
}

#[cfg(feature = "llm-openai")]
#[tokio::test]
async fn openai_mock_tool_call_response() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_body(
            serde_json::json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": null,
                        "tool_calls": [{
                            "id": "call_abc123",
                            "type": "function",
                            "function": {
                                "name": "file_read",
                                "arguments": "{\"path\":\"main.rs\"}"
                            }
                        }]
                    },
                    "finish_reason": "tool_calls"
                }],
                "usage": {
                    "prompt_tokens": 20,
                    "completion_tokens": 10,
                    "total_tokens": 30
                }
            })
            .to_string(),
        )
        .create_async()
        .await;

    let provider =
        crate::chat::providers::openai::OpenAiChatProvider::new(server.url(), "test-key");
    let config = LlmConfig::new("gpt-4");
    let messages = vec![ChatMessage::user("Read main.rs")];
    let tools = vec![ToolDef::new(
        "file_read",
        "Read a file",
        serde_json::json!({"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}),
    )];

    let response = provider
        .chat(&messages, &tools, &config)
        .await
        .expect("OpenAI chat should succeed");

    assert!(response.message.has_tool_calls());
    let calls = response.message.tool_calls();
    assert_eq!(calls.len(), 1);
    match &calls[0] {
        ContentBlock::ToolCall {
            id,
            name,
            arguments,
        } => {
            assert_eq!(id, "call_abc123");
            assert_eq!(name, "file_read");
            assert_eq!(arguments["path"], "main.rs");
        }
        _ => panic!("expected ToolCall"),
    }
    assert_eq!(response.finish_reason.as_deref(), Some("tool_calls"));
    mock.assert_async().await;
}

#[cfg(feature = "llm-openai")]
#[tokio::test]
async fn openai_mock_rate_limited() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/chat/completions")
        .with_status(429)
        .with_body("rate limited")
        .create_async()
        .await;

    let provider =
        crate::chat::providers::openai::OpenAiChatProvider::new(server.url(), "test-key");
    let config = LlmConfig::new("gpt-4");
    let messages = vec![ChatMessage::user("Hi")];

    let result = provider.chat(&messages, &[], &config).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        LlmError::RateLimited { .. } => {}
        other => panic!("expected RateLimited, got {other}"),
    }
    mock.assert_async().await;
}

#[cfg(feature = "llm-anthropic")]
#[tokio::test]
async fn anthropic_mock_text_response() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/messages")
        .match_header("x-api-key", "sk-ant-test")
        .match_header("anthropic-version", "2023-06-01")
        .with_status(200)
        .with_body(
            serde_json::json!({
                "content": [{"type": "text", "text": "Hello from Claude"}],
                "stop_reason": "end_turn",
                "usage": {"input_tokens": 15, "output_tokens": 8}
            })
            .to_string(),
        )
        .create_async()
        .await;

    let provider = crate::chat::providers::anthropic::AnthropicChatProvider::new("sk-ant-test")
        .with_endpoint(server.url());
    let config = LlmConfig::new("claude-3-sonnet-20240229");
    let messages = vec![ChatMessage::user("Hi")];

    let response = provider
        .chat(&messages, &[], &config)
        .await
        .expect("Anthropic chat should succeed");

    assert_eq!(response.message.text(), Some("Hello from Claude"));
    assert!(!response.message.has_tool_calls());
    assert_eq!(response.usage.prompt_tokens, Some(15));
    assert_eq!(response.usage.completion_tokens, Some(8));
    assert_eq!(response.finish_reason.as_deref(), Some("end_turn"));
    mock.assert_async().await;
}

#[cfg(feature = "llm-anthropic")]
#[tokio::test]
async fn anthropic_mock_tool_call_response() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/messages")
        .with_status(200)
        .with_body(serde_json::json!({
            "content": [
                {"type": "text", "text": "I'll read that file."},
                {"type": "tool_use", "id": "toolu_xyz789", "name": "file_read", "input": {"path": "main.rs"}}
            ],
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 25, "output_tokens": 20}
        }).to_string())
        .create_async()
        .await;

    let provider = crate::chat::providers::anthropic::AnthropicChatProvider::new("sk-ant-test")
        .with_endpoint(server.url());
    let config = LlmConfig::new("claude-3-sonnet-20240229");
    let messages = vec![ChatMessage::user("Read main.rs")];
    let tools = vec![ToolDef::new(
        "file_read",
        "Read a file",
        serde_json::json!({"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}),
    )];

    let response = provider
        .chat(&messages, &tools, &config)
        .await
        .expect("Anthropic chat should succeed");

    assert!(response.message.has_tool_calls());
    assert_eq!(response.message.text(), Some("I'll read that file."));
    let calls = response.message.tool_calls();
    assert_eq!(calls.len(), 1);
    match &calls[0] {
        ContentBlock::ToolCall {
            id,
            name,
            arguments,
        } => {
            assert_eq!(id, "toolu_xyz789");
            assert_eq!(name, "file_read");
            assert_eq!(arguments["path"], "main.rs");
        }
        _ => panic!("expected ToolCall"),
    }
    assert_eq!(response.finish_reason.as_deref(), Some("tool_use"));
    mock.assert_async().await;
}

#[cfg(feature = "llm-ollama")]
#[tokio::test]
async fn ollama_mock_streaming_tokens() {
    use futures::StreamExt;

    let mut server = mockito::Server::new_async().await;
    let body = "{\"message\":{\"role\":\"assistant\",\"content\":\"Hello\"},\"done\":false}\n{\"message\":{\"role\":\"assistant\",\"content\":\" world\"},\"done\":false}\n{\"done\":true,\"prompt_eval_count\":10,\"eval_count\":5}\n";
    let mock = server
        .mock("POST", "/api/chat")
        .with_status(200)
        .with_header("content-type", "application/x-ndjson")
        .with_body(body)
        .create_async()
        .await;

    let provider = crate::chat::providers::ollama::OllamaChatProvider::new(server.url());
    let config = LlmConfig::new("qwen3.5-agent:latest");
    let messages = vec![ChatMessage::user("Hi")];

    let stream = provider.chat_stream(&messages, &[], &config);
    let events: Vec<StreamEvent> = stream.collect().await;

    assert!(events.len() >= 3);
    assert!(matches!(&events[0], StreamEvent::Token(t) if t == "Hello"));
    assert!(matches!(&events[1], StreamEvent::Token(t) if t == " world"));
    assert!(events.iter().any(|e| matches!(e, StreamEvent::Done)));
    mock.assert_async().await;
}

#[cfg(feature = "llm-ollama")]
#[tokio::test]
async fn ollama_mock_streaming_tool_call() {
    use futures::StreamExt;

    let mut server = mockito::Server::new_async().await;
    let body = "{\"message\":{\"role\":\"assistant\",\"content\":\"\",\"tool_calls\":[{\"function\":{\"name\":\"file_read\",\"arguments\":{\"path\":\"main.rs\"}}}]},\"done\":false}\n{\"done\":true,\"prompt_eval_count\":15,\"eval_count\":10}\n";
    let mock = server
        .mock("POST", "/api/chat")
        .with_status(200)
        .with_header("content-type", "application/x-ndjson")
        .with_body(body)
        .create_async()
        .await;

    let provider = crate::chat::providers::ollama::OllamaChatProvider::new(server.url());
    let config = LlmConfig::new("qwen3.5-agent:latest");
    let messages = vec![ChatMessage::user("Read main.rs")];
    let tools = vec![ToolDef::new(
        "file_read",
        "Read a file",
        serde_json::json!({"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}),
    )];

    let stream = provider.chat_stream(&messages, &tools, &config);
    let events: Vec<StreamEvent> = stream.collect().await;

    let tool_start = events
        .iter()
        .find(|e| matches!(e, StreamEvent::ToolCallStart { .. }));
    assert!(tool_start.is_some(), "expected ToolCallStart event");
    match tool_start.unwrap() {
        StreamEvent::ToolCallStart { name, .. } => {
            assert_eq!(name, "file_read");
        }
        _ => unreachable!(),
    }
    mock.assert_async().await;
}

#[cfg(feature = "llm-openai")]
#[tokio::test]
async fn openai_mock_streaming_tokens() {
    use futures::StreamExt;

    let mut server = mockito::Server::new_async().await;
    let body = "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"},\"finish_reason\":null}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\" world\"},\"finish_reason\":null}]}\n\ndata: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5,\"total_tokens\":15}}\n\ndata: [DONE]\n\n";
    let mock = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body(body)
        .create_async()
        .await;

    let provider =
        crate::chat::providers::openai::OpenAiChatProvider::new(server.url(), "test-key");
    let config = LlmConfig::new("gpt-4");
    let messages = vec![ChatMessage::user("Hi")];

    let stream = provider.chat_stream(&messages, &[], &config);
    let events: Vec<StreamEvent> = stream.collect().await;

    assert!(events.len() >= 3);
    assert!(matches!(&events[0], StreamEvent::Token(t) if t == "Hello"));
    assert!(matches!(&events[1], StreamEvent::Token(t) if t == " world"));
    assert!(events.iter().any(|e| matches!(e, StreamEvent::Done)));
    mock.assert_async().await;
}

#[cfg(feature = "llm-openai")]
#[tokio::test]
async fn openai_mock_streaming_tool_call() {
    use futures::StreamExt;

    let mut server = mockito::Server::new_async().await;
    let body = "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_abc\",\"type\":\"function\",\"function\":{\"name\":\"file_read\",\"arguments\":\"\"}}]},\"finish_reason\":null}]}\n\ndata: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"path\\\":\\\"main.rs\\\"}\"}}]},\"finish_reason\":null}]}\n\ndata: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\ndata: [DONE]\n\n";
    let mock = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body(body)
        .create_async()
        .await;

    let provider =
        crate::chat::providers::openai::OpenAiChatProvider::new(server.url(), "test-key");
    let config = LlmConfig::new("gpt-4");
    let messages = vec![ChatMessage::user("Read main.rs")];
    let tools = vec![ToolDef::new(
        "file_read",
        "Read a file",
        serde_json::json!({"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}),
    )];

    let stream = provider.chat_stream(&messages, &tools, &config);
    let events: Vec<StreamEvent> = stream.collect().await;

    let tool_start = events
        .iter()
        .find(|e| matches!(e, StreamEvent::ToolCallStart { .. }));
    assert!(tool_start.is_some(), "expected ToolCallStart event");
    match tool_start.unwrap() {
        StreamEvent::ToolCallStart { id, name, .. } => {
            assert_eq!(id, "call_abc");
            assert_eq!(name, "file_read");
        }
        _ => unreachable!(),
    }
    let arg_delta = events
        .iter()
        .find(|e| matches!(e, StreamEvent::ToolCallArgumentDelta { .. }));
    assert!(arg_delta.is_some(), "expected ToolCallArgumentDelta event");
    mock.assert_async().await;
}

#[cfg(feature = "llm-anthropic")]
#[tokio::test]
async fn anthropic_mock_streaming_tokens() {
    use futures::StreamExt;

    let mut server = mockito::Server::new_async().await;
    let body = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":10}}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\" world\"}}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":5}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
    let mock = server
        .mock("POST", "/v1/messages")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body(body)
        .create_async()
        .await;

    let provider = crate::chat::providers::anthropic::AnthropicChatProvider::new("sk-ant-test")
        .with_endpoint(server.url());
    let config = LlmConfig::new("claude-3-sonnet-20240229");
    let messages = vec![ChatMessage::user("Hi")];

    let stream = provider.chat_stream(&messages, &[], &config);
    let events: Vec<StreamEvent> = stream.collect().await;

    assert!(events.len() >= 3);
    assert!(matches!(&events[0], StreamEvent::Token(t) if t == "Hello"));
    assert!(matches!(&events[1], StreamEvent::Token(t) if t == " world"));
    assert!(events.iter().any(|e| matches!(e, StreamEvent::Done)));
    mock.assert_async().await;
}

#[cfg(feature = "llm-anthropic")]
#[tokio::test]
async fn anthropic_mock_streaming_tool_call() {
    use futures::StreamExt;

    let mut server = mockito::Server::new_async().await;
    let body = "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_xyz\",\"name\":\"file_read\",\"input\":{}}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"path\\\":\\\"main.rs\\\"}\"}}\n\nevent: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":1}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"},\"usage\":{\"output_tokens\":10}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
    let mock = server
        .mock("POST", "/v1/messages")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body(body)
        .create_async()
        .await;

    let provider = crate::chat::providers::anthropic::AnthropicChatProvider::new("sk-ant-test")
        .with_endpoint(server.url());
    let config = LlmConfig::new("claude-3-sonnet-20240229");
    let messages = vec![ChatMessage::user("Read main.rs")];
    let tools = vec![ToolDef::new(
        "file_read",
        "Read a file",
        serde_json::json!({"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}),
    )];

    let stream = provider.chat_stream(&messages, &tools, &config);
    let events: Vec<StreamEvent> = stream.collect().await;

    let tool_start = events
        .iter()
        .find(|e| matches!(e, StreamEvent::ToolCallStart { .. }));
    assert!(tool_start.is_some(), "expected ToolCallStart event");
    match tool_start.unwrap() {
        StreamEvent::ToolCallStart { id, name, .. } => {
            assert_eq!(id, "toolu_xyz");
            assert_eq!(name, "file_read");
        }
        _ => unreachable!(),
    }
    mock.assert_async().await;
}

#[tokio::test]
async fn mock_provider_default_stream_returns_error() {
    use futures::StreamExt;

    let mock = MockChatProvider::from_text("hello");
    let config = LlmConfig::new("test");
    let messages = vec![ChatMessage::user("Hi")];

    let stream = mock.chat_stream(&messages, &[], &config);
    let events: Vec<StreamEvent> = stream.collect().await;
    assert_eq!(events.len(), 1);
    assert!(
        matches!(&events[0], StreamEvent::Error(msg) if msg.contains("streaming not supported"))
    );
}

use crate::chat::types::{ChatMessage, ContentBlock, LlmError, Role, Usage};

#[test]
fn role_serialization_roundtrip() {
    let roles = vec![Role::System, Role::User, Role::Assistant, Role::Tool];
    for role in roles {
        let json = serde_json::to_string(&role).expect("serialize");
        let back: Role = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(role, back);
    }
}

#[test]
fn role_serializes_to_lowercase() {
    let json = serde_json::to_string(&Role::Assistant).expect("serialize");
    assert_eq!(json, "\"assistant\"");
}

#[test]
fn content_block_text_construction() {
    let block = ContentBlock::text("Hello");
    assert!(block.is_text());
    assert!(!block.is_tool_call());
    assert_eq!(
        block,
        ContentBlock::Text {
            text: "Hello".to_string()
        }
    );
}

#[test]
fn content_block_tool_call_construction() {
    let block = ContentBlock::tool_call("id_1", "file_read", serde_json::json!({"path": "a.rs"}));
    assert!(block.is_tool_call());
    assert!(!block.is_text());
    match &block {
        ContentBlock::ToolCall {
            id,
            name,
            arguments,
        } => {
            assert_eq!(id, "id_1");
            assert_eq!(name, "file_read");
            assert_eq!(arguments["path"], "a.rs");
        }
        _ => panic!("expected ToolCall"),
    }
}

#[test]
fn content_block_tool_result_construction() {
    let ok = ContentBlock::tool_result("id_1", "file contents");
    let err = ContentBlock::tool_error("id_1", "file not found");
    match &ok {
        ContentBlock::ToolResult {
            tool_call_id,
            content,
            is_error,
            ..
        } => {
            assert_eq!(tool_call_id, "id_1");
            assert_eq!(content, "file contents");
            assert!(!is_error);
        }
        _ => panic!("expected ToolResult"),
    }
    match &err {
        ContentBlock::ToolResult {
            is_error: true,
            content,
            ..
        } => {
            assert_eq!(content, "file not found");
        }
        _ => panic!("expected ToolResult with error"),
    }
}

#[test]
fn chat_message_user_construction() {
    let msg = ChatMessage::user("What does foo do?");
    assert_eq!(msg.role, Role::User);
    assert_eq!(msg.text(), Some("What does foo do?"));
    assert!(!msg.has_tool_calls());
}

#[test]
fn chat_message_system_construction() {
    let msg = ChatMessage::system("Be helpful");
    assert_eq!(msg.role, Role::System);
    assert_eq!(msg.text(), Some("Be helpful"));
}

#[test]
fn chat_message_with_tool_calls() {
    let msg = ChatMessage {
        role: Role::Assistant,
        content: vec![
            ContentBlock::text("I will read that file."),
            ContentBlock::tool_call("c1", "file_read", serde_json::json!({"path": "x.rs"})),
            ContentBlock::tool_call("c2", "file_read", serde_json::json!({"path": "y.rs"})),
        ],
    };
    assert!(msg.has_tool_calls());
    assert_eq!(msg.tool_calls().len(), 2);
    assert_eq!(msg.text(), Some("I will read that file."));
}

#[test]
fn chat_message_tool_result_construction() {
    let msg = ChatMessage::tool_result("c1", "fn main() {}");
    assert_eq!(msg.role, Role::Tool);
    assert!(msg.text().is_none());
}

#[test]
fn chat_message_tool_error_construction() {
    let msg = ChatMessage::tool_error("c1", "permission denied");
    assert_eq!(msg.role, Role::Tool);
    match &msg.content[0] {
        ContentBlock::ToolResult { is_error: true, .. } => {}
        _ => panic!("expected error ToolResult"),
    }
}

#[test]
fn chat_message_with_content_replaces_content() {
    let msg = ChatMessage::assistant("old").with_content(vec![ContentBlock::tool_call(
        "c1",
        "run",
        serde_json::json!({}),
    )]);
    assert_eq!(msg.content.len(), 1);
    assert!(msg.has_tool_calls());
    assert!(msg.text().is_none());
}

#[test]
fn usage_default_is_none() {
    let usage = Usage::default();
    assert!(usage.prompt_tokens.is_none());
    assert!(usage.completion_tokens.is_none());
    assert!(usage.total_tokens.is_none());
}

#[test]
fn content_block_serde_roundtrip() {
    let blocks = vec![
        ContentBlock::text("hello"),
        ContentBlock::tool_call("id1", "tool", serde_json::json!({"a": 1})),
        ContentBlock::tool_result("id1", "result"),
        ContentBlock::tool_error("id1", "failed"),
    ];
    for block in &blocks {
        let json = serde_json::to_string(block).expect("serialize");
        let back: ContentBlock = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(block, &back, "roundtrip failed for {json}");
    }
}

#[test]
fn chat_message_serde_roundtrip() {
    let msg = ChatMessage {
        role: Role::Assistant,
        content: vec![
            ContentBlock::text("Sure"),
            ContentBlock::tool_call("c1", "shell", serde_json::json!({"cmd": "ls"})),
        ],
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    let back: ChatMessage = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, back);
}

#[test]
fn llm_error_display() {
    assert_eq!(
        LlmError::Http("conn refused".into()).to_string(),
        "HTTP request failed: conn refused"
    );
    assert_eq!(
        LlmError::RateLimited {
            retry_after: Some(30)
        }
        .to_string(),
        "Rate limited (retry after Some(30)s)"
    );
    assert_eq!(
        LlmError::ContextLengthExceeded.to_string(),
        "Context length exceeded"
    );
}

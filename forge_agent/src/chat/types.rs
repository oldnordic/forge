use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ContentBlock {
    Text {
        text: String,
    },
    ToolCall {
        id: String,
        name: String,
        arguments: serde_json::Value,
    },
    ToolResult {
        tool_call_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        content: String,
        is_error: bool,
    },
}

impl ContentBlock {
    pub fn text(text: impl Into<String>) -> Self {
        ContentBlock::Text { text: text.into() }
    }

    pub fn tool_call(
        id: impl Into<String>,
        name: impl Into<String>,
        arguments: serde_json::Value,
    ) -> Self {
        ContentBlock::ToolCall {
            id: id.into(),
            name: name.into(),
            arguments,
        }
    }

    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        ContentBlock::ToolResult {
            tool_call_id: tool_call_id.into(),
            name: None,
            content: content.into(),
            is_error: false,
        }
    }

    pub fn tool_error(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        ContentBlock::ToolResult {
            tool_call_id: tool_call_id.into(),
            name: None,
            content: content.into(),
            is_error: true,
        }
    }

    pub fn is_tool_call(&self) -> bool {
        matches!(self, ContentBlock::ToolCall { .. })
    }

    pub fn is_text(&self) -> bool {
        matches!(self, ContentBlock::Text { .. })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ChatMessage {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}

impl ChatMessage {
    pub fn system(text: impl Into<String>) -> Self {
        ChatMessage {
            role: Role::System,
            content: vec![ContentBlock::text(text)],
        }
    }

    pub fn user(text: impl Into<String>) -> Self {
        ChatMessage {
            role: Role::User,
            content: vec![ContentBlock::text(text)],
        }
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        ChatMessage {
            role: Role::Assistant,
            content: vec![ContentBlock::text(text)],
        }
    }

    pub fn tool_result(tool_call_id: impl Into<String>, output: impl Into<String>) -> Self {
        ChatMessage {
            role: Role::Tool,
            content: vec![ContentBlock::tool_result(tool_call_id, output)],
        }
    }

    pub fn tool_error(tool_call_id: impl Into<String>, error: impl Into<String>) -> Self {
        ChatMessage {
            role: Role::Tool,
            content: vec![ContentBlock::tool_error(tool_call_id, error)],
        }
    }

    pub fn with_content(mut self, blocks: Vec<ContentBlock>) -> Self {
        self.content = blocks;
        self
    }

    pub fn text(&self) -> Option<&str> {
        self.content.iter().find_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
    }

    pub fn tool_calls(&self) -> Vec<&ContentBlock> {
        self.content.iter().filter(|b| b.is_tool_call()).collect()
    }

    pub fn has_tool_calls(&self) -> bool {
        self.content.iter().any(|b| b.is_tool_call())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Usage {
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ChatResponse {
    pub message: ChatMessage,
    pub usage: Usage,
    pub model: String,
    pub finish_reason: Option<String>,
}

impl ChatResponse {
    pub fn new(message: ChatMessage, usage: Usage, model: impl Into<String>) -> Self {
        ChatResponse {
            message,
            usage,
            model: model.into(),
            finish_reason: None,
        }
    }

    pub fn with_finish_reason(mut self, reason: impl Into<String>) -> Self {
        self.finish_reason = Some(reason.into());
        self
    }
}

#[derive(Clone, Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LlmError {
    #[error("HTTP request failed: {0}")]
    Http(String),

    #[error("Failed to parse response: {0}")]
    Parse(String),

    #[error("Rate limited (retry after {retry_after:?}s)")]
    RateLimited { retry_after: Option<u64> },

    #[error("Context length exceeded")]
    ContextLengthExceeded,

    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Provider error: {0}")]
    Provider(String),
}

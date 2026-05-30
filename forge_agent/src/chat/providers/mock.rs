use crate::chat::providers::ChatProvider;
use crate::chat::tools::types::ToolDef;
use crate::chat::types::{ChatMessage, ChatResponse, ContentBlock, LlmError, Usage};
use crate::llm::LlmConfig;
use async_trait::async_trait;
use std::sync::Mutex;

enum MockResponse {
    Text(String),
    ToolCalls(Vec<(String, serde_json::Value)>),
    Error(LlmError),
}

pub struct MockChatProvider {
    responses: Mutex<Vec<MockResponse>>,
    default_text: String,
}

impl MockChatProvider {
    pub fn from_text(text: impl Into<String>) -> Self {
        MockChatProvider {
            responses: Mutex::new(Vec::new()),
            default_text: text.into(),
        }
    }

    pub fn with_tool_call(self, name: impl Into<String>, args: serde_json::Value) -> Self {
        self.responses
            .lock()
            .expect("invariant: mock lock")
            .push(MockResponse::ToolCalls(vec![(name.into(), args)]));
        self
    }

    pub fn with_text(self, text: impl Into<String>) -> Self {
        self.responses
            .lock()
            .expect("invariant: mock lock")
            .push(MockResponse::Text(text.into()));
        self
    }

    pub fn with_error(self, error: LlmError) -> Self {
        self.responses
            .lock()
            .expect("invariant: mock lock")
            .push(MockResponse::Error(error));
        self
    }

    fn next_response(&self) -> MockResponse {
        let mut responses = self.responses.lock().expect("invariant: mock lock");
        if responses.is_empty() {
            MockResponse::Text(self.default_text.clone())
        } else {
            responses.remove(0)
        }
    }
}

#[async_trait]
impl ChatProvider for MockChatProvider {
    async fn chat(
        &self,
        _messages: &[ChatMessage],
        _tools: &[ToolDef],
        _config: &LlmConfig,
    ) -> Result<ChatResponse, LlmError> {
        match self.next_response() {
            MockResponse::Text(text) => Ok(ChatResponse {
                message: ChatMessage::assistant(text),
                usage: Usage::default(),
                model: "mock".to_string(),
                finish_reason: Some("stop".to_string()),
            }),
            MockResponse::ToolCalls(calls) => {
                let mut call_index = 0u32;
                let content: Vec<ContentBlock> = calls
                    .into_iter()
                    .map(|(name, args)| {
                        let id = format!("mock_call_{}", call_index);
                        call_index += 1;
                        ContentBlock::tool_call(id, name, args)
                    })
                    .collect();
                Ok(ChatResponse {
                    message: ChatMessage {
                        role: crate::chat::types::Role::Assistant,
                        content,
                    },
                    usage: Usage::default(),
                    model: "mock".to_string(),
                    finish_reason: Some("tool_calls".to_string()),
                })
            }
            MockResponse::Error(err) => Err(err),
        }
    }
}

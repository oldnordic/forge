use crate::chat::providers::ChatProvider;
use crate::chat::tools::types::ToolDef;
use crate::chat::types::{ChatMessage, ChatResponse, LlmError, Role, Usage};
use crate::llm::{LlmConfig, LlmProvider};
use async_trait::async_trait;
use std::sync::Arc;

pub struct LlmProviderAdapter {
    inner: Arc<dyn LlmProvider>,
}

impl LlmProviderAdapter {
    pub fn new(provider: Arc<dyn LlmProvider>) -> Self {
        LlmProviderAdapter { inner: provider }
    }
}

#[async_trait]
impl ChatProvider for LlmProviderAdapter {
    async fn chat(
        &self,
        messages: &[ChatMessage],
        _tools: &[ToolDef],
        _config: &LlmConfig,
    ) -> Result<ChatResponse, LlmError> {
        let mut system_text = String::new();
        let mut user_text = String::new();

        for msg in messages {
            match msg.role {
                Role::System => {
                    if let Some(t) = msg.text() {
                        if !system_text.is_empty() {
                            system_text.push('\n');
                        }
                        system_text.push_str(t);
                    }
                }
                Role::User => {
                    if let Some(t) = msg.text() {
                        if !user_text.is_empty() {
                            user_text.push('\n');
                        }
                        user_text.push_str(t);
                    }
                }
                _ => {}
            }
        }

        let system = if system_text.is_empty() {
            None
        } else {
            Some(system_text)
        };

        self.inner
            .complete(&user_text, system.as_deref())
            .await
            .map(|text| ChatResponse {
                message: ChatMessage::assistant(text),
                usage: Usage::default(),
                model: "legacy".to_string(),
                finish_reason: Some("stop".to_string()),
            })
            .map_err(LlmError::Provider)
    }
}

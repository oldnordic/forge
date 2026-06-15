use crate::chat::providers::ChatProvider;
use crate::chat::tools::types::ToolDef;
use crate::chat::types::{ChatMessage, ChatResponse, ContentBlock, LlmError, Role, Usage};
use crate::llm::{LlmConfig, LlmProvider};
use async_trait::async_trait;
use std::sync::Arc;

/// Bridges legacy [`LlmProvider`] (text-only, single-turn) to [`ChatProvider`].
///
/// # Limitations
///
/// This adapter cannot support tool calling or multi-turn conversations because
/// the underlying `LlmProvider` trait only accepts a flat prompt string. Tool
/// definitions, assistant messages, and tool-result messages are ignored. Usage
/// data is unavailable.
///
/// For tool-calling agents, use a native `ChatProvider` implementation
/// (`OllamaChatProvider`, `OpenAiChatProvider`, `AnthropicChatProvider`).
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
        tools: &[ToolDef],
        config: &LlmConfig,
    ) -> Result<ChatResponse, LlmError> {
        if !tools.is_empty() {
            return Err(LlmError::Provider(
                "LlmProviderAdapter does not support tool calling; use a native ChatProvider"
                    .into(),
            ));
        }

        let mut system_text = String::new();
        let mut user_parts: Vec<String> = Vec::new();

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
                        user_parts.push(t.to_string());
                    }
                }
                Role::Assistant => {
                    for block in &msg.content {
                        if let ContentBlock::Text { text } = block {
                            user_parts.push(format!("[Assistant]: {text}"));
                        }
                    }
                }
                Role::Tool => {
                    for block in &msg.content {
                        if let ContentBlock::ToolResult {
                            content, is_error, ..
                        } = block
                        {
                            let label = if *is_error {
                                "[Tool Error]"
                            } else {
                                "[Tool Result]"
                            };
                            user_parts.push(format!("{label}: {content}"));
                        }
                    }
                }
            }
        }

        let user_text = user_parts.join("\n");
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
                model: config.model.clone(),
                finish_reason: Some("stop".to_string()),
            })
            .map_err(LlmError::Provider)
    }
}

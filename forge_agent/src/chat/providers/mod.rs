pub mod adapter;
pub mod mock;
#[cfg(any(
    feature = "llm-ollama",
    feature = "llm-openai",
    feature = "llm-anthropic"
))]
pub(crate) mod ndjson_stream;
pub mod retry;

pub use adapter::LlmProviderAdapter;
pub use mock::MockChatProvider;
pub use retry::{ContextTrimmer, RetryProvider};

use crate::chat::stream::StreamEvent;
use crate::chat::tools::types::ToolDef;
use crate::chat::types::{ChatMessage, ChatResponse, LlmError};
use crate::llm::LlmConfig;
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

/// Provider-agnostic chat completion trait.
///
/// Implement this trait to add a new LLM backend to the agent.
///
/// ## Stability
///
/// This trait is part of the stable SDK contract. Breaking changes to the
/// signature will be accompanied by a major version bump.
#[async_trait]
pub trait ChatProvider: Send + Sync {
    async fn chat(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDef],
        config: &LlmConfig,
    ) -> Result<ChatResponse, LlmError>;

    fn chat_stream(
        &self,
        _messages: &[ChatMessage],
        _tools: &[ToolDef],
        _config: &LlmConfig,
    ) -> Pin<Box<dyn Stream<Item = StreamEvent> + Send>> {
        Box::pin(futures::stream::once(async {
            StreamEvent::Error("streaming not supported by this provider".to_string())
        }))
    }
}

pub async fn chat_structured<T: serde::de::DeserializeOwned>(
    provider: &dyn ChatProvider,
    messages: &[ChatMessage],
    config: &LlmConfig,
) -> Result<T, LlmError> {
    let response = provider.chat(messages, &[], config).await?;
    let text = response.message.text().unwrap_or_default();
    let trimmed = text.trim();
    let json_str = if trimmed.starts_with("```") {
        let inner = trimmed
            .trim_start_matches("```")
            .trim_start_matches("json")
            .trim_start_matches("JSON");
        inner.trim_end_matches("```").trim()
    } else {
        trimmed
    };
    serde_json::from_str(json_str)
        .map_err(|e| LlmError::Parse(format!("structured output parse error: {e}")))
}

#[cfg(feature = "llm-ollama")]
pub mod ollama;

#[cfg(feature = "llm-ollama")]
pub use ollama::OllamaChatProvider;

#[cfg(feature = "llm-openai")]
pub mod openai;

#[cfg(feature = "llm-openai")]
pub use openai::OpenAiChatProvider;

#[cfg(feature = "llm-anthropic")]
pub mod anthropic;

#[cfg(feature = "llm-anthropic")]
pub use anthropic::AnthropicChatProvider;

#[cfg(test)]
mod tests;

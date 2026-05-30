pub mod adapter;
pub mod mock;
pub mod retry;

pub use adapter::LlmProviderAdapter;
pub use mock::MockChatProvider;
pub use retry::RetryProvider;

use crate::chat::stream::StreamEvent;
use crate::chat::tools::types::ToolDef;
use crate::chat::types::{ChatMessage, ChatResponse, LlmError};
use crate::llm::LlmConfig;
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

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

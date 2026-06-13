use crate::chat::providers::ChatProvider;
use crate::chat::stream::StreamEvent;
use crate::chat::tools::types::ToolDef;
use crate::chat::types::{ChatMessage, ChatResponse, LlmError};
use crate::llm::LlmConfig;
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

pub type ContextTrimmer = Arc<dyn Fn(&[ChatMessage]) -> Vec<ChatMessage> + Send + Sync>;

pub struct RetryProvider {
    inner: Box<dyn ChatProvider>,
    max_retries: u32,
    base_delay: Duration,
    context_trimmer: Option<ContextTrimmer>,
}

impl RetryProvider {
    pub fn new(inner: Box<dyn ChatProvider>, max_retries: u32) -> Self {
        RetryProvider {
            inner,
            max_retries,
            base_delay: Duration::from_secs(1),
            context_trimmer: None,
        }
    }

    pub fn with_base_delay(mut self, delay: Duration) -> Self {
        self.base_delay = delay;
        self
    }

    pub fn with_context_trimmer(mut self, trimmer: ContextTrimmer) -> Self {
        self.context_trimmer = Some(trimmer);
        self
    }

    fn default_trim(messages: &[ChatMessage]) -> Vec<ChatMessage> {
        if messages.len() <= 2 {
            return messages.to_vec();
        }
        let mut trimmed = messages.to_vec();
        let system_count = trimmed
            .iter()
            .take_while(|m| matches!(m.role, crate::chat::types::Role::System))
            .count();
        if system_count < trimmed.len() - 1 {
            trimmed.remove(system_count);
        }
        trimmed
    }
}

#[async_trait]
impl ChatProvider for RetryProvider {
    async fn chat(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDef],
        config: &LlmConfig,
    ) -> Result<ChatResponse, LlmError> {
        let mut last_error = None;
        let mut current_messages = messages.to_vec();

        for attempt in 0..=self.max_retries {
            match self.inner.chat(&current_messages, tools, config).await {
                Ok(response) => return Ok(response),
                Err(LlmError::RateLimited { retry_after }) => {
                    if attempt >= self.max_retries {
                        return Err(LlmError::RateLimited { retry_after });
                    }
                    let delay = retry_after
                        .map(Duration::from_secs)
                        .unwrap_or_else(|| self.base_delay * 2u32.saturating_pow(attempt));
                    sleep(delay).await;
                }
                Err(LlmError::ContextLengthExceeded) => {
                    if attempt >= self.max_retries {
                        return Err(LlmError::ContextLengthExceeded);
                    }
                    let trimmed = match self.context_trimmer {
                        Some(ref trimmer) => trimmer(&current_messages),
                        None => Self::default_trim(&current_messages),
                    };
                    if trimmed.len() == current_messages.len() {
                        return Err(LlmError::ContextLengthExceeded);
                    }
                    current_messages = trimmed;
                }
                Err(LlmError::Http(msg)) => {
                    if attempt >= self.max_retries {
                        return Err(LlmError::Http(msg));
                    }
                    if msg.contains("connection") || msg.contains("timeout") {
                        let delay = self.base_delay * 2u32.saturating_pow(attempt);
                        sleep(delay).await;
                    } else {
                        return Err(LlmError::Http(msg));
                    }
                }
                Err(e) => {
                    last_error = Some(e);
                    break;
                }
            }
        }

        Err(last_error.unwrap_or(LlmError::Provider("retry exhausted".to_string())))
    }

    fn chat_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDef],
        config: &LlmConfig,
    ) -> Pin<Box<dyn Stream<Item = StreamEvent> + Send>> {
        self.inner.chat_stream(messages, tools, config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::providers::mock::MockChatProvider;

    #[tokio::test]
    async fn retry_succeeds_after_rate_limit() {
        let provider = MockChatProvider::from_text("success")
            .with_error(LlmError::RateLimited { retry_after: None });
        let retry = RetryProvider::new(Box::new(provider), 2);
        let config = LlmConfig::new("test");
        let messages = vec![ChatMessage::user("hi")];

        let result = retry.chat(&messages, &[], &config).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().message.text(), Some("success"));
    }

    #[tokio::test]
    async fn retry_exhausted_returns_rate_limit() {
        let provider = MockChatProvider::from_text("never reached")
            .with_error(LlmError::RateLimited { retry_after: None })
            .with_error(LlmError::RateLimited { retry_after: None });
        let retry =
            RetryProvider::new(Box::new(provider), 1).with_base_delay(Duration::from_millis(10));
        let config = LlmConfig::new("test");
        let messages = vec![ChatMessage::user("hi")];

        let result = retry.chat(&messages, &[], &config).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            LlmError::RateLimited { .. } => {}
            other => panic!("expected RateLimited, got {other}"),
        }
    }

    #[tokio::test]
    async fn retry_no_retry_on_parse_error() {
        let provider =
            MockChatProvider::from_text("ok").with_error(LlmError::Parse("bad json".to_string()));
        let retry = RetryProvider::new(Box::new(provider), 3);
        let config = LlmConfig::new("test");
        let messages = vec![ChatMessage::user("hi")];

        let result = retry.chat(&messages, &[], &config).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            LlmError::Parse(msg) => assert_eq!(msg, "bad json"),
            other => panic!("expected Parse, got {other}"),
        }
    }

    #[tokio::test]
    async fn retry_succeeds_first_try() {
        let provider = MockChatProvider::from_text("immediate");
        let retry = RetryProvider::new(Box::new(provider), 3);
        let config = LlmConfig::new("test");
        let messages = vec![ChatMessage::user("hi")];

        let result = retry.chat(&messages, &[], &config).await;
        assert_eq!(result.unwrap().message.text(), Some("immediate"));
    }

    #[tokio::test]
    async fn retry_context_length_trims_and_retries() {
        let provider =
            MockChatProvider::from_text("trimmed ok").with_error(LlmError::ContextLengthExceeded);
        let retry =
            RetryProvider::new(Box::new(provider), 2).with_base_delay(Duration::from_millis(10));
        let config = LlmConfig::new("test");
        let messages = vec![
            ChatMessage::system("you are helpful"),
            ChatMessage::user("msg 1"),
            ChatMessage::assistant("reply 1"),
            ChatMessage::user("msg 2"),
        ];

        let result = retry.chat(&messages, &[], &config).await;
        assert_eq!(result.unwrap().message.text(), Some("trimmed ok"));
    }

    #[tokio::test]
    async fn retry_context_length_cannot_trim_fails() {
        let provider =
            MockChatProvider::from_text("never").with_error(LlmError::ContextLengthExceeded);
        let retry =
            RetryProvider::new(Box::new(provider), 2).with_base_delay(Duration::from_millis(10));
        let config = LlmConfig::new("test");
        let messages = vec![ChatMessage::user("only message")];

        let result = retry.chat(&messages, &[], &config).await;
        match result.unwrap_err() {
            LlmError::ContextLengthExceeded => {}
            other => panic!("expected ContextLengthExceeded, got {other}"),
        }
    }

    #[tokio::test]
    async fn retry_custom_context_trimmer() {
        let provider = MockChatProvider::from_text("custom trimmed")
            .with_error(LlmError::ContextLengthExceeded);
        let trimmer: ContextTrimmer = Arc::new(|msgs| {
            msgs.iter()
                .filter(|m| !matches!(m.role, crate::chat::types::Role::Assistant))
                .cloned()
                .collect()
        });
        let retry = RetryProvider::new(Box::new(provider), 2)
            .with_base_delay(Duration::from_millis(10))
            .with_context_trimmer(trimmer);
        let config = LlmConfig::new("test");
        let messages = vec![
            ChatMessage::system("sys"),
            ChatMessage::user("q1"),
            ChatMessage::assistant("a1"),
            ChatMessage::user("q2"),
        ];

        let result = retry.chat(&messages, &[], &config).await;
        assert_eq!(result.unwrap().message.text(), Some("custom trimmed"));
    }

    #[tokio::test]
    async fn retry_connection_error_with_backoff() {
        let provider = MockChatProvider::from_text("recovered")
            .with_error(LlmError::Http("connection timeout".to_string()));
        let retry =
            RetryProvider::new(Box::new(provider), 2).with_base_delay(Duration::from_millis(10));
        let config = LlmConfig::new("test");
        let messages = vec![ChatMessage::user("hi")];

        let result = retry.chat(&messages, &[], &config).await;
        assert_eq!(result.unwrap().message.text(), Some("recovered"));
    }

    #[tokio::test]
    async fn retry_non_retryable_http_fails_immediately() {
        let provider = MockChatProvider::from_text("never")
            .with_error(LlmError::Http("400 bad request".to_string()));
        let retry =
            RetryProvider::new(Box::new(provider), 5).with_base_delay(Duration::from_millis(10));
        let config = LlmConfig::new("test");
        let messages = vec![ChatMessage::user("hi")];

        let result = retry.chat(&messages, &[], &config).await;
        match result.unwrap_err() {
            LlmError::Http(msg) => assert!(msg.contains("400")),
            other => panic!("expected Http, got {other}"),
        }
    }
}

use crate::chat::providers::ChatProvider;
use crate::chat::stream::StreamEvent;
use crate::chat::tools::types::ToolDef;
use crate::chat::types::{ChatMessage, ChatResponse, LlmError};
use crate::llm::LlmConfig;
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use std::time::Duration;
use tokio::time::sleep;

pub struct RetryProvider {
    inner: Box<dyn ChatProvider>,
    max_retries: u32,
    base_delay: Duration,
}

impl RetryProvider {
    pub fn new(inner: Box<dyn ChatProvider>, max_retries: u32) -> Self {
        RetryProvider {
            inner,
            max_retries,
            base_delay: Duration::from_secs(1),
        }
    }

    pub fn with_base_delay(mut self, delay: Duration) -> Self {
        self.base_delay = delay;
        self
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

        for attempt in 0..=self.max_retries {
            match self.inner.chat(messages, tools, config).await {
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
}

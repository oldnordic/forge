//! Token usage tracker for cumulative token accounting.
//!
//! Subscribes to `AgentEvent::LlmResponseReceived` events on the `EventBus`
//! and maintains a running total of prompt, completion, and total tokens.

use std::sync::Arc;

use crate::chat::events::{AgentEvent, EventBus};

#[derive(Clone, Debug, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub llm_calls: u64,
}

pub struct TokenTracker {
    usage: Arc<std::sync::Mutex<TokenUsage>>,
}

impl std::fmt::Debug for TokenTracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenTracker").finish_non_exhaustive()
    }
}

impl Clone for TokenTracker {
    fn clone(&self) -> Self {
        TokenTracker {
            usage: Arc::clone(&self.usage),
        }
    }
}

impl TokenTracker {
    pub fn new() -> Self {
        TokenTracker {
            usage: Arc::new(std::sync::Mutex::new(TokenUsage::default())),
        }
    }

    pub async fn attach(&self, bus: &EventBus) {
        let usage = Arc::clone(&self.usage);
        bus.subscribe(move |event| {
            if let AgentEvent::LlmResponseReceived { usage: Some(u), .. } = event {
                let mut guard = usage.lock().expect("invariant: token tracker lock");
                guard.prompt_tokens += u.prompt_tokens.unwrap_or(0);
                guard.completion_tokens += u.completion_tokens.unwrap_or(0);
                guard.total_tokens += u.total_tokens.unwrap_or(0);
                guard.llm_calls += 1;
            }
        })
        .await;
    }

    pub async fn usage(&self) -> TokenUsage {
        self.usage
            .lock()
            .expect("invariant: token tracker lock")
            .clone()
    }

    pub async fn total_tokens(&self) -> u64 {
        self.usage
            .lock()
            .expect("invariant: token tracker lock")
            .total_tokens
    }

    pub async fn llm_calls(&self) -> u64 {
        self.usage
            .lock()
            .expect("invariant: token tracker lock")
            .llm_calls
    }
}

impl Default for TokenTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::types::Usage;

    #[tokio::test]
    async fn tracker_starts_zero() {
        let tracker = TokenTracker::new();
        let usage = tracker.usage().await;
        assert_eq!(usage.prompt_tokens, 0);
        assert_eq!(usage.completion_tokens, 0);
        assert_eq!(usage.total_tokens, 0);
        assert_eq!(usage.llm_calls, 0);
    }

    #[tokio::test]
    async fn attach_and_accumulate() {
        let bus = EventBus::new();
        let tracker = TokenTracker::new();
        tracker.attach(&bus).await;

        bus.emit(&AgentEvent::LlmResponseReceived {
            iteration: 0,
            usage: Some(Usage {
                prompt_tokens: Some(100),
                completion_tokens: Some(50),
                total_tokens: Some(150),
            }),
            has_tool_calls: false,
        })
        .await;

        bus.emit(&AgentEvent::LlmResponseReceived {
            iteration: 1,
            usage: Some(Usage {
                prompt_tokens: Some(200),
                completion_tokens: Some(75),
                total_tokens: Some(275),
            }),
            has_tool_calls: true,
        })
        .await;

        let usage = tracker.usage().await;
        assert_eq!(usage.prompt_tokens, 300);
        assert_eq!(usage.completion_tokens, 125);
        assert_eq!(usage.total_tokens, 425);
        assert_eq!(usage.llm_calls, 2);
    }

    #[tokio::test]
    async fn ignores_none_usage() {
        let bus = EventBus::new();
        let tracker = TokenTracker::new();
        tracker.attach(&bus).await;

        bus.emit(&AgentEvent::LlmResponseReceived {
            iteration: 0,
            usage: None,
            has_tool_calls: false,
        })
        .await;

        assert_eq!(tracker.total_tokens().await, 0);
        assert_eq!(tracker.llm_calls().await, 0);
    }

    #[tokio::test]
    async fn ignores_non_llm_events() {
        let bus = EventBus::new();
        let tracker = TokenTracker::new();
        tracker.attach(&bus).await;

        bus.emit(&AgentEvent::ToolCallStarted {
            iteration: 0,
            tool_name: "file_read".to_string(),
            tool_call_id: "tc_1".to_string(),
        })
        .await;

        assert_eq!(tracker.total_tokens().await, 0);
    }

    #[tokio::test]
    async fn clone_shares_state() {
        let bus = EventBus::new();
        let tracker = TokenTracker::new();
        tracker.attach(&bus).await;

        let tracker2 = tracker.clone();
        bus.emit(&AgentEvent::LlmResponseReceived {
            iteration: 0,
            usage: Some(Usage {
                prompt_tokens: Some(50),
                completion_tokens: Some(25),
                total_tokens: Some(75),
            }),
            has_tool_calls: false,
        })
        .await;

        assert_eq!(tracker2.total_tokens().await, 75);
    }
}

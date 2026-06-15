//! Event bus for agent lifecycle observability.
//!
//! Provides a typed event system that external code can subscribe to for
//! monitoring agent execution without modifying the agent loop itself.

use std::sync::Arc;

use tokio::sync::RwLock;

use crate::chat::types::Usage;

#[derive(Clone, Debug)]
pub enum AgentEvent {
    SessionStarted {
        session_id: String,
    },
    IterationStarted {
        iteration: usize,
        max_iterations: usize,
    },
    LlmResponseReceived {
        iteration: usize,
        usage: Option<Usage>,
        has_tool_calls: bool,
    },
    LlmError {
        iteration: usize,
        consecutive_errors: usize,
        error: String,
    },
    ToolCallStarted {
        iteration: usize,
        tool_name: String,
        tool_call_id: String,
    },
    ToolCallCompleted {
        iteration: usize,
        tool_name: String,
        tool_call_id: String,
        success: bool,
        output_bytes: usize,
        truncated: bool,
    },
    RetrievalInjected {
        num_snippets: usize,
    },
    VerificationFailed {
        iteration: usize,
    },
    AnswerProduced {
        iteration: usize,
        answer_length: usize,
    },
    MaxIterationsReached {
        iterations: usize,
    },
}

type Subscriber = Box<dyn Fn(&AgentEvent) + Send + Sync>;

struct EventBusInner {
    subscribers: Vec<Subscriber>,
}

pub struct EventBus {
    inner: Arc<RwLock<EventBusInner>>,
}

impl std::fmt::Debug for EventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventBus").finish_non_exhaustive()
    }
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        EventBus {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        EventBus::new()
    }
}

impl EventBus {
    pub fn new() -> Self {
        EventBus {
            inner: Arc::new(RwLock::new(EventBusInner {
                subscribers: Vec::new(),
            })),
        }
    }

    pub async fn subscribe(&self, handler: impl Fn(&AgentEvent) + Send + Sync + 'static) {
        let mut inner = self.inner.write().await;
        inner.subscribers.push(Box::new(handler));
    }

    pub async fn emit(&self, event: &AgentEvent) {
        let inner = self.inner.read().await;
        for handler in &inner.subscribers {
            handler(event);
        }
    }

    pub async fn subscriber_count(&self) -> usize {
        self.inner.read().await.subscribers.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[tokio::test]
    async fn emit_with_no_subscribers() {
        let bus = EventBus::new();
        bus.emit(&AgentEvent::SessionStarted {
            session_id: "test".to_string(),
        })
        .await;
        assert_eq!(bus.subscriber_count().await, 0);
    }

    #[tokio::test]
    async fn subscribe_and_emit() {
        let bus = EventBus::new();
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();
        bus.subscribe(move |event| {
            received_clone.lock().unwrap().push(format!("{event:?}"));
        })
        .await;

        bus.emit(&AgentEvent::SessionStarted {
            session_id: "s1".to_string(),
        })
        .await;
        bus.emit(&AgentEvent::AnswerProduced {
            iteration: 5,
            answer_length: 42,
        })
        .await;

        let events = received.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert!(events[0].contains("SessionStarted"));
        assert!(events[1].contains("AnswerProduced"));
    }

    #[tokio::test]
    async fn multiple_subscribers() {
        let bus = EventBus::new();
        let count = Arc::new(Mutex::new(0usize));
        let c1 = count.clone();
        let c2 = count.clone();
        bus.subscribe(move |_| {
            *c1.lock().unwrap() += 1;
        })
        .await;
        bus.subscribe(move |_| {
            *c2.lock().unwrap() += 1;
        })
        .await;

        bus.emit(&AgentEvent::MaxIterationsReached { iterations: 10 })
            .await;

        assert_eq!(*count.lock().unwrap(), 2);
        assert_eq!(bus.subscriber_count().await, 2);
    }

    #[tokio::test]
    async fn clone_shares_subscribers() {
        let bus = EventBus::new();
        let count = Arc::new(Mutex::new(0usize));
        let c = count.clone();
        bus.subscribe(move |_| {
            *c.lock().unwrap() += 1;
        })
        .await;

        let bus2 = bus.clone();
        bus2.emit(&AgentEvent::SessionStarted {
            session_id: "x".to_string(),
        })
        .await;

        assert_eq!(*count.lock().unwrap(), 1);
        assert_eq!(bus.subscriber_count().await, 1);
    }

    #[tokio::test]
    async fn event_variants_debug_format() {
        let event = AgentEvent::ToolCallCompleted {
            iteration: 3,
            tool_name: "file_read".to_string(),
            tool_call_id: "tc_1".to_string(),
            success: true,
            output_bytes: 1024,
            truncated: false,
        };
        let s = format!("{event:?}");
        assert!(s.contains("file_read"));
        assert!(s.contains("1024"));
    }

    #[tokio::test]
    async fn subscriber_captures_event_details() {
        let bus = EventBus::new();
        let captured = Arc::new(Mutex::new(None));
        let c = captured.clone();
        bus.subscribe(move |event| {
            if let AgentEvent::ToolCallStarted {
                tool_name,
                iteration,
                ..
            } = event
            {
                *c.lock().unwrap() = Some((tool_name.clone(), *iteration));
            }
        })
        .await;

        bus.emit(&AgentEvent::ToolCallStarted {
            iteration: 7,
            tool_name: "graph_query".to_string(),
            tool_call_id: "tc_42".to_string(),
        })
        .await;

        let details = captured.lock().unwrap().take();
        assert_eq!(details, Some(("graph_query".to_string(), 7)));
    }
}

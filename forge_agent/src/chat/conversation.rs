use crate::chat::memory::ConversationStore;
use crate::chat::types::{ChatMessage, Role, Usage};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Conversation {
    messages: Vec<ChatMessage>,
    max_messages: Option<usize>,
    session_id: Option<String>,
    store: Option<Arc<dyn ConversationStore>>,
    accumulated_usage: Usage,
}

impl Conversation {
    pub fn new() -> Self {
        Conversation {
            messages: Vec::new(),
            max_messages: None,
            session_id: None,
            store: None,
            accumulated_usage: Usage::default(),
        }
    }

    pub fn with_system(text: impl Into<String>) -> Self {
        let mut conv = Self::new();
        conv.push(ChatMessage::system(text));
        conv
    }

    pub fn with_max_messages(mut self, limit: usize) -> Self {
        self.max_messages = Some(limit);
        self
    }

    pub fn with_session_id(mut self, id: impl Into<String>) -> Self {
        self.session_id = Some(id.into());
        self
    }

    pub fn with_store(mut self, store: Arc<dyn ConversationStore>) -> Self {
        self.store = Some(store);
        self
    }

    pub fn record_usage(&mut self, usage: Usage) {
        let acc = &mut self.accumulated_usage;
        acc.prompt_tokens = Some(acc.prompt_tokens.unwrap_or(0) + usage.prompt_tokens.unwrap_or(0));
        acc.completion_tokens =
            Some(acc.completion_tokens.unwrap_or(0) + usage.completion_tokens.unwrap_or(0));
        acc.total_tokens = Some(acc.total_tokens.unwrap_or(0) + usage.total_tokens.unwrap_or(0));
        self.auto_save();
    }

    pub fn total_tokens(&self) -> u64 {
        self.accumulated_usage.total_tokens.unwrap_or(0)
    }

    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    pub fn push(&mut self, message: ChatMessage) {
        self.messages.push(message);
        self.enforce_limit();
        self.auto_save();
    }

    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn last(&self) -> Option<&ChatMessage> {
        self.messages.last()
    }

    pub fn last_user_message(&self) -> Option<&ChatMessage> {
        self.messages.iter().rev().find(|m| m.role == Role::User)
    }

    pub fn truncate_to(&mut self, keep: usize) {
        if self.messages.len() <= keep {
            return;
        }
        let system_count = self
            .messages
            .iter()
            .take_while(|m| m.role == Role::System)
            .count();
        if keep <= system_count {
            self.messages.truncate(system_count);
            return;
        }
        let system_msgs: Vec<ChatMessage> =
            self.messages.iter().take(system_count).cloned().collect();
        let remaining = keep - system_count;
        let non_system_len = self.messages.len() - system_count;
        let skip = non_system_len.saturating_sub(remaining);
        let recent: Vec<ChatMessage> = self
            .messages
            .iter()
            .skip(system_count)
            .skip(skip)
            .take(remaining)
            .cloned()
            .collect();
        self.messages = system_msgs;
        self.messages.extend(recent);
    }

    fn enforce_limit(&mut self) {
        if let Some(max) = self.max_messages {
            if self.messages.len() > max {
                self.truncate_to(max);
            }
        }
    }

    fn auto_save(&self) {
        if let (Some(store), Some(session_id)) = (&self.store, &self.session_id) {
            let _ = store.save(session_id, &self.messages, &self.accumulated_usage);
        }
    }
}

impl Default for Conversation {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::types::ContentBlock;

    #[test]
    fn empty_conversation() {
        let conv = Conversation::new();
        assert!(conv.is_empty());
        assert_eq!(conv.len(), 0);
        assert!(conv.last().is_none());
    }

    #[test]
    fn with_system_sets_system_message() {
        let conv = Conversation::with_system("You are helpful.");
        assert_eq!(conv.len(), 1);
        assert_eq!(conv.messages()[0].role, Role::System);
        assert_eq!(conv.messages()[0].text(), Some("You are helpful."));
    }

    #[test]
    fn push_and_retrieve() {
        let mut conv = Conversation::new();
        conv.push(ChatMessage::user("Hello"));
        conv.push(ChatMessage::assistant("Hi there"));

        assert_eq!(conv.len(), 2);
        assert_eq!(conv.last().unwrap().role, Role::Assistant);
    }

    #[test]
    fn last_user_message_skips_assistant() {
        let mut conv = Conversation::new();
        conv.push(ChatMessage::user("First"));
        conv.push(ChatMessage::assistant("Response"));
        conv.push(ChatMessage::user("Second"));

        assert_eq!(conv.last_user_message().unwrap().text(), Some("Second"));
    }

    #[test]
    fn truncate_preserves_system_messages() {
        let mut conv = Conversation::with_system("System prompt");
        for i in 0..5 {
            conv.push(ChatMessage::user(format!("User {i}")));
            conv.push(ChatMessage::assistant(format!("Reply {i}")));
        }
        assert_eq!(conv.len(), 11);

        conv.truncate_to(5);

        assert_eq!(conv.len(), 5);
        assert_eq!(conv.messages()[0].role, Role::System);
        assert_eq!(conv.messages()[0].text(), Some("System prompt"));
    }

    #[test]
    fn truncate_without_system() {
        let mut conv = Conversation::new();
        for i in 0..10 {
            conv.push(ChatMessage::user(format!("Msg {i}")));
        }
        assert_eq!(conv.len(), 10);

        conv.truncate_to(3);
        assert_eq!(conv.len(), 3);
        assert_eq!(conv.messages()[0].text(), Some("Msg 7"));
    }

    #[test]
    fn max_messages_enforced_on_push() {
        let mut conv = Conversation::with_system("Sys").with_max_messages(4);
        conv.push(ChatMessage::user("A"));
        conv.push(ChatMessage::assistant("B"));
        conv.push(ChatMessage::user("C"));
        conv.push(ChatMessage::assistant("D"));

        assert!(conv.len() <= 4);
        assert_eq!(conv.messages()[0].role, Role::System);
    }

    #[test]
    fn conversation_with_tool_roundtrip() {
        let mut conv = Conversation::with_system("Sys");
        conv.push(ChatMessage::user("Read foo.rs"));

        let assistant = ChatMessage {
            role: Role::Assistant,
            content: vec![ContentBlock::tool_call(
                "call_1",
                "file_read",
                serde_json::json!({"path": "foo.rs"}),
            )],
        };
        conv.push(assistant);
        conv.push(ChatMessage::tool_result("call_1", "fn main() {}"));

        assert_eq!(conv.len(), 4);
        assert!(matches!(
            &conv.last().unwrap().content[0],
            ContentBlock::ToolResult {
                is_error: false,
                ..
            }
        ));
    }

    #[test]
    fn default_is_empty() {
        let conv = Conversation::default();
        assert!(conv.is_empty());
    }
}

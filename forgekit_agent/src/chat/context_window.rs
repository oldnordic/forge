use crate::chat::types::{ChatMessage, Role};

#[derive(Clone, Debug)]
pub struct ContextWindow {
    pub max_tokens: u64,
    pub reserved_for_response: u64,
    pub strategy: TrimStrategy,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TrimStrategy {
    SlidingWindow { keep_recent: usize },
    KeepSystemAndRecent { keep_recent: usize },
}

impl ContextWindow {
    pub fn new(max_tokens: u64) -> Self {
        ContextWindow {
            max_tokens,
            reserved_for_response: max_tokens / 4,
            strategy: TrimStrategy::KeepSystemAndRecent { keep_recent: 10 },
        }
    }

    pub fn with_response_reserve(mut self, tokens: u64) -> Self {
        self.reserved_for_response = tokens;
        self
    }

    pub fn with_strategy(mut self, strategy: TrimStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    pub fn available_for_messages(&self) -> u64 {
        self.max_tokens.saturating_sub(self.reserved_for_response)
    }

    pub fn should_trim(&self, _messages: &[ChatMessage], used_tokens: u64) -> bool {
        let budget = self.available_for_messages();
        used_tokens > budget
    }

    pub fn trim(&self, messages: &mut Vec<ChatMessage>) {
        match &self.strategy {
            TrimStrategy::SlidingWindow { keep_recent } => {
                if messages.len() <= *keep_recent {
                    return;
                }
                let skip = messages.len() - keep_recent;
                let trimmed: Vec<ChatMessage> = messages.iter().skip(skip).cloned().collect();
                *messages = trimmed;
            }
            TrimStrategy::KeepSystemAndRecent { keep_recent } => {
                let system_count = messages
                    .iter()
                    .take_while(|m| m.role == Role::System)
                    .count();
                let non_system_len = messages.len() - system_count;
                if non_system_len <= *keep_recent {
                    return;
                }
                let system_msgs: Vec<ChatMessage> =
                    messages.iter().take(system_count).cloned().collect();
                let skip = non_system_len - keep_recent;
                let recent: Vec<ChatMessage> = messages
                    .iter()
                    .skip(system_count)
                    .skip(skip)
                    .cloned()
                    .collect();
                *messages = system_msgs;
                messages.extend(recent);
            }
        }
    }
}

impl Default for ContextWindow {
    fn default() -> Self {
        ContextWindow::new(128_000)
    }
}

pub fn estimate_tokens(messages: &[ChatMessage]) -> u64 {
    let mut total: u64 = 0;
    for msg in messages {
        for block in &msg.content {
            match block {
                crate::chat::types::ContentBlock::Text { text } => {
                    total += (text.len() as u64) / 4 + 1;
                }
                crate::chat::types::ContentBlock::ToolCall {
                    name, arguments, ..
                } => {
                    total += (name.len() as u64) / 4 + 1;
                    total += (arguments.to_string().len() as u64) / 4 + 1;
                }
                crate::chat::types::ContentBlock::ToolResult { content, .. } => {
                    total += (content.len() as u64) / 4 + 1;
                }
            }
        }
        total += 4;
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_context_window() {
        let cw = ContextWindow::default();
        assert_eq!(cw.max_tokens, 128_000);
        assert_eq!(cw.available_for_messages(), 96_000);
    }

    #[test]
    fn custom_reserve() {
        let cw = ContextWindow::new(100_000).with_response_reserve(10_000);
        assert_eq!(cw.available_for_messages(), 90_000);
    }

    #[test]
    fn sliding_window_trim() {
        let cw = ContextWindow::new(100_000)
            .with_strategy(TrimStrategy::SlidingWindow { keep_recent: 3 });

        let mut msgs = vec![
            ChatMessage::system("sys"),
            ChatMessage::user("a"),
            ChatMessage::assistant("b"),
            ChatMessage::user("c"),
            ChatMessage::assistant("d"),
        ];
        cw.trim(&mut msgs);
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0].text(), Some("b"));
        assert_eq!(msgs[1].text(), Some("c"));
        assert_eq!(msgs[2].text(), Some("d"));
    }

    #[test]
    fn keep_system_and_recent_trim() {
        let cw = ContextWindow::new(100_000)
            .with_strategy(TrimStrategy::KeepSystemAndRecent { keep_recent: 2 });

        let mut msgs = vec![
            ChatMessage::system("sys"),
            ChatMessage::user("a"),
            ChatMessage::assistant("b"),
            ChatMessage::user("c"),
            ChatMessage::assistant("d"),
        ];
        cw.trim(&mut msgs);
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0].role, Role::System);
        assert_eq!(msgs[1].text(), Some("c"));
        assert_eq!(msgs[2].text(), Some("d"));
    }

    #[test]
    fn no_trim_when_within_budget() {
        let cw = ContextWindow::new(100_000);
        let msgs = vec![ChatMessage::user("hello")];
        assert!(!cw.should_trim(&msgs, 10));
    }

    #[test]
    fn trim_triggered_over_budget() {
        let cw = ContextWindow::new(100_000);
        let msgs = vec![ChatMessage::user("hello")];
        assert!(cw.should_trim(&msgs, 200_000));
    }

    #[test]
    fn estimate_tokens_approximate() {
        let msgs = vec![
            ChatMessage::system("You are helpful."),
            ChatMessage::user("Hello world"),
        ];
        let tokens = estimate_tokens(&msgs);
        assert!(tokens > 0);
        assert!(tokens < 100);
    }

    #[test]
    fn no_trim_when_messages_fewer_than_keep() {
        let cw = ContextWindow::new(100_000)
            .with_strategy(TrimStrategy::SlidingWindow { keep_recent: 10 });

        let mut msgs = vec![ChatMessage::user("a"), ChatMessage::assistant("b")];
        cw.trim(&mut msgs);
        assert_eq!(msgs.len(), 2);
    }

    #[test]
    fn keep_system_and_recent_no_system() {
        let cw = ContextWindow::new(100_000)
            .with_strategy(TrimStrategy::KeepSystemAndRecent { keep_recent: 2 });

        let mut msgs = vec![
            ChatMessage::user("a"),
            ChatMessage::assistant("b"),
            ChatMessage::user("c"),
            ChatMessage::assistant("d"),
            ChatMessage::user("e"),
        ];
        cw.trim(&mut msgs);
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].text(), Some("d"));
        assert_eq!(msgs[1].text(), Some("e"));
    }
}

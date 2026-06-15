use crate::chat::types::{ChatMessage, Usage};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionMeta {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: usize,
    pub total_tokens: u64,
}

pub trait ConversationStore: Send + Sync + std::fmt::Debug {
    fn save(&self, session_id: &str, messages: &[ChatMessage], usage: &Usage) -> Result<()>;
    fn load(&self, session_id: &str) -> Result<Option<StoredConversation>>;
    fn list_sessions(&self) -> Result<Vec<SessionMeta>>;
    fn delete(&self, session_id: &str) -> Result<bool>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredConversation {
    pub meta: SessionMeta,
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug)]
pub struct FileConversationStore {
    dir: PathBuf,
}

impl FileConversationStore {
    pub fn new(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    pub fn in_memory() -> Self {
        Self {
            dir: PathBuf::from(":memory:"),
        }
    }

    fn session_path(&self, session_id: &str) -> PathBuf {
        self.dir.join(format!("{session_id}.json"))
    }
}

impl ConversationStore for FileConversationStore {
    fn save(&self, session_id: &str, messages: &[ChatMessage], usage: &Usage) -> Result<()> {
        if self.dir.to_str() == Some(":memory:") {
            return Ok(());
        }
        let now = chrono::Utc::now().to_rfc3339();
        let meta = SessionMeta {
            id: session_id.to_string(),
            created_at: now.clone(),
            updated_at: now,
            message_count: messages.len(),
            total_tokens: usage.total_tokens.unwrap_or(0),
        };
        let stored = StoredConversation {
            meta,
            messages: messages.to_vec(),
        };
        let json = serde_json::to_string_pretty(&stored)?;
        let path = self.session_path(session_id);
        std::fs::write(&path, json)?;
        Ok(())
    }

    fn load(&self, session_id: &str) -> Result<Option<StoredConversation>> {
        if self.dir.to_str() == Some(":memory:") {
            return Ok(None);
        }
        let path = self.session_path(session_id);
        if !path.exists() {
            return Ok(None);
        }
        let json = std::fs::read_to_string(&path)?;
        let stored: StoredConversation = serde_json::from_str(&json)?;
        Ok(Some(stored))
    }

    fn list_sessions(&self) -> Result<Vec<SessionMeta>> {
        if self.dir.to_str() == Some(":memory:") {
            return Ok(Vec::new());
        }
        let mut sessions = Vec::new();
        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(json) = std::fs::read_to_string(&path) {
                    if let Ok(stored) = serde_json::from_str::<StoredConversation>(&json) {
                        sessions.push(stored.meta);
                    }
                }
            }
        }
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(sessions)
    }

    fn delete(&self, session_id: &str) -> Result<bool> {
        if self.dir.to_str() == Some(":memory:") {
            return Ok(false);
        }
        let path = self.session_path(session_id);
        if path.exists() {
            std::fs::remove_file(path)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::types::Role;

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileConversationStore::new(dir.path()).unwrap();

        let mut messages = vec![
            ChatMessage::system("You are helpful."),
            ChatMessage::user("Hello"),
            ChatMessage::assistant("Hi there!"),
        ];
        messages.push(ChatMessage {
            role: Role::Assistant,
            content: vec![crate::chat::types::ContentBlock::tool_call(
                "call_1",
                "file_read",
                serde_json::json!({"path": "test.rs"}),
            )],
        });
        messages.push(ChatMessage::tool_result("call_1", "fn main() {}"));

        let usage = Usage {
            prompt_tokens: Some(100),
            completion_tokens: Some(50),
            total_tokens: Some(150),
        };

        store.save("session-1", &messages, &usage).unwrap();

        let loaded = store.load("session-1").unwrap().expect("should load");
        assert_eq!(loaded.messages.len(), 5);
        assert_eq!(loaded.messages[0].role, Role::System);
        assert_eq!(loaded.messages[1].text(), Some("Hello"));
        assert_eq!(loaded.meta.total_tokens, 150);
        assert_eq!(loaded.meta.message_count, 5);
    }

    #[test]
    fn load_nonexistent_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileConversationStore::new(dir.path()).unwrap();
        let result = store.load("no-such-session").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn list_sessions_returns_sorted() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileConversationStore::new(dir.path()).unwrap();

        let msgs = vec![ChatMessage::user("test")];
        let usage = Usage::default();

        store.save("session-a", &msgs, &usage).unwrap();
        store.save("session-b", &msgs, &usage).unwrap();

        let sessions = store.list_sessions().unwrap();
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn delete_removes_session() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileConversationStore::new(dir.path()).unwrap();

        let msgs = vec![ChatMessage::user("test")];
        let usage = Usage::default();
        store.save("to-delete", &msgs, &usage).unwrap();
        assert!(store.load("to-delete").unwrap().is_some());

        let deleted = store.delete("to-delete").unwrap();
        assert!(deleted);
        assert!(store.load("to-delete").unwrap().is_none());
    }

    #[test]
    fn delete_nonexistent_returns_false() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileConversationStore::new(dir.path()).unwrap();
        assert!(!store.delete("nope").unwrap());
    }

    #[test]
    fn save_overwrites_existing() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileConversationStore::new(dir.path()).unwrap();

        let usage = Usage::default();
        store
            .save("session-1", &[ChatMessage::user("first")], &usage)
            .unwrap();
        store
            .save("session-1", &[ChatMessage::user("second")], &usage)
            .unwrap();

        let loaded = store.load("session-1").unwrap().unwrap();
        assert_eq!(loaded.messages.len(), 1);
        assert_eq!(loaded.messages[0].text(), Some("second"));
    }

    #[test]
    fn in_memory_store_is_noop() {
        let store = FileConversationStore::in_memory();
        let usage = Usage {
            total_tokens: Some(42),
            ..Default::default()
        };
        store
            .save("test", &[ChatMessage::user("hello")], &usage)
            .unwrap();
        assert!(store.load("test").unwrap().is_none());
        assert!(store.list_sessions().unwrap().is_empty());
        assert!(!store.delete("test").unwrap());
    }

    #[test]
    fn conversation_with_store_saves_on_push() {
        let dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(FileConversationStore::new(dir.path()).unwrap());

        let mut conv = crate::chat::conversation::Conversation::new()
            .with_session_id("auto-save-test")
            .with_store(store.clone());

        conv.push(ChatMessage::user("Hello"));
        conv.push(ChatMessage::assistant("World"));

        let loaded = store.load("auto-save-test").unwrap().unwrap();
        assert_eq!(loaded.messages.len(), 2);
    }

    #[test]
    fn conversation_accumulates_usage() {
        let dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(FileConversationStore::new(dir.path()).unwrap());

        let mut conv = crate::chat::conversation::Conversation::new()
            .with_session_id("usage-test")
            .with_store(store.clone());

        conv.push(ChatMessage::user("Hello"));
        conv.record_usage(Usage {
            prompt_tokens: Some(10),
            completion_tokens: Some(5),
            total_tokens: Some(15),
        });
        conv.push(ChatMessage::assistant("World"));
        conv.record_usage(Usage {
            prompt_tokens: Some(20),
            completion_tokens: Some(10),
            total_tokens: Some(30),
        });

        let loaded = store.load("usage-test").unwrap().unwrap();
        assert_eq!(loaded.meta.total_tokens, 45);
    }
}

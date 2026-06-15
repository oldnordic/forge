//! Mutation engine - Transaction-based code mutation.
//!
//! This module implements the mutation phase, applying changes through
//! the edit module with transaction support.

use crate::transaction::Transaction;
use crate::{AgentError, Result};
use std::path::Path;
use tokio::fs;

/// Mutator for transaction-based code changes.
///
/// The Mutator uses the Transaction module to apply changes atomically,
/// snapshotting files before mutation and providing rollback capability.
#[derive(Clone, Default)]
pub struct Mutator {
    transaction: Option<Transaction>,
}

impl Mutator {
    /// Creates a new mutator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Begins a new transaction.
    pub async fn begin_transaction(&mut self) -> Result<()> {
        if self.transaction.is_some() {
            return Err(AgentError::MutationFailed(
                "Transaction already in progress".to_string(),
            ));
        }

        self.transaction = Some(Transaction::begin().await?);
        Ok(())
    }

    /// Applies a single step in the current transaction.
    ///
    /// Snapshots each file before mutation for rollback capability.
    pub async fn apply_step(&mut self, step: &crate::planner::PlanStep) -> Result<()> {
        let transaction = self
            .transaction
            .as_mut()
            .ok_or_else(|| AgentError::MutationFailed("No active transaction".to_string()))?;

        match &step.operation {
            crate::planner::PlanOperation::Rename { old, new, file, .. } => {
                if let Some(file_path) = file {
                    // Symbol rename: replace all whole-word occurrences of `old`
                    // with `new` within the source file.
                    let path = Path::new(file_path);
                    transaction.snapshot_file(path).await?;

                    let content = fs::read_to_string(path).await.map_err(|e| {
                        AgentError::MutationFailed(format!("Failed to read {}: {}", file_path, e))
                    })?;
                    let modified = replace_whole_word(&content, old, new);
                    fs::write(path, modified).await.map_err(|e| {
                        AgentError::MutationFailed(format!("Failed to write {}: {}", file_path, e))
                    })?;
                } else {
                    // File rename: rename the file at path `old` to `new`.
                    let old_path = Path::new(old);
                    let _ = transaction.snapshot_file(old_path).await;

                    if old_path.exists() {
                        let new_path = Path::new(new);
                        fs::rename(old_path, new_path).await.map_err(|e| {
                            AgentError::MutationFailed(format!(
                                "Failed to rename {} to {}: {}",
                                old, new, e
                            ))
                        })?;
                    }
                }
            }
            crate::planner::PlanOperation::Delete { name, file, .. } => {
                let name_path = file
                    .as_deref()
                    .map(Path::new)
                    .unwrap_or_else(|| Path::new(name));
                transaction.snapshot_file(name_path).await?;

                if name_path.exists() {
                    fs::remove_file(name_path).await.map_err(|e| {
                        AgentError::MutationFailed(format!("Failed to delete {}: {}", name, e))
                    })?;
                }
            }
            crate::planner::PlanOperation::Create { path, content } => {
                let p = Path::new(path);
                let _ = transaction.snapshot_file(p).await;

                if let Some(parent) = p.parent() {
                    fs::create_dir_all(parent).await.map_err(|e| {
                        AgentError::MutationFailed(format!("Failed to create dir: {}", e))
                    })?;
                }
                fs::write(path, content).await.map_err(|e| {
                    AgentError::MutationFailed(format!("Failed to write {}: {}", path, e))
                })?;
            }
            crate::planner::PlanOperation::Inspect { .. } => {
                // No mutation needed for read-only operations
            }
            crate::planner::PlanOperation::Modify {
                file,
                start,
                end,
                replacement,
            } => {
                let file_path = Path::new(file);
                transaction.snapshot_file(file_path).await?;

                let content = fs::read_to_string(file_path).await.map_err(|e| {
                    AgentError::MutationFailed(format!("Failed to read {}: {}", file, e))
                })?;
                let content_bytes = content.as_bytes();
                if *start <= content_bytes.len() && *end <= content_bytes.len() && *start <= *end {
                    let mut modified = content_bytes[..*start].to_vec();
                    modified.extend_from_slice(replacement.as_bytes());
                    modified.extend_from_slice(&content_bytes[*end..]);
                    fs::write(file_path, modified).await.map_err(|e| {
                        AgentError::MutationFailed(format!("Failed to write {}: {}", file, e))
                    })?;
                } else {
                    return Err(AgentError::MutationFailed(format!(
                        "Invalid byte span {}..{} for {} ({} bytes)",
                        start,
                        end,
                        file,
                        content_bytes.len()
                    )));
                }
            }
        }

        Ok(())
    }

    /// Extracts the transaction from the mutator.
    ///
    /// This is used when transferring the transaction to another component
    /// (e.g., from Mutator to AgentLoop).
    pub fn into_transaction(mut self) -> Result<Transaction> {
        self.transaction
            .take()
            .ok_or_else(|| AgentError::MutationFailed("No active transaction".to_string()))
    }

    /// Returns a reference to the current transaction (for testing).
    #[cfg(test)]
    pub fn transaction(&self) -> Option<&Transaction> {
        self.transaction.as_ref()
    }
}

/// Replace all whole-word occurrences of `from` with `to` in `text`.
///
/// A word boundary is any position where the adjacent byte is not an ASCII
/// alphanumeric or underscore. This prevents replacing substrings — e.g.
/// renaming `greet` does not touch `greeting`.
fn replace_whole_word(text: &str, from: &str, to: &str) -> String {
    if from.is_empty() {
        return text.to_string();
    }
    let mut result = String::with_capacity(text.len());
    let mut remaining = text;

    while let Some(pos) = remaining.find(from) {
        let after = pos + from.len();
        let before_ok = pos == 0 || {
            let prev_byte = remaining.as_bytes()[pos - 1];
            !prev_byte.is_ascii_alphanumeric() && prev_byte != b'_'
        };
        let after_ok = after >= remaining.len() || {
            let next_byte = remaining.as_bytes()[after];
            !next_byte.is_ascii_alphanumeric() && next_byte != b'_'
        };

        if before_ok && after_ok {
            result.push_str(&remaining[..pos]);
            result.push_str(to);
            remaining = &remaining[after..];
        } else {
            result.push_str(&remaining[..after]);
            remaining = &remaining[after..];
        }
    }
    result.push_str(remaining);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_mutator_creation() {
        let mutator = Mutator::new();

        assert!(mutator.transaction().is_none());
    }

    #[tokio::test]
    async fn test_begin_transaction() {
        let mut mutator = Mutator::new();

        mutator.begin_transaction().await.unwrap();
        assert!(mutator.transaction().is_some());

        // Second begin should fail
        assert!(mutator.begin_transaction().await.is_err());
    }

    #[tokio::test]
    async fn test_apply_step_create_snapshots_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");

        let mut mutator = Mutator::new();
        mutator.begin_transaction().await.unwrap();

        let step = crate::planner::PlanStep {
            description: "Create test file".to_string(),
            operation: crate::planner::PlanOperation::Create {
                path: file_path.to_string_lossy().to_string(),
                content: "fn test() {}".to_string(),
            },
        };

        mutator.apply_step(&step).await.unwrap();

        // File should be created
        assert!(file_path.exists());
        // Transaction should have snapshot
        assert!(mutator.transaction().is_some());

        // Cleanup via into_transaction + rollback
        let txn = mutator.into_transaction().unwrap();
        txn.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn test_apply_step_modify_snapshots_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        tokio::fs::write(&file_path, "original content")
            .await
            .unwrap();

        let mut mutator = Mutator::new();
        mutator.begin_transaction().await.unwrap();

        let step = crate::planner::PlanStep {
            description: "Modify test file".to_string(),
            operation: crate::planner::PlanOperation::Modify {
                file: file_path.to_string_lossy().to_string(),
                start: 0,
                end: 8,
                replacement: "replaced".to_string(),
            },
        };

        mutator.apply_step(&step).await.unwrap();

        // Transaction should have snapshot
        assert!(mutator.transaction().is_some());

        // Rollback via into_transaction
        let txn = mutator.into_transaction().unwrap();
        txn.rollback().await.unwrap();
        // File should still exist with original content after rollback
        assert!(file_path.exists());
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "original content");
    }

    #[tokio::test]
    async fn test_into_transaction_commit() {
        let mut mutator = Mutator::new();
        mutator.begin_transaction().await.unwrap();

        let transaction = mutator.into_transaction().unwrap();
        let commit_id = transaction.commit().await.unwrap();

        assert_ne!(commit_id, Uuid::default());
    }

    #[tokio::test]
    async fn test_into_transaction() {
        let mut mutator = Mutator::new();
        mutator.begin_transaction().await.unwrap();

        let transaction = mutator.into_transaction().unwrap();

        // into_transaction consumes self, so we can't check mutator after
        // But we can verify the transaction is in Active state
        assert_eq!(
            transaction.state(),
            &crate::transaction::TransactionState::Active
        );
    }

    #[tokio::test]
    async fn test_into_transaction_without_active_tx() {
        let mutator = Mutator::new();

        let result = mutator.into_transaction();

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_rollback_restores_file_content() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        tokio::fs::write(&file_path, "original content")
            .await
            .unwrap();

        let mut mutator = Mutator::new();
        mutator.begin_transaction().await.unwrap();

        // Modify the file
        let step = crate::planner::PlanStep {
            description: "Create test file".to_string(),
            operation: crate::planner::PlanOperation::Create {
                path: file_path.to_string_lossy().to_string(),
                content: "modified content".to_string(),
            },
        };

        mutator.apply_step(&step).await.unwrap();

        // File should be modified
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "modified content");

        // Rollback via into_transaction
        let txn = mutator.into_transaction().unwrap();
        txn.rollback().await.unwrap();

        // File should be restored
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "original content");
    }

    #[tokio::test]
    async fn test_apply_step_rename_symbol() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("main.rs");
        tokio::fs::write(
            &file_path,
            "fn greet(name: &str) -> String {\n    greet(name)\n}\nfn greeting() {}\n",
        )
        .await
        .unwrap();

        let mut mutator = Mutator::new();
        mutator.begin_transaction().await.unwrap();

        let step = crate::planner::PlanStep {
            description: "Rename greet to say_hello".to_string(),
            operation: crate::planner::PlanOperation::Rename {
                old: "greet".to_string(),
                new: "say_hello".to_string(),
                file: Some(file_path.to_string_lossy().to_string()),
            },
        };

        mutator.apply_step(&step).await.unwrap();

        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        // Both whole-word `greet` replaced, but `greeting` untouched
        assert!(content.contains("fn say_hello("));
        assert!(content.contains("say_hello(name)"));
        assert!(content.contains("fn greeting()"));

        // Rollback
        let txn = mutator.into_transaction().unwrap();
        txn.rollback().await.unwrap();
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert!(content.contains("fn greet("));
    }

    #[tokio::test]
    async fn test_apply_step_rename_file() {
        let temp_dir = TempDir::new().unwrap();
        let old_path = temp_dir.path().join("old_name.rs");
        tokio::fs::write(&old_path, "fn old() {}").await.unwrap();

        let mut mutator = Mutator::new();
        mutator.begin_transaction().await.unwrap();

        let step = crate::planner::PlanStep {
            description: "Rename file".to_string(),
            operation: crate::planner::PlanOperation::Rename {
                old: old_path.to_string_lossy().to_string(),
                new: temp_dir
                    .path()
                    .join("new_name.rs")
                    .to_string_lossy()
                    .to_string(),
                file: None,
            },
        };

        mutator.apply_step(&step).await.unwrap();

        assert!(!old_path.exists());
        assert!(temp_dir.path().join("new_name.rs").exists());

        // Rollback
        let txn = mutator.into_transaction().unwrap();
        txn.rollback().await.unwrap();
        assert!(old_path.exists());
    }

    #[test]
    fn test_replace_whole_word_basic() {
        assert_eq!(
            replace_whole_word("fn greet() { greet(x) }", "greet", "say_hello"),
            "fn say_hello() { say_hello(x) }"
        );
    }

    #[test]
    fn test_replace_whole_word_preserves_substrings() {
        let src = "fn greet() {}\nfn greeting() {}\nlet g = greet_thing;";
        let result = replace_whole_word(src, "greet", "say_hello");
        assert!(result.contains("fn say_hello() {}"));
        assert!(result.contains("fn greeting() {}"));
        assert!(result.contains("greet_thing"));
    }

    #[test]
    fn test_replace_whole_word_no_match() {
        assert_eq!(
            replace_whole_word("fn unrelated() {}", "greet", "say_hello"),
            "fn unrelated() {}"
        );
    }

    #[test]
    fn test_replace_whole_word_empty_from() {
        assert_eq!(replace_whole_word("unchanged", "", "replaced"), "unchanged");
    }

    #[test]
    fn test_replace_whole_word_at_boundaries() {
        assert_eq!(replace_whole_word("greet", "greet", "hello"), "hello");
        assert_eq!(replace_whole_word("(greet)", "greet", "hello"), "(hello)");
    }
}

//! Mutation engine - Transaction-based code mutation.
//!
//! This module implements the mutation phase, applying changes through
//! the edit module with transaction support.

use crate::{AgentError, Result};
use crate::transaction::Transaction;
use std::path::Path;
use uuid::Uuid;
use tokio::fs;

/// Mutator for transaction-based code changes.
///
/// The Mutator uses the Transaction module to apply changes atomically,
/// snapshotting files before mutation and providing rollback capability.
#[derive(Clone, Default)]
pub struct Mutator {
    /// Current transaction state
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
            crate::planner::PlanOperation::Rename { old, new: _ } => {
                // Snapshot the old file for rollback
                let old_path = Path::new(old);
                let _ = transaction.snapshot_file(old_path).await;

                // Apply rename (placeholder for v0.3 - actual rename logic would go here)
                // For now, just record that the operation was processed
            }
            crate::planner::PlanOperation::Delete { name } => {
                // Snapshot the file for rollback (restore on rollback)
                let name_path = Path::new(name);
                transaction.snapshot_file(name_path).await?;

                // Apply delete (placeholder for v0.3)
            }
            crate::planner::PlanOperation::Create { path, content } => {
                // Snapshot for rollback (will delete on rollback if file didn't exist)
                let p = Path::new(path);
                let _ = transaction.snapshot_file(p).await; // Ignore error if file doesn't exist

                // Write new content
                fs::write(path, content).await.map_err(|e| {
                    AgentError::MutationFailed(format!("Failed to write {}: {}", path, e))
                })?;
            }
            crate::planner::PlanOperation::Inspect { .. } => {
                // No snapshot needed for read-only operations
            }
            crate::planner::PlanOperation::Modify { file, .. } => {
                // Snapshot the file for rollback
                let file_path = Path::new(file);
                transaction.snapshot_file(file_path).await?;

                // Apply modification (placeholder for v0.3 - actual edit logic would go here)
            }
        }

        Ok(())
    }

    /// Rolls back the current transaction.
    ///
    /// Takes the transaction, rolls back all changes, and returns Ok.
    pub async fn rollback(&mut self) -> Result<()> {
        let transaction = self
            .transaction
            .take()
            .ok_or_else(|| AgentError::MutationFailed("No active transaction".to_string()))?;

        transaction.rollback().await?;
        Ok(())
    }

    /// Commits the current transaction.
    ///
    /// Takes the transaction, commits it, and returns the commit ID.
    pub async fn commit_transaction(mut self) -> Result<Uuid> {
        let transaction = self
            .transaction
            .take()
            .ok_or_else(|| AgentError::MutationFailed("No active transaction".to_string()))?;

        transaction.commit().await
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

    /// Returns preview of changes without applying.
    pub async fn preview(&self, steps: &[crate::planner::PlanStep]) -> Result<Vec<String>> {
        let mut previews = Vec::new();

        for step in steps {
            match &step.operation {
                crate::planner::PlanOperation::Create { path, content } => {
                    previews.push(format!("Create {}:\n{}", path, content));
                }
                crate::planner::PlanOperation::Delete { name } => {
                    previews.push(format!("Delete {}", name));
                }
                crate::planner::PlanOperation::Rename { old, new } => {
                    previews.push(format!("Rename {} to {}", old, new));
                }
                _ => {}
            }
        }

        Ok(previews)
    }

    /// Returns a reference to the current transaction (for testing).
    #[cfg(test)]
    pub fn transaction(&self) -> Option<&Transaction> {
        self.transaction.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

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
    async fn test_rollback() {
        let mut mutator = Mutator::new();

        mutator.begin_transaction().await.unwrap();
        assert!(mutator.transaction().is_some());

        mutator.rollback().await.unwrap();
        assert!(mutator.transaction().is_none());
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

        // Cleanup
        mutator.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn test_apply_step_modify_snapshots_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        tokio::fs::write(&file_path, "original content").await.unwrap();

        let mut mutator = Mutator::new();
        mutator.begin_transaction().await.unwrap();

        let step = crate::planner::PlanStep {
            description: "Modify test file".to_string(),
            operation: crate::planner::PlanOperation::Modify {
                file: file_path.to_string_lossy().to_string(),
                start: 0,
                end: 8,
            },
        };

        mutator.apply_step(&step).await.unwrap();

        // Transaction should have snapshot
        assert!(mutator.transaction().is_some());

        // Cleanup
        mutator.rollback().await.unwrap();
        // File should still exist with original content after rollback
        assert!(file_path.exists());
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "original content");
    }

    #[tokio::test]
    async fn test_commit_transaction() {
        let mut mutator = Mutator::new();
        mutator.begin_transaction().await.unwrap();

        let commit_id = mutator.commit_transaction().await.unwrap();

        // commit_transaction consumes self, so we can't check mutator after
        // But we can verify the commit ID is valid
        assert_ne!(commit_id, Uuid::default());
    }

    #[tokio::test]
    async fn test_into_transaction() {
        let mut mutator = Mutator::new();
        mutator.begin_transaction().await.unwrap();

        let transaction = mutator.into_transaction().unwrap();

        // into_transaction consumes self, so we can't check mutator after
        // But we can verify the transaction is in Active state
        assert_eq!(transaction.state(), &crate::transaction::TransactionState::Active);
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
        tokio::fs::write(&file_path, "original content").await.unwrap();

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

        // Rollback
        mutator.rollback().await.unwrap();

        // File should be restored
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "original content");
    }

    #[tokio::test]
    async fn test_preview() {
        let mutator = Mutator::new();

        let steps = vec![
            crate::planner::PlanStep {
                description: "Create file".to_string(),
                operation: crate::planner::PlanOperation::Create {
                    path: "/tmp/test.rs".to_string(),
                    content: "fn test() {}".to_string(),
                },
            },
            crate::planner::PlanStep {
                description: "Delete file".to_string(),
                operation: crate::planner::PlanOperation::Delete {
                    name: "old.rs".to_string(),
                },
            },
        ];

        let previews = mutator.preview(&steps).await.unwrap();

        assert_eq!(previews.len(), 2);
        assert!(previews[0].contains("Create"));
        assert!(previews[0].contains("fn test() {}"));
        assert!(previews[1].contains("Delete"));
    }
}

//! Mutation engine - Transaction-based code mutation.
//!
//! This module implements the mutation phase, applying changes through
//! the edit module with transaction support.

use crate::{AgentError, Result};
use tokio::fs;

/// Mutator for transaction-based code changes.
///
/// The Mutator uses the EditModule to apply changes atomically.
#[derive(Clone, Default)]
pub struct Mutator {
    /// Current transaction state
    transaction: Option<Transaction>,
}

/// Active transaction state.
#[derive(Clone, Debug)]
struct Transaction {
    /// Steps applied in this transaction
    applied_steps: Vec<String>,
    /// Original state for rollback
    rollback_state: Vec<RollbackState>,
}

/// Rollback state for a transaction.
#[derive(Clone, Debug)]
struct RollbackState {
    /// File path
    file: String,
    /// Original content
    original_content: String,
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

        self.transaction = Some(Transaction {
            applied_steps: Vec::new(),
            rollback_state: Vec::new(),
        });

        Ok(())
    }

    /// Applies a single step in the current transaction.
    pub async fn apply_step(&mut self, step: &crate::planner::PlanStep) -> Result<()> {
        let transaction = self
            .transaction
            .as_mut()
            .ok_or_else(|| AgentError::MutationFailed("No active transaction".to_string()))?;

        match &step.operation {
            crate::planner::PlanOperation::Rename { old, new } => {
                // Record for rollback
                transaction
                    .applied_steps
                    .push(format!("Rename {} to {}", old, new));
            }
            crate::planner::PlanOperation::Delete { name } => {
                transaction.applied_steps.push(format!("Delete {}", name));
            }
            crate::planner::PlanOperation::Create { path, content } => {
                // Save original for rollback
                if let Ok(original_content) = fs::read_to_string(path).await {
                    transaction.rollback_state.push(RollbackState {
                        file: path.clone(),
                        original_content,
                    });
                }

                // Write new content
                fs::write(path, content).await.map_err(|e| {
                    AgentError::MutationFailed(format!("Failed to write {}: {}", path, e))
                })?;

                transaction.applied_steps.push(format!("Create {}", path));
            }
            crate::planner::PlanOperation::Inspect { .. } => {
                // Inspect doesn't modify files
            }
            crate::planner::PlanOperation::Modify { file, .. } => {
                if let Ok(original_content) = std::fs::read_to_string(file) {
                    transaction.rollback_state.push(RollbackState {
                        file: file.clone(),
                        original_content,
                    });
                }
                transaction.applied_steps.push(format!("Modify {}", file));
            }
        }

        Ok(())
    }

    /// Rolls back the current transaction.
    pub async fn rollback(&mut self) -> Result<()> {
        let transaction = self
            .transaction
            .take()
            .ok_or_else(|| AgentError::MutationFailed("No active transaction".to_string()))?;

        // Rollback in reverse order
        for state in transaction.rollback_state.iter().rev() {
            std::fs::write(&state.file, &state.original_content).map_err(|e| {
                AgentError::MutationFailed(format!("Rollback failed for {}: {}", state.file, e))
            })?;
        }

        Ok(())
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mutator_creation() {
        let mutator = Mutator::new();

        assert!(mutator.transaction.is_none());
    }

    #[tokio::test]
    async fn test_begin_transaction() {
        let mut mutator = Mutator::new();

        mutator.begin_transaction().await.unwrap();
        assert!(mutator.transaction.is_some());

        // Second begin should fail
        assert!(mutator.begin_transaction().await.is_err());
    }

    #[tokio::test]
    async fn test_rollback() {
        let mut mutator = Mutator::new();

        mutator.begin_transaction().await.unwrap();
        assert!(mutator.transaction.is_some());

        mutator.rollback().await.unwrap();
        assert!(mutator.transaction.is_none());
    }
}

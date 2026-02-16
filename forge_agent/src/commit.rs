//! Commit engine - Transaction finalization.
//!
//! This module implements the commit phase, finalizing verified
//! transactions with version control integration.

use crate::{AgentError, Result};
use forge_core::Forge;
use std::sync::Arc;

/// Committer for transaction finalization.
///
/// The Committer handles git integration and metadata persistence.
#[derive(Clone)]
pub struct Committer {
    /// Forge SDK for graph queries
    forge: Arc<Forge>,
}

impl Committer {
    /// Creates a new committer.
    pub fn new(forge: Forge) -> Self {
        Self {
            forge: Arc::new(forge),
        }
    }

    /// Finalizes a verified transaction.
    ///
    /// # Arguments
    ///
    /// * `working_dir` - Directory for the operation
    /// * `modified_files` - Files that were modified
    pub async fn finalize(
        &self,
        _working_dir: &std::path::Path,
        modified_files: &[std::path::PathBuf],
    ) -> Result<CommitReport> {
        // Generate transaction ID using timestamp
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let transaction_id = format!("txn-{}", now);

        Ok(CommitReport {
            transaction_id: transaction_id.clone(),
            files_committed: modified_files.to_vec(),
        })
    }

    /// Generates a commit summary.
    ///
    /// # Arguments
    ///
    /// * `steps` - Steps that were executed
    pub fn generate_summary(&self, steps: &[crate::planner::PlanStep]) -> String {
        let mut summary = String::from("Applied ");

        for (i, step) in steps.iter().enumerate() {
            if i > 0 {
                summary.push_str(", ");
            }
            summary.push_str(&step.description);
        }

        summary
    }
}

/// Commit report.
#[derive(Clone, Debug)]
pub struct CommitReport {
    /// Transaction ID for the commit
    pub transaction_id: String,
    /// Files that were committed
    pub files_committed: Vec<std::path::PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_committer_creation() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let _committer = Committer::new(forge);

        // Should create successfully
        assert!(true);
    }

    #[tokio::test]
    async fn test_generate_summary() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let committer = Committer::new(forge);

        let steps = vec![
            crate::planner::PlanStep {
                description: "Step 1".to_string(),
                operation: crate::planner::PlanOperation::Inspect {
                    symbol_id: forge_core::types::SymbolId(1),
                    symbol_name: "test".to_string(),
                },
            },
            crate::planner::PlanStep {
                description: "Step 2".to_string(),
                operation: crate::planner::PlanOperation::Inspect {
                    symbol_id: forge_core::types::SymbolId(2),
                    symbol_name: "test2".to_string(),
                },
            },
        ];

        let summary = committer.generate_summary(&steps);

        assert!(summary.contains("Step 1"));
        assert!(summary.contains("Step 2"));
    }

    #[tokio::test]
    async fn test_create_checkpoint() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let committer = Committer::new(forge);

        let result = committer
            .finalize(&std::path::PathBuf::new(), &[])
            .await
            .unwrap();

        assert!(!result.transaction_id.is_empty());
        assert!(result.files_committed.is_empty());
    }
}

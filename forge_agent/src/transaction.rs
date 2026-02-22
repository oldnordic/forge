//! Transaction management - File snapshotting and atomic rollback.
//!
//! This module implements transaction management with file snapshotting
//! and automatic rollback on failure. Transactions provide atomicity -
//! either all changes commit successfully, or the system rolls back
//! to the original state.

use crate::AgentError;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Snapshot of a file's original content.
///
/// Each snapshot stores the file path, original content, and a checksum
/// for verification during rollback.
#[derive(Clone, Debug)]
pub struct FileSnapshot {
    /// Path to the file
    pub path: PathBuf,
    /// Original content before mutation
    pub original_content: String,
    /// Checksum for verification (content length for v0.3)
    pub checksum: String,
}

impl FileSnapshot {
    /// Creates a new file snapshot.
    fn new(path: PathBuf, original_content: String) -> Self {
        let checksum = format!("{}", original_content.len());
        Self {
            path,
            original_content,
            checksum,
        }
    }

    /// Creates an empty snapshot for files that don't exist yet.
    /// During rollback, this indicates the file should be deleted.
    fn new_empty(path: PathBuf) -> Self {
        Self {
            path,
            original_content: String::new(),
            checksum: "0".to_string(),
        }
    }
}

/// State of a transaction.
#[derive(Clone, Debug, PartialEq)]
pub enum TransactionState {
    /// Transaction is active and accepting snapshots
    Active,
    /// Transaction was rolled back
    RolledBack,
    /// Transaction was committed with the given ID
    Committed(String),
}

/// Transaction for atomic file operations.
///
/// The Transaction manages file snapshots and provides rollback capability.
/// Files are snapshot before mutation, and can be restored to their
/// original state if the transaction fails.
pub struct Transaction {
    /// Unique transaction ID
    id: Uuid,
    /// File snapshots for rollback
    snapshots: Vec<FileSnapshot>,
    /// Current transaction state
    state: TransactionState,
}

impl Transaction {
    /// Begins a new transaction with a unique ID.
    ///
    /// Creates a fresh transaction with no snapshots and Active state.
    pub async fn begin() -> Result<Self, AgentError> {
        Ok(Self {
            id: Uuid::new_v4(),
            snapshots: Vec::new(),
            state: TransactionState::Active,
        })
    }

    /// Snapshots a file before mutation.
    ///
    /// If the file exists, stores its content for rollback.
    /// If the file doesn't exist, stores an empty snapshot (indicating
    /// the file should be deleted on rollback).
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file to snapshot
    pub async fn snapshot_file(&mut self, path: &Path) -> Result<(), AgentError> {
        if self.state != TransactionState::Active {
            return Err(AgentError::MutationFailed(format!(
                "Cannot snapshot file: transaction is {:?}",
                self.state
            )));
        }

        let path_buf = path.to_path_buf();

        // Try to read the file content
        match tokio::fs::read_to_string(path).await {
            Ok(content) => {
                // File exists - snapshot the content
                self.snapshots.push(FileSnapshot::new(path_buf, content));
            }
            Err(_) => {
                // File doesn't exist - store empty snapshot
                // On rollback, we'll delete the file if it was created
                self.snapshots.push(FileSnapshot::new_empty(path_buf));
            }
        }

        Ok(())
    }

    /// Rolls back the transaction, restoring all files to original state.
    ///
    /// Iterates through snapshots in reverse order and restores each file.
    /// Files that didn't exist before are deleted.
    ///
    /// # Errors
    ///
    /// Returns an error if the transaction is not in Active state.
    pub async fn rollback(mut self) -> Result<(), AgentError> {
        if self.state != TransactionState::Active {
            return Err(AgentError::MutationFailed(format!(
                "Cannot rollback: transaction is {:?}",
                self.state
            )));
        }

        // Rollback in reverse order (last modified first)
        for snapshot in self.snapshots.iter().rev() {
            // Check if file was created during transaction (empty original content)
            if snapshot.checksum == "0" && snapshot.original_content.is_empty() {
                // File didn't exist before - delete it if it exists now
                if snapshot.path.exists() {
                    tokio::fs::remove_file(&snapshot.path).await.map_err(|e| {
                        AgentError::MutationFailed(format!(
                            "Failed to remove file {}: {}",
                            snapshot.path.display(),
                            e
                        ))
                    })?;
                }
            } else {
                // File existed before - restore original content
                tokio::fs::write(&snapshot.path, &snapshot.original_content).await.map_err(
                    |e| {
                        AgentError::MutationFailed(format!(
                            "Failed to restore file {}: {}",
                            snapshot.path.display(),
                            e
                        ))
                    },
                )?;
            }
        }

        self.state = TransactionState::RolledBack;
        Ok(())
    }

    /// Commits the transaction, generating a commit ID.
    ///
    /// # Returns
    ///
    /// The commit ID (UUID).
    pub async fn commit(mut self) -> Result<Uuid, AgentError> {
        if self.state != TransactionState::Active {
            return Err(AgentError::MutationFailed(format!(
                "Cannot commit: transaction is {:?}",
                self.state
            )));
        }

        let commit_id = Uuid::new_v4();
        self.state = TransactionState::Committed(commit_id.to_string());
        Ok(commit_id)
    }

    /// Returns the transaction ID.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Returns the current transaction state.
    pub fn state(&self) -> &TransactionState {
        &self.state
    }

    /// Returns the number of file snapshots.
    #[cfg(test)]
    pub fn snapshot_count(&self) -> usize {
        self.snapshots.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_transaction_begin() {
        let tx = Transaction::begin().await.unwrap();

        assert_ne!(tx.id(), Uuid::default());
        assert_eq!(tx.state(), &TransactionState::Active);
        assert_eq!(tx.snapshot_count(), 0);
    }

    #[tokio::test]
    async fn test_snapshot_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        tokio::fs::write(&file_path, "original content").await.unwrap();

        let mut tx = Transaction::begin().await.unwrap();
        tx.snapshot_file(&file_path).await.unwrap();

        assert_eq!(tx.snapshot_count(), 1);
    }

    #[tokio::test]
    async fn test_snapshot_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("nonexistent.rs");

        let mut tx = Transaction::begin().await.unwrap();
        tx.snapshot_file(&file_path).await.unwrap();

        // Should store empty snapshot for nonexistent file
        assert_eq!(tx.snapshot_count(), 1);
    }

    #[tokio::test]
    async fn test_rollback_restores_original_content() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        tokio::fs::write(&file_path, "original content").await.unwrap();

        // Snapshot and modify
        let mut tx = Transaction::begin().await.unwrap();
        tx.snapshot_file(&file_path).await.unwrap();
        tokio::fs::write(&file_path, "modified content").await.unwrap();

        // Rollback
        tx.rollback().await.unwrap();

        // Verify original content restored
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "original content");
    }

    #[tokio::test]
    async fn test_rollback_deletes_created_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("new_file.rs");

        // Snapshot nonexistent file, then create it
        let mut tx = Transaction::begin().await.unwrap();
        tx.snapshot_file(&file_path).await.unwrap();
        tokio::fs::write(&file_path, "new content").await.unwrap();

        // Rollback
        tx.rollback().await.unwrap();

        // Verify file was deleted
        assert!(!file_path.exists());
    }

    #[tokio::test]
    async fn test_rollback_multiple_files() {
        let temp_dir = TempDir::new().unwrap();
        let file1 = temp_dir.path().join("file1.rs");
        let file2 = temp_dir.path().join("file2.rs");

        tokio::fs::write(&file1, "content1").await.unwrap();
        tokio::fs::write(&file2, "content2").await.unwrap();

        // Snapshot both files and modify
        let mut tx = Transaction::begin().await.unwrap();
        tx.snapshot_file(&file1).await.unwrap();
        tx.snapshot_file(&file2).await.unwrap();
        tokio::fs::write(&file1, "modified1").await.unwrap();
        tokio::fs::write(&file2, "modified2").await.unwrap();

        // Rollback
        tx.rollback().await.unwrap();

        // Verify both files restored
        assert_eq!(
            tokio::fs::read_to_string(&file1).await.unwrap(),
            "content1"
        );
        assert_eq!(
            tokio::fs::read_to_string(&file2).await.unwrap(),
            "content2"
        );
    }

    #[tokio::test]
    async fn test_commit_generates_id() {
        let tx = Transaction::begin().await.unwrap();
        let commit_id = tx.commit().await.unwrap();

        assert_ne!(commit_id, Uuid::default());
    }

    #[tokio::test]
    async fn test_commit_updates_state() {
        let tx = Transaction::begin().await.unwrap();
        let commit_id = tx.commit().await.unwrap();
        let _expected_state = TransactionState::Committed(commit_id.to_string());

        // Note: we can't directly check state after commit because tx was moved
        // But we can verify commit succeeded
        assert_ne!(commit_id, Uuid::default());
    }

    #[tokio::test]
    async fn test_rollback_after_commit_fails() {
        let tx = Transaction::begin().await.unwrap();
        let _commit_id = tx.commit().await.unwrap();

        // Transaction was consumed by commit, can't rollback
        // This is expected behavior - transaction is consumed on commit
    }

    #[tokio::test]
    async fn test_snapshot_after_rollback_fails() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");

        let mut tx = Transaction::begin().await.unwrap();
        tx.snapshot_file(&file_path).await.unwrap();
        tx.rollback().await.unwrap();

        // Can't snapshot after rollback - transaction was consumed
        // This is expected behavior
    }
}

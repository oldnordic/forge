//! Storage abstraction for checkpointing

use crate::checkpoint::{CheckpointId, CheckpointSummary, SessionId, TemporalCheckpoint};
use crate::errors::{ReasoningError, Result, StorageError};

/// Storage backend trait for checkpoints
/// 
/// Thread-safe version requires Send + Sync
pub trait CheckpointStorage: Send + Sync {
    /// Store a checkpoint
    fn store(&self, checkpoint: &TemporalCheckpoint) -> Result<()>;

    /// Retrieve a checkpoint by ID
    fn get(&self, id: CheckpointId) -> Result<TemporalCheckpoint>;

    /// Get the latest checkpoint for a session
    fn get_latest(&self, session_id: SessionId) -> Result<Option<TemporalCheckpoint>>;

    /// List all checkpoints for a session
    fn list_by_session(&self, session_id: SessionId) -> Result<Vec<CheckpointSummary>>;

    /// List checkpoints with a specific tag
    fn list_by_tag(&self, tag: &str) -> Result<Vec<CheckpointSummary>>;

    /// Delete a checkpoint
    fn delete(&self, id: CheckpointId) -> Result<()>;

    /// Get the next sequence number for a session
    fn next_sequence(&self, session_id: SessionId) -> Result<u64>;

    /// Get the maximum sequence number across all checkpoints
    fn get_max_sequence(&self) -> Result<u64>;
}

/// Storage configuration
#[derive(Clone, Debug)]
pub struct StorageConfig {
    pub path: std::path::PathBuf,
    pub backend: BackendKind,
    pub max_checkpoints: usize,
    pub compression: bool,
}

impl StorageConfig {
    pub fn sqlite(path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            path: path.into(),
            backend: BackendKind::SQLite,
            max_checkpoints: 0,
            compression: false,
        }
    }
}

/// Backend kind selection
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendKind {
    SQLite,
    NativeV3,
}

/// Factory function for creating storage backends
///
/// Creates the appropriate storage implementation based on the backend kind.
///
/// # Arguments
///
/// * `config` - Storage configuration specifying backend type and path
///
/// # Returns
///
/// A boxed trait object implementing `CheckpointStorage`
///
/// # Examples
///
/// ```no_run
/// use forge_reasoning::{StorageConfig, create_storage};
///
/// let config = StorageConfig::sqlite("/tmp/checkpoints.db");
/// let storage = create_storage(&config)?;
/// ```
pub fn create_storage(config: &StorageConfig) -> Result<Box<dyn CheckpointStorage>> {
    match config.backend {
        BackendKind::SQLite => {
            // Check if path is empty - use in-memory storage for testing
            let storage = if config.path.as_os_str().is_empty() {
                crate::storage_sqlitegraph::SqliteGraphStorage::in_memory()
                    .map_err(|e| ReasoningError::Storage(StorageError::ConnectionFailed(
                        format!("Failed to create in-memory SQLite storage: {}", e)
                    )))?
            } else {
                crate::storage_sqlitegraph::SqliteGraphStorage::open(&config.path)
                    .map_err(|e| ReasoningError::Storage(StorageError::ConnectionFailed(
                        format!("Failed to open SQLite storage at {}: {}", config.path.display(), e)
                    )))?
            };
            Ok(Box::new(storage))
        }
        BackendKind::NativeV3 => {
            // Native V3 backend is not yet implemented
            // This is a planned future enhancement
            Err(ReasoningError::Storage(StorageError::BackendNotAvailable(
                "Native V3 backend is not yet available. Use BackendKind::SQLite or check future updates.".to_string()
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkpoint::{CheckpointId, CheckpointTrigger};
    use std::path::PathBuf;

    /// Test SQLite backend creation with a file path
    #[test]
    fn test_create_storage_sqlite() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_checkpoints.db");

        let config = StorageConfig::sqlite(&db_path);
        let storage = create_storage(&config);

        assert!(storage.is_ok(), "Should create SQLite storage");
        let storage = storage.unwrap();

        // Verify it's a valid CheckpointStorage implementation
        // by calling a method that requires the trait
        let max_seq = storage.get_max_sequence();
        assert!(max_seq.is_ok(), "Should call get_max_sequence");
        assert_eq!(max_seq.unwrap(), 0, "New storage should have max sequence 0");
    }

    /// Test SQLite backend creation with empty path (in-memory)
    #[test]
    fn test_create_storage_sqlite_in_memory() {
        let config = StorageConfig {
            path: PathBuf::new(),
            backend: BackendKind::SQLite,
            max_checkpoints: 0,
            compression: false,
        };
        let storage = create_storage(&config);

        assert!(storage.is_ok(), "Should create in-memory SQLite storage");
        let storage = storage.unwrap();

        // Verify it's functional
        let max_seq = storage.get_max_sequence();
        assert!(max_seq.is_ok(), "Should call get_max_sequence");
    }

    /// Test NativeV3 backend returns NotImplemented error
    #[test]
    fn test_create_storage_native_v3_not_implemented() {
        let config = StorageConfig {
            path: PathBuf::new(),
            backend: BackendKind::NativeV3,
            max_checkpoints: 0,
            compression: false,
        };
        let result = create_storage(&config);

        match result {
            Ok(_) => panic!("Should fail for NativeV3 backend"),
            Err(ReasoningError::Storage(StorageError::BackendNotAvailable(msg))) => {
                assert!(
                    msg.contains("Native V3"),
                    "Error message should mention Native V3"
                );
                assert!(
                    msg.contains("not yet available"),
                    "Error message should indicate it's not available"
                );
            }
            Err(e) => panic!("Expected BackendNotAvailable error, got: {}", e),
        }
    }

    /// Test that the returned object implements CheckpointStorage trait
    #[test]
    fn test_storage_trait_object() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_trait_object.db");

        let config = StorageConfig::sqlite(&db_path);
        let storage = create_storage(&config).expect("Should create storage");

        // Test that all trait methods are accessible through the trait object

        // 1. Test next_sequence
        let session_id = SessionId(uuid::Uuid::new_v4());
        let seq = storage.next_sequence(session_id);
        assert!(seq.is_ok(), "next_sequence should work");

        // 2. Test get_max_sequence
        let max_seq = storage.get_max_sequence();
        assert!(max_seq.is_ok(), "get_max_sequence should work");

        // 3. Test list_by_session (empty list)
        let list = storage.list_by_session(session_id);
        assert!(list.is_ok(), "list_by_session should work");
        assert!(list.unwrap().is_empty(), "New storage should have no checkpoints");

        // 4. Test list_by_tag (empty list)
        let list = storage.list_by_tag("test-tag");
        assert!(list.is_ok(), "list_by_tag should work");
        assert!(list.unwrap().is_empty(), "New storage should have no checkpoints");

        // 5. Test get_latest (None for new session)
        let latest = storage.get_latest(session_id);
        assert!(latest.is_ok(), "get_latest should work");
        assert!(latest.unwrap().is_none(), "New storage should have no latest checkpoint");
    }

    /// Test SQLite backend can store and retrieve checkpoints
    #[test]
    fn test_storage_sqlite_store_and_retrieve() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_store_retrieve.db");

        let config = StorageConfig::sqlite(&db_path);
        let storage = create_storage(&config).expect("Should create storage");

        // Create a checkpoint
        let checkpoint_id = CheckpointId(uuid::Uuid::new_v4());
        let session_id = SessionId(uuid::Uuid::new_v4());
        let now = chrono::Utc::now();

        let checkpoint = TemporalCheckpoint {
            id: checkpoint_id,
            timestamp: now,
            sequence_number: 1,
            message: "Test checkpoint".to_string(),
            tags: vec!["test".to_string(), "factory".to_string()],
            state: crate::checkpoint::DebugStateSnapshot {
                session_id,
                started_at: now,
                checkpoint_timestamp: now,
                working_dir: None,
                env_vars: std::collections::HashMap::new(),
                metrics: crate::checkpoint::SessionMetrics::default(),
                hypothesis_state: None,
            },
            trigger: CheckpointTrigger::Manual,
            session_id,
            checksum: String::new(),
        };

        // Store the checkpoint
        let store_result = storage.store(&checkpoint);
        assert!(store_result.is_ok(), "Should store checkpoint");

        // Retrieve the checkpoint
        let retrieved = storage.get(checkpoint_id);
        assert!(retrieved.is_ok(), "Should retrieve checkpoint");
        let retrieved = retrieved.unwrap();

        assert_eq!(retrieved.id, checkpoint.id);
        assert_eq!(retrieved.message, checkpoint.message);
        assert_eq!(retrieved.sequence_number, checkpoint.sequence_number);
    }

    /// Test multiple SQLite storages can be created independently
    #[test]
    fn test_multiple_sqlite_storages() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let db_path1 = temp_dir.path().join("test1.db");
        let db_path2 = temp_dir.path().join("test2.db");

        let config1 = StorageConfig::sqlite(&db_path1);
        let config2 = StorageConfig::sqlite(&db_path2);

        let storage1 = create_storage(&config1);
        let storage2 = create_storage(&config2);

        assert!(storage1.is_ok(), "Should create first storage");
        assert!(storage2.is_ok(), "Should create second storage");

        // Verify they are independent
        let max_seq1 = storage1.unwrap().get_max_sequence().unwrap();
        let max_seq2 = storage2.unwrap().get_max_sequence().unwrap();

        assert_eq!(max_seq1, 0, "First storage should be empty");
        assert_eq!(max_seq2, 0, "Second storage should be empty");
    }
}

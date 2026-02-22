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

//! Storage abstraction for checkpointing

use crate::checkpoint::{CheckpointId, CheckpointSummary, SessionId, TemporalCheckpoint};
use crate::errors::Result;

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

/// Factory function (placeholder for future implementations)
pub fn create_storage(_config: &StorageConfig) -> Result<Box<dyn CheckpointStorage>> {
    unimplemented!("Storage factory not yet implemented")
}

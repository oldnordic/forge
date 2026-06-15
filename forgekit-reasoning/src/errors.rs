//! Error types for reasoning tools

use thiserror::Error;

/// Main error type for reasoning tools
#[derive(Error, Debug)]
pub enum ReasoningError {
    /// Storage-related errors
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    /// Checkpoint-specific errors
    #[error("Checkpoint error: {0}")]
    Checkpoint(#[from] CheckpointError),

    /// Serialization/deserialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Generic IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid state or operation
    #[error("Invalid state: {0}")]
    InvalidState(String),

    /// Not found
    #[error("Not found: {0}")]
    NotFound(String),

    /// Validation failed (checksum mismatch, etc.)
    #[error("Validation failed: {0}")]
    ValidationFailed(String),
}

/// Storage-specific errors
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Failed to connect to storage: {0}")]
    ConnectionFailed(String),

    #[error("Failed to store checkpoint: {0}")]
    StoreFailed(String),

    #[error("Failed to retrieve checkpoint: {0}")]
    RetrieveFailed(String),

    #[error("Failed to list checkpoints: {0}")]
    ListFailed(String),

    #[error("Failed to delete checkpoint: {0}")]
    DeleteFailed(String),

    #[error("Backend not available: {0}")]
    BackendNotAvailable(String),
}

/// Checkpoint-specific errors
#[derive(Error, Debug)]
pub enum CheckpointError {
    #[error("Checkpoint not found: {0}")]
    NotFound(String),

    #[error("Checkpoint already exists: {0}")]
    AlreadyExists(String),

    #[error("Failed to capture state: {0}")]
    CaptureFailed(String),

    #[error("Failed to restore state: {0}")]
    RestoreFailed(String),
}

/// Result type alias for reasoning operations
pub type Result<T> = std::result::Result<T, ReasoningError>;

/// Convert SQLiteGraph errors
impl From<sqlitegraph::SqliteGraphError> for ReasoningError {
    fn from(err: sqlitegraph::SqliteGraphError) -> Self {
        ReasoningError::Storage(StorageError::ConnectionFailed(err.to_string()))
    }
}

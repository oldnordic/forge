//! Forge Reasoning Tools
//!
//! Provides cognitive scaffolding for LLM debugging:
//! - Temporal Checkpointing: Save/restore debugging state

// Module declarations
pub mod belief;
pub mod checkpoint;
pub mod errors;
pub mod export_import;
pub mod hypothesis;
pub mod service;
pub mod storage;
pub mod storage_sqlitegraph;
pub mod thread_safe;

#[cfg(feature = "websocket")]
pub mod websocket;

// Re-export main types
pub use checkpoint::{
    AutoTrigger, CheckpointId, CheckpointSummary, CheckpointTrigger, CompactionPolicy, DebugNote,
    DebugStateSnapshot, SessionId, SessionMetrics, TemporalCheckpoint, TemporalCheckpointManager,
    VerificationResult,
};

pub use hypothesis::{
    Confidence, ConfidenceError,
    Hypothesis, HypothesisBoard, HypothesisId, HypothesisStatus,
    HypothesisStorage, InMemoryHypothesisStorage,
};

pub use belief::{BeliefGraph, ReasoningSystem};

pub use export_import::{CheckpointExporter, CheckpointImporter};

pub use errors::{CheckpointError, ReasoningError, Result, StorageError};

pub use storage::{create_storage, BackendKind, CheckpointStorage, StorageConfig};

pub use storage_sqlitegraph::SqliteGraphStorage;

pub use thread_safe::{ThreadSafeCheckpointManager, ThreadSafeStorage};

pub use service::{
    AnnotationSeverity, AutoCheckpointConfig, CheckpointAnnotation, CheckpointCommand,
    CheckpointEvent, CheckpointService, CommandResult, HealthStatus, ImportResult, ServiceMetrics,
    AnnotatedCheckpoint, ValidationReport,
};

#[cfg(feature = "websocket")]
pub use websocket::{
    CheckpointWebSocketServer, WebSocketCommand, WebSocketConfig, WebSocketEvent,
    WebSocketResponse,
};

/// Version of the reasoning tools crate
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Initialize the reasoning tools system
pub fn init() {
    tracing::info!("Forge Reasoning Tools v{}", VERSION);
}

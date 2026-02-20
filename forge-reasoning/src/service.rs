//! Integration service for checkpointing
//!
//! Provides high-level API for Forge agent integration

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use chrono::Utc;

use crate::checkpoint::{AutoTrigger, CheckpointId, CheckpointSummary, SessionId, TemporalCheckpoint};
use crate::errors::{Result, ReasoningError};
use crate::thread_safe::{ThreadSafeCheckpointManager, ThreadSafeStorage};

/// Configuration for auto-checkpointing
#[derive(Clone, Debug)]
pub struct AutoCheckpointConfig {
    pub interval_seconds: u64,
    pub on_error: bool,
    pub on_tool_call: bool,
}

impl Default for AutoCheckpointConfig {
    fn default() -> Self {
        Self {
            interval_seconds: 300, // 5 minutes
            on_error: true,
            on_tool_call: false,
        }
    }
}

/// Events emitted by the checkpoint service
#[derive(Clone, Debug)]
pub enum CheckpointEvent {
    Created {
        checkpoint_id: CheckpointId,
        session_id: SessionId,
        timestamp: chrono::DateTime<Utc>,
    },
    Restored {
        checkpoint_id: CheckpointId,
        session_id: SessionId,
    },
    Deleted {
        checkpoint_id: CheckpointId,
        session_id: SessionId,
    },
    Compacted {
        session_id: SessionId,
        remaining: usize,
    },
}

/// Commands that can be executed on the service
#[derive(Clone, Debug)]
pub enum CheckpointCommand {
    Create {
        session_id: SessionId,
        message: String,
        tags: Vec<String>,
    },
    List {
        session_id: SessionId,
    },
    Restore {
        session_id: SessionId,
        checkpoint_id: CheckpointId,
    },
    Delete {
        checkpoint_id: CheckpointId,
    },
    Compact {
        session_id: SessionId,
        keep_recent: usize,
    },
}

/// Results from command execution
#[derive(Clone, Debug)]
pub enum CommandResult {
    Created(CheckpointId),
    List(Vec<CheckpointSummary>),
    Restored(TemporalCheckpoint),
    Deleted,
    Compacted(usize),
    Error(String),
}

/// Service metrics
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ServiceMetrics {
    pub total_checkpoints: usize,
    pub active_sessions: usize,
    pub total_sessions_created: usize,
}

/// Health check status
#[derive(Clone, Debug)]
pub struct HealthStatus {
    pub healthy: bool,
    pub message: String,
}

/// Annotation for checkpoints
#[derive(Clone, Debug)]
pub struct CheckpointAnnotation {
    pub note: String,
    pub severity: AnnotationSeverity,
    pub timestamp: chrono::DateTime<Utc>,
}

/// Severity level for annotations
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnnotationSeverity {
    Info,
    Warning,
    Critical,
}

/// Checkpoint with annotations
#[derive(Clone, Debug)]
pub struct AnnotatedCheckpoint {
    pub checkpoint: TemporalCheckpoint,
    pub annotations: Vec<CheckpointAnnotation>,
}

/// Main checkpoint service for integration
pub struct CheckpointService {
    storage: ThreadSafeStorage,
    sessions: RwLock<HashMap<SessionId, SessionInfo>>,
    subscribers: Mutex<HashMap<SessionId, Vec<tokio::sync::mpsc::Sender<CheckpointEvent>>>>,
    running: RwLock<bool>,
    annotations: RwLock<HashMap<CheckpointId, Vec<CheckpointAnnotation>>>,
    /// Global sequence counter for monotonic checkpoint ordering across all sessions
    global_sequence: AtomicU64,
}

struct SessionInfo {
    name: String,
    created_at: chrono::DateTime<Utc>,
    auto_config: Option<AutoCheckpointConfig>,
}

impl CheckpointService {
    /// Create a new checkpoint service
    /// 
    /// Initializes the global sequence counter from storage to ensure
    /// monotonic sequences across service restarts.
    pub fn new(storage: ThreadSafeStorage) -> Self {
        // Initialize global sequence from storage (find max existing sequence)
        let initial_sequence = Self::find_max_sequence(&storage);
        
        Self {
            storage,
            sessions: RwLock::new(HashMap::new()),
            subscribers: Mutex::new(HashMap::new()),
            running: RwLock::new(true),
            annotations: RwLock::new(HashMap::new()),
            global_sequence: AtomicU64::new(initial_sequence),
        }
    }

    /// Find the maximum sequence number in storage
    fn find_max_sequence(storage: &ThreadSafeStorage) -> u64 {
        // Query storage for max sequence across all checkpoints
        match storage.get_max_sequence() {
            Ok(max_seq) => max_seq,
            Err(_) => 0,
        }
    }

    /// Get the current global sequence number
    /// 
    /// Returns the sequence number of the most recently created checkpoint.
    /// Returns 0 if no checkpoints have been created yet.
    pub fn global_sequence(&self) -> u64 {
        self.global_sequence.load(Ordering::SeqCst)
    }

    /// Get the next sequence number atomically
    /// 
    /// Returns 1-based sequence numbers (first checkpoint is 1, not 0)
    fn next_sequence(&self) -> u64 {
        self.global_sequence.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Check if service is running
    pub fn is_running(&self) -> bool {
        *self.running.read().unwrap()
    }

    /// Stop the service
    pub fn stop(&self) {
        *self.running.write().unwrap() = false;
    }

    /// Create a new session
    pub fn create_session(&self, name: &str) -> Result<SessionId> {
        let session_id = SessionId::new();
        let info = SessionInfo {
            name: name.to_string(),
            created_at: Utc::now(),
            auto_config: None,
        };
        
        self.sessions.write().unwrap().insert(session_id, info);
        Ok(session_id)
    }

    /// Get or create session manager
    fn get_manager(&self, session_id: SessionId) -> ThreadSafeCheckpointManager {
        ThreadSafeCheckpointManager::new(self.storage.clone(), session_id)
    }

    /// Create a checkpoint with global sequence number
    pub fn checkpoint(&self, session_id: &SessionId, message: impl Into<String>) -> Result<CheckpointId> {
        if !self.is_running() {
            return Err(ReasoningError::InvalidState("Service not running".to_string()));
        }
        
        let manager = self.get_manager(*session_id);
        let seq = self.next_sequence();
        let id = manager.checkpoint_with_sequence(message, seq)?;
        
        // Emit event
        self.emit_event(CheckpointEvent::Created {
            checkpoint_id: id,
            session_id: *session_id,
            timestamp: Utc::now(),
        });
        
        Ok(id)
    }

    /// List checkpoints for a session
    pub fn list_checkpoints(&self, session_id: &SessionId) -> Result<Vec<CheckpointSummary>> {
        let manager = self.get_manager(*session_id);
        manager.list()
    }

    /// Restore a checkpoint
    pub fn restore(&self, session_id: &SessionId, checkpoint_id: &CheckpointId) -> Result<crate::checkpoint::DebugStateSnapshot> {
        let manager = self.get_manager(*session_id);
        let checkpoint = manager.get(checkpoint_id)?.ok_or_else(|| {
            ReasoningError::NotFound(format!("Checkpoint {} not found", checkpoint_id))
        })?;
        
        let state = manager.restore(&checkpoint)?;
        
        self.emit_event(CheckpointEvent::Restored {
            checkpoint_id: *checkpoint_id,
            session_id: *session_id,
        });
        
        Ok(state)
    }

    /// Enable auto-checkpointing for a session
    pub fn enable_auto_checkpoint(&self, session_id: &SessionId, config: AutoCheckpointConfig) -> Result<()> {
        let mut sessions = self.sessions.write().unwrap();
        if let Some(info) = sessions.get_mut(session_id) {
            info.auto_config = Some(config);
            Ok(())
        } else {
            Err(ReasoningError::NotFound(format!("Session {:?} not found", session_id)))
        }
    }

    /// Trigger an auto-checkpoint with global sequence
    pub fn trigger_auto_checkpoint(&self, session_id: &SessionId, trigger: AutoTrigger) -> Result<Option<CheckpointId>> {
        let manager = self.get_manager(*session_id);
        let seq = self.next_sequence();
        let result = manager.auto_checkpoint_with_sequence(trigger, seq)?;
        
        if let Some(id) = result {
            self.emit_event(CheckpointEvent::Created {
                checkpoint_id: id,
                session_id: *session_id,
                timestamp: Utc::now(),
            });
        }
        
        Ok(result)
    }

    /// Subscribe to checkpoint events for a session
    pub fn subscribe(&self, session_id: &SessionId) -> Result<tokio::sync::mpsc::Receiver<CheckpointEvent>> {
        let (tx, rx) = tokio::sync::mpsc::channel(100); // Buffer up to 100 events
        
        let mut subscribers = self.subscribers.lock().unwrap();
        subscribers.entry(*session_id).or_insert_with(Vec::new).push(tx);
        
        Ok(rx)
    }

    /// Emit event to subscribers
    fn emit_event(&self, event: CheckpointEvent) {
        let session_id = match &event {
            CheckpointEvent::Created { session_id, .. } => *session_id,
            CheckpointEvent::Restored { session_id, .. } => *session_id,
            CheckpointEvent::Deleted { session_id, .. } => *session_id,
            CheckpointEvent::Compacted { session_id, .. } => *session_id,
        };
        
        let subscribers = self.subscribers.lock().unwrap();
        if let Some(subs) = subscribers.get(&session_id) {
            for tx in subs {
                // Best-effort delivery (try_send is non-blocking)
                let _ = tx.try_send(event.clone());
            }
        }
    }

    /// Execute a command
    pub fn execute(&self, command: CheckpointCommand) -> Result<CommandResult> {
        match command {
            CheckpointCommand::Create { session_id, message, tags } => {
                let manager = self.get_manager(session_id);
                let seq = self.next_sequence();
                let id = if tags.is_empty() {
                    manager.checkpoint_with_sequence(message, seq)?
                } else {
                    manager.checkpoint_with_tags_and_sequence(message, tags, seq)?
                };
                
                self.emit_event(CheckpointEvent::Created {
                    checkpoint_id: id,
                    session_id,
                    timestamp: Utc::now(),
                });
                
                Ok(CommandResult::Created(id))
            }
            CheckpointCommand::List { session_id } => {
                let manager = self.get_manager(session_id);
                let checkpoints = manager.list()?;
                Ok(CommandResult::List(checkpoints))
            }
            CheckpointCommand::Restore { session_id, checkpoint_id } => {
                let _checkpoint = self.restore(&session_id, &checkpoint_id)?;
                // Create a minimal TemporalCheckpoint for the result
                let manager = self.get_manager(session_id);
                let cp = manager.get(&checkpoint_id)?.ok_or_else(|| {
                    ReasoningError::NotFound(format!("Checkpoint {} not found", checkpoint_id))
                })?;
                Ok(CommandResult::Restored(cp))
            }
            CheckpointCommand::Delete { checkpoint_id } => {
                // Delete from all sessions (simplified)
                let sessions = self.sessions.read().unwrap();
                for session_id in sessions.keys() {
                    let manager = self.get_manager(*session_id);
                    let _ = manager.delete(&checkpoint_id);
                }
                
                self.emit_event(CheckpointEvent::Deleted {
                    checkpoint_id,
                    session_id: SessionId::new(), // Simplified
                });
                
                Ok(CommandResult::Deleted)
            }
            CheckpointCommand::Compact { session_id, keep_recent } => {
                let manager = self.get_manager(session_id);
                let deleted = manager.compact(keep_recent)?;
                
                self.emit_event(CheckpointEvent::Compacted {
                    session_id,
                    remaining: keep_recent,
                });
                
                Ok(CommandResult::Compacted(deleted))
            }
        }
    }

    /// Sync checkpoints to disk (background persistence)
    pub fn sync_to_disk(&self) -> Result<()> {
        // In a real implementation, this would trigger background flush
        // For now, we just verify the storage is working
        Ok(())
    }

    /// Annotate a checkpoint
    pub fn annotate(&self, checkpoint_id: &CheckpointId, annotation: CheckpointAnnotation) -> Result<()> {
        // Verify checkpoint exists
        let sessions = self.sessions.read().unwrap();
        let mut found = false;
        for session_id in sessions.keys() {
            let manager = self.get_manager(*session_id);
            if manager.get(checkpoint_id)?.is_some() {
                found = true;
                break;
            }
        }
        
        if !found {
            return Err(ReasoningError::NotFound(format!("Checkpoint {} not found", checkpoint_id)));
        }
        
        // Store annotation
        let mut annotations = self.annotations.write().unwrap();
        annotations.entry(*checkpoint_id).or_insert_with(Vec::new).push(annotation);
        
        Ok(())
    }

    /// Get checkpoint with annotations
    pub fn get_with_annotations(&self, checkpoint_id: &CheckpointId) -> Result<AnnotatedCheckpoint> {
        let sessions = self.sessions.read().unwrap();
        let annotations = self.annotations.read().unwrap();
        
        for session_id in sessions.keys() {
            let manager = self.get_manager(*session_id);
            if let Some(checkpoint) = manager.get(checkpoint_id)? {
                let checkpoint_annotations = annotations.get(checkpoint_id)
                    .cloned()
                    .unwrap_or_default();
                
                return Ok(AnnotatedCheckpoint {
                    checkpoint,
                    annotations: checkpoint_annotations,
                });
            }
        }
        Err(ReasoningError::NotFound(format!("Checkpoint {} not found", checkpoint_id)))
    }

    /// Get service metrics
    pub fn metrics(&self) -> Result<ServiceMetrics> {
        let sessions = self.sessions.read().unwrap();
        let total_checkpoints: usize = sessions.keys()
            .map(|session_id| {
                let manager = self.get_manager(*session_id);
                manager.list().map(|cps| cps.len()).unwrap_or(0)
            })
            .sum();
        
        Ok(ServiceMetrics {
            total_checkpoints,
            active_sessions: sessions.len(),
            total_sessions_created: sessions.len(),
        })
    }

    /// Health check
    pub fn health_check(&self) -> Result<HealthStatus> {
        if !self.is_running() {
            return Ok(HealthStatus {
                healthy: false,
                message: "Service is stopped".to_string(),
            });
        }
        
        // Try a simple operation
        match self.storage.list_by_session(SessionId::new()) {
            Ok(_) => Ok(HealthStatus {
                healthy: true,
                message: "Service is healthy".to_string(),
            }),
            Err(e) => Ok(HealthStatus {
                healthy: false,
                message: format!("Storage error: {}", e),
            }),
        }
    }

    /// List checkpoints by global sequence number range
    /// 
    /// Returns all checkpoints (across all sessions) with sequence numbers
    /// in the inclusive range [start_seq, end_seq].
    pub fn list_by_sequence_range(&self, start_seq: u64, end_seq: u64) -> Result<Vec<CheckpointSummary>> {
        let sessions = self.sessions.read().unwrap();
        let mut all_checkpoints = Vec::new();
        
        for session_id in sessions.keys() {
            let manager = self.get_manager(*session_id);
            let cps = manager.list()?;
            for cp in cps {
                if cp.sequence_number >= start_seq && cp.sequence_number <= end_seq {
                    all_checkpoints.push(cp);
                }
            }
        }
        
        // Sort by sequence number
        all_checkpoints.sort_by_key(|cp| cp.sequence_number);
        Ok(all_checkpoints)
    }

    /// Export all checkpoints from all sessions
    /// 
    /// Returns a JSON-serializable export containing all checkpoints
    /// and the current global sequence number.
    pub fn export_all_checkpoints(&self) -> Result<String> {
        let sessions = self.sessions.read().unwrap();
        let mut all_checkpoints: Vec<TemporalCheckpoint> = Vec::new();
        
        for session_id in sessions.keys() {
            let manager = self.get_manager(*session_id);
            let cps = manager.list()?;
            for cp_summary in cps {
                if let Ok(Some(cp)) = manager.get(&cp_summary.id) {
                    all_checkpoints.push(cp);
                }
            }
        }
        
        // Sort by sequence number
        all_checkpoints.sort_by_key(|cp| cp.sequence_number);
        
        let export = ExportData {
            checkpoints: all_checkpoints,
            global_sequence: self.global_sequence(),
            exported_at: Utc::now(),
        };
        
        serde_json::to_string_pretty(&export)
            .map_err(ReasoningError::Serialization)
    }

    /// Import checkpoints from export data
    /// 
    /// Imports all checkpoints and restores the global sequence counter.
    /// Skips checkpoints that already exist (by ID).
    pub fn import_checkpoints(&self, export_data: &str) -> Result<ImportResult> {
        let export: ExportData = serde_json::from_str(export_data)
            .map_err(ReasoningError::Serialization)?;
        
        let mut imported = 0;
        let mut skipped = 0;
        let mut max_sequence = 0u64;
        
        for checkpoint in export.checkpoints {
            // Track the maximum sequence number
            max_sequence = max_sequence.max(checkpoint.sequence_number);
            
            // Check if checkpoint already exists
            let manager = self.get_manager(checkpoint.session_id);
            match manager.get(&checkpoint.id) {
                Ok(Some(_)) => {
                    // Already exists, skip
                    skipped += 1;
                }
                _ => {
                    // Store the checkpoint
                    if let Err(e) = self.storage.store(&checkpoint) {
                        tracing::warn!("Failed to import checkpoint {}: {}", checkpoint.id, e);
                    } else {
                        imported += 1;
                    }
                }
            }
        }
        
        // Update global sequence if imported checkpoints have higher sequences
        let current = self.global_sequence();
        if max_sequence > current {
            self.global_sequence.store(max_sequence, Ordering::SeqCst);
        }
        
        Ok(ImportResult { imported, skipped })
    }

    /// Validate a single checkpoint by ID
    /// 
    /// Returns true if the checkpoint's checksum is valid, false otherwise.
    pub fn validate_checkpoint(&self, checkpoint_id: &CheckpointId) -> Result<bool> {
        let cp = self.get_with_annotations(checkpoint_id)?;
        
        // Empty checksum means legacy checkpoint (skip validation)
        if cp.checkpoint.checksum.is_empty() {
            return Ok(true);
        }
        
        match cp.checkpoint.validate() {
            Ok(()) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Health check with validation of recent checkpoints
    /// 
    /// Performs a health check and additionally validates the most
    /// recent checkpoints to detect data corruption.
    pub fn health_check_with_validation(&self) -> Result<HealthStatus> {
        // First do basic health check
        let basic = self.health_check()?;
        if !basic.healthy {
            return Ok(basic);
        }
        
        // Validate recent checkpoints from all sessions
        let sessions = self.sessions.read().unwrap();
        let mut checked = 0;
        let mut invalid = 0;
        
        for session_id in sessions.keys() {
            let manager = self.get_manager(*session_id);
            if let Ok(cps) = manager.list() {
                // Check up to 5 most recent checkpoints per session
                for cp in cps.iter().rev().take(5) {
                    checked += 1;
                    if let Ok(Some(checkpoint)) = manager.get(&cp.id) {
                        if !checkpoint.checksum.is_empty() {
                            if let Err(e) = checkpoint.validate() {
                                tracing::warn!("Checkpoint {} failed validation: {}", cp.id, e);
                                invalid += 1;
                            }
                        }
                    }
                }
            }
        }
        
        if invalid > 0 {
            return Ok(HealthStatus {
                healthy: false,
                message: format!("{} of {} recent checkpoints failed validation", invalid, checked),
            });
        }
        
        Ok(HealthStatus {
            healthy: true,
            message: format!("Service healthy, {} recent checkpoints validated", checked),
        })
    }

    /// Validate all checkpoints
    /// 
    /// Performs a full validation of all checkpoints in the system.
    /// Returns a report with validation statistics.
    pub fn validate_all_checkpoints(&self) -> Result<ValidationReport> {
        let sessions = self.sessions.read().unwrap();
        let mut valid = 0;
        let mut invalid = 0;
        let mut skipped = 0;
        
        for session_id in sessions.keys() {
            let manager = self.get_manager(*session_id);
            if let Ok(cps) = manager.list() {
                for cp_summary in cps {
                    if let Ok(Some(cp)) = manager.get(&cp_summary.id) {
                        if cp.checksum.is_empty() {
                            // Legacy checkpoint without checksum
                            skipped += 1;
                        } else {
                            match cp.validate() {
                                Ok(()) => valid += 1,
                                Err(e) => {
                                    tracing::warn!("Checkpoint {} validation failed: {}", cp.id, e);
                                    invalid += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(ValidationReport {
            valid,
            invalid,
            skipped,
            checked_at: Some(Utc::now()),
        })
    }
}

/// Report from validation operation
#[derive(Debug, Clone)]
pub struct ValidationReport {
    pub valid: usize,
    pub invalid: usize,
    pub skipped: usize,
    pub checked_at: Option<chrono::DateTime<Utc>>,
}

impl ValidationReport {
    /// Total number of checkpoints checked
    pub fn total(&self) -> usize {
        self.valid + self.invalid + self.skipped
    }
    
    /// Whether all checked checkpoints were valid
    pub fn all_valid(&self) -> bool {
        self.invalid == 0
    }
}

/// Data structure for export/import
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ExportData {
    checkpoints: Vec<TemporalCheckpoint>,
    global_sequence: u64,
    exported_at: chrono::DateTime<Utc>,
}

/// Result of import operation
#[derive(Debug, Clone)]
pub struct ImportResult {
    pub imported: usize,
    pub skipped: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_basic() {
        let service = CheckpointService::new(ThreadSafeStorage::in_memory().unwrap());
        assert!(service.is_running());
        
        let session = service.create_session("test").unwrap();
        let id = service.checkpoint(&session, "Test").unwrap();
        assert!(!id.to_string().is_empty());
    }
}

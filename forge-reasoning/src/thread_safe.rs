//! Thread-safe implementations for concurrent checkpointing
//!
//! Provides `Arc<Mutex<>>` wrappers for storage and manager
//! to enable multi-threaded checkpoint operations.

use std::path::Path;
use std::sync::{Arc, Mutex};

use chrono::Utc;

use crate::checkpoint::{
    CheckpointId, CheckpointSummary, CompactionPolicy, DebugStateSnapshot, 
    SessionId, TemporalCheckpoint
};
use crate::errors::Result;
use crate::storage::CheckpointStorage;
use crate::SqliteGraphStorage;

/// Thread-safe wrapper around any CheckpointStorage
pub struct ThreadSafeStorage {
    inner: Arc<Mutex<Box<dyn CheckpointStorage>>>,
}

impl ThreadSafeStorage {
    /// Create from existing storage
    pub fn new<S: CheckpointStorage + 'static>(storage: S) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Box::new(storage))),
        }
    }

    /// Create in-memory thread-safe storage
    pub fn in_memory() -> Result<Self> {
        let storage = SqliteGraphStorage::in_memory()?;
        Ok(Self::new(storage))
    }

    /// Create file-based thread-safe storage
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let storage = SqliteGraphStorage::open(path)?;
        Ok(Self::new(storage))
    }

    /// Store a checkpoint
    pub fn store(&self, checkpoint: &TemporalCheckpoint) -> Result<()> {
        let storage = self.inner.lock().expect("Storage lock poisoned");
        storage.store(checkpoint)
    }

    /// Get checkpoint by ID
    pub fn get(&self, id: CheckpointId) -> Result<TemporalCheckpoint> {
        let storage = self.inner.lock().expect("Storage lock poisoned");
        storage.get(id)
    }

    /// Get latest checkpoint for session
    pub fn get_latest(&self, session_id: SessionId) -> Result<Option<TemporalCheckpoint>> {
        let storage = self.inner.lock().expect("Storage lock poisoned");
        storage.get_latest(session_id)
    }

    /// List checkpoints by session
    pub fn list_by_session(&self, session_id: SessionId) -> Result<Vec<CheckpointSummary>> {
        let storage = self.inner.lock().expect("Storage lock poisoned");
        storage.list_by_session(session_id)
    }

    /// List checkpoints by tag
    pub fn list_by_tag(&self, tag: &str) -> Result<Vec<CheckpointSummary>> {
        let storage = self.inner.lock().expect("Storage lock poisoned");
        storage.list_by_tag(tag)
    }

    /// Delete checkpoint
    pub fn delete(&self, id: CheckpointId) -> Result<()> {
        let storage = self.inner.lock().expect("Storage lock poisoned");
        storage.delete(id)
    }

    /// Get maximum sequence number across all checkpoints
    pub fn get_max_sequence(&self) -> Result<u64> {
        let storage = self.inner.lock().expect("Storage lock poisoned");
        storage.get_max_sequence()
    }
}

impl Clone for ThreadSafeStorage {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

// Safety: ThreadSafeStorage uses Arc<Mutex<>> internally
unsafe impl Send for ThreadSafeStorage {}
unsafe impl Sync for ThreadSafeStorage {}

/// Thread-safe checkpoint manager
/// 
/// Wraps operations in Mutex for concurrent access
pub struct ThreadSafeCheckpointManager {
    storage: ThreadSafeStorage,
    session_id: SessionId,
    sequence_counter: Mutex<u64>,
    last_checkpoint_time: Mutex<chrono::DateTime<Utc>>,
}

impl ThreadSafeCheckpointManager {
    /// Create a new thread-safe manager
    pub fn new(storage: ThreadSafeStorage, session_id: SessionId) -> Self {
        Self {
            storage,
            session_id,
            sequence_counter: Mutex::new(0),
            last_checkpoint_time: Mutex::new(Utc::now()),
        }
    }

    /// Create a manual checkpoint with auto-generated sequence
    pub fn checkpoint(&self, message: impl Into<String>) -> Result<CheckpointId> {
        let seq = {
            let mut counter = self.sequence_counter.lock().expect("Counter poisoned");
            *counter += 1;
            *counter
        };
        self.checkpoint_with_sequence(message, seq)
    }

    /// Create a checkpoint with a specific sequence number (for global sequencing)
    pub fn checkpoint_with_sequence(
        &self,
        message: impl Into<String>,
        sequence: u64,
    ) -> Result<CheckpointId> {
        let state = self.capture_state()?;

        let checkpoint = TemporalCheckpoint::new(
            sequence,
            message,
            state,
            crate::checkpoint::CheckpointTrigger::Manual,
            self.session_id,
        );

        self.storage.store(&checkpoint)?;
        self.update_last_checkpoint_time();

        // Update local counter to track sequences
        let mut counter = self.sequence_counter.lock().expect("Counter poisoned");
        *counter = (*counter).max(sequence);

        Ok(checkpoint.id)
    }

    /// Create a checkpoint with tags and auto-generated sequence
    pub fn checkpoint_with_tags(
        &self,
        message: impl Into<String>,
        tags: Vec<String>,
    ) -> Result<CheckpointId> {
        let seq = {
            let mut counter = self.sequence_counter.lock().expect("Counter poisoned");
            *counter += 1;
            *counter
        };
        self.checkpoint_with_tags_and_sequence(message, tags, seq)
    }

    /// Create a checkpoint with tags and specific sequence number
    pub fn checkpoint_with_tags_and_sequence(
        &self,
        message: impl Into<String>,
        tags: Vec<String>,
        sequence: u64,
    ) -> Result<CheckpointId> {
        let state = self.capture_state()?;

        let mut checkpoint = TemporalCheckpoint::new(
            sequence,
            message,
            state,
            crate::checkpoint::CheckpointTrigger::Manual,
            self.session_id,
        );
        checkpoint.tags = tags;

        self.storage.store(&checkpoint)?;
        self.update_last_checkpoint_time();

        // Update local counter to track sequences
        let mut counter = self.sequence_counter.lock().expect("Counter poisoned");
        *counter = (*counter).max(sequence);

        Ok(checkpoint.id)
    }

    /// Create an automatic checkpoint with auto-generated sequence
    pub fn auto_checkpoint(&self, trigger: crate::checkpoint::AutoTrigger) -> Result<Option<CheckpointId>> {
        let should_checkpoint = match trigger {
            crate::checkpoint::AutoTrigger::SignificantTimePassed => {
                let last = *self.last_checkpoint_time.lock().expect("Time lock poisoned");
                Utc::now().signed_duration_since(last).num_minutes() > 5
            }
            _ => true,
        };

        if !should_checkpoint {
            return Ok(None);
        }

        let seq = {
            let mut counter = self.sequence_counter.lock().expect("Counter poisoned");
            *counter += 1;
            *counter
        };
        
        self.auto_checkpoint_with_sequence(trigger, seq)
    }

    /// Create an automatic checkpoint with specific sequence number
    pub fn auto_checkpoint_with_sequence(
        &self,
        trigger: crate::checkpoint::AutoTrigger,
        sequence: u64,
    ) -> Result<Option<CheckpointId>> {
        let state = self.capture_state()?;

        let checkpoint = TemporalCheckpoint::new(
            sequence,
            format!("Auto: {:?}", trigger),
            state,
            crate::checkpoint::CheckpointTrigger::Automatic(trigger),
            self.session_id,
        );

        self.storage.store(&checkpoint)?;
        self.update_last_checkpoint_time();

        // Update local counter to track sequences
        let mut counter = self.sequence_counter.lock().expect("Counter poisoned");
        *counter = (*counter).max(sequence);

        Ok(Some(checkpoint.id))
    }

    /// List all checkpoints for this session
    pub fn list(&self) -> Result<Vec<CheckpointSummary>> {
        self.storage.list_by_session(self.session_id)
    }

    /// Get a checkpoint by ID
    pub fn get(&self, id: &CheckpointId) -> Result<Option<TemporalCheckpoint>> {
        match self.storage.get(*id) {
            Ok(cp) => Ok(Some(cp)),
            Err(_) => Ok(None),
        }
    }

    /// List checkpoints for a specific session
    pub fn list_by_session(&self, session_id: &SessionId) -> Result<Vec<CheckpointSummary>> {
        self.storage.list_by_session(*session_id)
    }

    /// List checkpoints with a specific tag
    pub fn list_by_tag(&self, tag: &str) -> Result<Vec<CheckpointSummary>> {
        self.storage.list_by_tag(tag)
    }

    /// Delete a checkpoint
    pub fn delete(&self, id: &CheckpointId) -> Result<()> {
        self.storage.delete(*id)
    }

    /// Compact checkpoints
    pub fn compact(&self, keep_recent: usize) -> Result<usize> {
        self.compact_with_policy(CompactionPolicy::KeepRecent(keep_recent))
    }

    /// Compact with policy
    pub fn compact_with_policy(&self, policy: CompactionPolicy) -> Result<usize> {
        let all_checkpoints = self.storage.list_by_session(self.session_id)?;
        
        // Determine which checkpoints to keep
        let ids_to_keep: std::collections::HashSet<CheckpointId> = match &policy {
            CompactionPolicy::KeepRecent(n) => {
                let mut sorted = all_checkpoints.clone();
                sorted.sort_by_key(|cp| cp.sequence_number);
                sorted.iter().rev().take(*n).map(|cp| cp.id).collect()
            }
            CompactionPolicy::PreserveTagged(tags) => {
                all_checkpoints.iter()
                    .filter(|cp| cp.tags.iter().any(|t| tags.contains(t)))
                    .map(|cp| cp.id)
                    .collect()
            }
            CompactionPolicy::Hybrid { keep_recent, preserve_tags } => {
                let mut to_keep = std::collections::HashSet::new();
                
                let mut sorted = all_checkpoints.clone();
                sorted.sort_by_key(|cp| cp.sequence_number);
                for cp in sorted.iter().rev().take(*keep_recent) {
                    to_keep.insert(cp.id);
                }
                
                for cp in &all_checkpoints {
                    if cp.tags.iter().any(|t| preserve_tags.contains(t)) {
                        to_keep.insert(cp.id);
                    }
                }
                
                to_keep
            }
        };
        
        // Delete checkpoints not in keep list
        let mut deleted = 0;
        for cp in &all_checkpoints {
            if !ids_to_keep.contains(&cp.id) {
                self.storage.delete(cp.id)?;
                deleted += 1;
            }
        }
        
        Ok(deleted)
    }

    /// Restore state from checkpoint
    pub fn restore(&self, checkpoint: &TemporalCheckpoint) -> Result<DebugStateSnapshot> {
        if checkpoint.state.working_dir.is_none() {
            return Err(crate::errors::ReasoningError::InvalidState(
                "Checkpoint has no working directory".to_string()
            ));
        }
        Ok(checkpoint.state.clone())
    }

    /// Get summary by ID
    pub fn get_summary(&self, id: &CheckpointId) -> Result<Option<CheckpointSummary>> {
        match self.storage.get(*id) {
            Ok(cp) => Ok(Some(CheckpointSummary {
                id: cp.id,
                timestamp: cp.timestamp,
                sequence_number: cp.sequence_number,
                message: cp.message,
                trigger: cp.trigger.to_string(),
                tags: cp.tags,
                has_notes: false,
            })),
            Err(_) => Ok(None),
        }
    }

    fn capture_state(&self) -> Result<DebugStateSnapshot> {
        Ok(DebugStateSnapshot {
            session_id: self.session_id,
            started_at: Utc::now(),
            checkpoint_timestamp: Utc::now(),
            working_dir: std::env::current_dir().ok(),
            env_vars: std::env::vars().collect(),
            metrics: crate::checkpoint::SessionMetrics::default(),
        })
    }

    fn update_last_checkpoint_time(&self) {
        *self.last_checkpoint_time.lock().expect("Time lock poisoned") = Utc::now();
    }
}

// Safety: ThreadSafeCheckpointManager uses Mutex internally
unsafe impl Send for ThreadSafeCheckpointManager {}
unsafe impl Sync for ThreadSafeCheckpointManager {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thread_safe_storage_creation() {
        let storage = ThreadSafeStorage::in_memory().unwrap();
        // Just verify it doesn't panic
        let _ = storage.list_by_session(SessionId::new());
    }

    #[test]
    fn test_thread_safe_manager_creation() {
        let storage = ThreadSafeStorage::in_memory().unwrap();
        let session_id = SessionId::new();
        let manager = ThreadSafeCheckpointManager::new(storage, session_id);
        
        // Should be able to create checkpoint
        let id = manager.checkpoint("Test").unwrap();
        assert!(!id.to_string().is_empty());
    }
}

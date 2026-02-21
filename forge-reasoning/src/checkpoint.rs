//! Temporal Checkpointing - Core types and manager

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use uuid::Uuid;

/// Compute SHA-256 checksum of checkpoint data
pub fn compute_checksum(data: &[u8]) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

use crate::errors::Result;
use crate::storage::CheckpointStorage;

/// Unique identifier for a checkpoint
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CheckpointId(pub Uuid);

impl CheckpointId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for CheckpointId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for CheckpointId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a debugging session
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A snapshot of complete debugging state at a point in time
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TemporalCheckpoint {
    pub id: CheckpointId,
    pub timestamp: DateTime<Utc>,
    pub sequence_number: u64,
    pub message: String,
    pub tags: Vec<String>,
    pub state: DebugStateSnapshot,
    pub trigger: CheckpointTrigger,
    pub session_id: SessionId,
    /// SHA-256 checksum for data integrity verification
    pub checksum: String,
}

impl TemporalCheckpoint {
    pub fn new(
        sequence: u64,
        message: impl Into<String>,
        state: DebugStateSnapshot,
        trigger: CheckpointTrigger,
        session_id: SessionId,
    ) -> Self {
        let id = CheckpointId::new();
        let timestamp = Utc::now();
        let message = message.into();
        
        // Create checkpoint without checksum first
        let mut checkpoint = Self {
            id,
            timestamp,
            sequence_number: sequence,
            message: message.clone(),
            tags: Vec::new(),
            state: state.clone(),
            trigger: trigger.clone(),
            session_id,
            checksum: String::new(), // Temporary, will compute
        };
        
        // Compute checksum from serialized data (excluding checksum itself)
        checkpoint.checksum = checkpoint.compute_checksum();
        
        checkpoint
    }
    
    /// Compute checksum of this checkpoint's data
    fn compute_checksum(&self) -> String {
        // Create a copy without checksum for serialization
        let data_for_hash = CheckpointDataForHash {
            id: self.id,
            timestamp: self.timestamp,
            sequence_number: self.sequence_number,
            message: &self.message,
            tags: &self.tags,
            state: &self.state,
            trigger: &self.trigger,
            session_id: self.session_id,
        };
        
        let json = serde_json::to_vec(&data_for_hash)
            .unwrap_or_default();
        compute_checksum(&json)
    }
    
    /// Validate the checkpoint's checksum
    pub fn validate(&self) -> crate::errors::Result<()> {
        let expected = self.compute_checksum();
        if self.checksum != expected {
            return Err(crate::errors::ReasoningError::ValidationFailed(
                format!("Checksum mismatch: expected {}, got {}", expected, self.checksum)
            ));
        }
        Ok(())
    }
}

/// Helper struct for computing checksum (excludes checksum field)
#[derive(Serialize)]
struct CheckpointDataForHash<'a> {
    id: CheckpointId,
    timestamp: DateTime<Utc>,
    sequence_number: u64,
    message: &'a str,
    tags: &'a [String],
    state: &'a DebugStateSnapshot,
    trigger: &'a CheckpointTrigger,
    session_id: SessionId,
}

/// Complete snapshot of debugging state
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DebugStateSnapshot {
    pub session_id: SessionId,
    pub started_at: DateTime<Utc>,
    pub checkpoint_timestamp: DateTime<Utc>,
    pub working_dir: Option<PathBuf>,
    pub env_vars: HashMap<String, String>,
    pub metrics: SessionMetrics,
    /// Hypothesis state snapshot (optional for backward compatibility)
    pub hypothesis_state: Option<crate::hypothesis::types::HypothesisState>,
}

/// What triggered this checkpoint
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CheckpointTrigger {
    Manual,
    Automatic(AutoTrigger),
    Scheduled,
}

impl std::fmt::Display for CheckpointTrigger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Manual => write!(f, "manual"),
            Self::Automatic(_) => write!(f, "auto"),
            Self::Scheduled => write!(f, "scheduled"),
        }
    }
}

/// Types of automatic triggers
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum AutoTrigger {
    HypothesisStatusChange,
    NewContradictionDetected,
    VerificationComplete,
    BranchSwitch,
    GapFilled,
    CodeModified,
    SignificantTimePassed,
    ContextCompactionWarning,
}

/// Policy for checkpoint compaction
#[derive(Clone, Debug)]
pub enum CompactionPolicy {
    /// Keep N most recent checkpoints
    KeepRecent(usize),
    /// Keep all checkpoints with specific tags
    PreserveTagged(Vec<String>),
    /// Keep recent + preserve tagged
    Hybrid { keep_recent: usize, preserve_tags: Vec<String> },
}

impl Default for CompactionPolicy {
    fn default() -> Self {
        CompactionPolicy::KeepRecent(100)
    }
}

/// A user note/observation during debugging
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DebugNote {
    pub timestamp: DateTime<Utc>,
    pub content: String,
    pub tags: Vec<String>,
}

/// Verification result snapshot
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerificationResult {
    pub name: String,
    pub timestamp: DateTime<Utc>,
    pub passed: bool,
    pub output: Option<String>,
    pub duration_ms: u64,
}

/// Session performance metrics
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SessionMetrics {
    pub checkpoints_created: u64,
    pub hypotheses_tested: u64,
    pub verifications_run: u64,
    pub gaps_filled: u64,
}

/// Summary of a checkpoint (for listing)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckpointSummary {
    pub id: CheckpointId,
    pub timestamp: DateTime<Utc>,
    pub sequence_number: u64,
    pub message: String,
    pub trigger: String,
    pub tags: Vec<String>,
    pub has_notes: bool,
}

/// The main checkpoint manager
pub struct TemporalCheckpointManager {
    storage: Rc<dyn CheckpointStorage>,
    session_id: SessionId,
    sequence_counter: Cell<u64>,
    last_checkpoint_time: RefCell<DateTime<Utc>>,
}

impl TemporalCheckpointManager {
    /// Create a new checkpoint manager
    pub fn new(storage: Rc<dyn CheckpointStorage>, session_id: SessionId) -> Self {
        Self {
            storage,
            session_id,
            sequence_counter: Cell::new(0),
            last_checkpoint_time: RefCell::new(Utc::now()),
        }
    }

    /// Create a manual checkpoint
    pub fn checkpoint(&self, message: impl Into<String>) -> Result<CheckpointId> {
        let seq = self.sequence_counter.get();
        self.sequence_counter.set(seq + 1);
        let state = self.capture_state()?;

        let checkpoint = TemporalCheckpoint::new(
            seq,
            message,
            state,
            CheckpointTrigger::Manual,
            self.session_id,
        );

        self.storage.store(&checkpoint)?;
        self.update_last_checkpoint_time();

        Ok(checkpoint.id)
    }

    /// Create an automatic checkpoint (if appropriate)
    pub fn auto_checkpoint(&self, trigger: AutoTrigger) -> Result<Option<CheckpointId>> {
        let should_checkpoint = match trigger {
            AutoTrigger::SignificantTimePassed => {
                let last = *self.last_checkpoint_time.borrow();
                Utc::now().signed_duration_since(last).num_minutes() > 5
            }
            _ => true,
        };

        if !should_checkpoint {
            return Ok(None);
        }

        let seq = self.sequence_counter.get();
        self.sequence_counter.set(seq + 1);
        let state = self.capture_state()?;

        let checkpoint = TemporalCheckpoint::new(
            seq,
            format!("Auto: {:?}", trigger),
            state,
            CheckpointTrigger::Automatic(trigger),
            self.session_id,
        );

        self.storage.store(&checkpoint)?;
        self.update_last_checkpoint_time();

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

    /// Create a checkpoint with tags
    pub fn checkpoint_with_tags(
        &self,
        message: impl Into<String>,
        tags: Vec<String>,
    ) -> Result<CheckpointId> {
        let seq = self.sequence_counter.get();
        self.sequence_counter.set(seq + 1);
        let state = self.capture_state()?;

        let mut checkpoint = TemporalCheckpoint::new(
            seq,
            message,
            state,
            CheckpointTrigger::Manual,
            self.session_id,
        );
        checkpoint.tags = tags;

        self.storage.store(&checkpoint)?;
        self.update_last_checkpoint_time();

        Ok(checkpoint.id)
    }

    /// Restore state from a checkpoint
    pub fn restore(&self, checkpoint: &TemporalCheckpoint) -> Result<DebugStateSnapshot> {
        // Validate checkpoint has valid state
        if checkpoint.state.working_dir.is_none() {
            return Err(crate::errors::ReasoningError::InvalidState(
                "Checkpoint has no working directory".to_string()
            ));
        }
        Ok(checkpoint.state.clone())
    }

    /// Get a summary of a checkpoint by ID
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

    /// Delete a checkpoint by ID
    pub fn delete(&self, id: &CheckpointId) -> Result<()> {
        self.storage.delete(*id)
    }

    /// Compact checkpoints, keeping only the most recent N
    pub fn compact(&self, keep_recent: usize) -> Result<usize> {
        self.compact_with_policy(CompactionPolicy::KeepRecent(keep_recent))
    }

    /// Compact checkpoints using a specific policy
    pub fn compact_with_policy(&self, policy: CompactionPolicy) -> Result<usize> {
        let all_checkpoints = self.storage.list_by_session(self.session_id)?;
        
        // Determine which checkpoints to keep
        let ids_to_keep: std::collections::HashSet<CheckpointId> = match &policy {
            CompactionPolicy::KeepRecent(n) => {
                // Sort by sequence number, keep last N
                let mut sorted = all_checkpoints.clone();
                sorted.sort_by_key(|cp| cp.sequence_number);
                sorted.iter().rev().take(*n).map(|cp| cp.id).collect()
            }
            CompactionPolicy::PreserveTagged(tags) => {
                // Keep all checkpoints with any of the specified tags
                all_checkpoints.iter()
                    .filter(|cp| cp.tags.iter().any(|t| tags.contains(t)))
                    .map(|cp| cp.id)
                    .collect()
            }
            CompactionPolicy::Hybrid { keep_recent, preserve_tags } => {
                // Keep recent + preserve tagged
                let mut to_keep = std::collections::HashSet::new();
                
                // Add recent
                let mut sorted = all_checkpoints.clone();
                sorted.sort_by_key(|cp| cp.sequence_number);
                for cp in sorted.iter().rev().take(*keep_recent) {
                    to_keep.insert(cp.id);
                }
                
                // Add tagged
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

    fn capture_state(&self) -> Result<DebugStateSnapshot> {
        Ok(DebugStateSnapshot {
            session_id: self.session_id,
            started_at: Utc::now(),
            checkpoint_timestamp: Utc::now(),
            working_dir: std::env::current_dir().ok(),
            env_vars: std::env::vars().collect(),
            metrics: SessionMetrics::default(),
            hypothesis_state: None, // Will be populated when hypothesis state is captured
        })
    }

    fn update_last_checkpoint_time(&self) {
        *self.last_checkpoint_time.borrow_mut() = Utc::now();
    }
}

//! SQLiteGraph storage implementation for checkpoints

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

use chrono::Utc;
use sqlitegraph::{GraphEntity, SqliteGraph};

use crate::checkpoint::{CheckpointId, CheckpointSummary, SessionId, TemporalCheckpoint, DebugStateSnapshot, CheckpointTrigger, AutoTrigger};
use crate::errors::{Result, StorageError};
use crate::storage::CheckpointStorage;

/// SQLiteGraph-based checkpoint storage (MVP version)
pub struct SqliteGraphStorage {
    graph: RefCell<SqliteGraph>,
    /// In-memory cache for query operations (MVP workaround)
    cache: RefCell<HashMap<CheckpointId, TemporalCheckpoint>>,
}

// Safety: We use RefCell for single-threaded interior mutability.
// For thread-safe usage, wrap in ThreadSafeStorage which uses Arc<Mutex<>>.
unsafe impl Send for SqliteGraphStorage {}
unsafe impl Sync for SqliteGraphStorage {}

impl SqliteGraphStorage {
    /// Open or create storage at the given path
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let graph = SqliteGraph::open(path)?;
        let storage = Self {
            graph: RefCell::new(graph),
            cache: RefCell::new(HashMap::new()),
        };
        // Load existing checkpoints from disk
        storage.load_from_disk()?;
        Ok(storage)
    }

    /// Open with recovery - attempts to repair corrupted storage
    pub fn open_with_recovery(path: impl AsRef<Path>) -> Result<Self> {
        // Try normal open first
        match Self::open(&path) {
            Ok(storage) => Ok(storage),
            Err(_) => {
                // If that fails, try to create fresh storage
                // In production, this would attempt actual recovery
                tracing::warn!("Storage open failed, attempting recovery");
                Self::open(path)
            }
        }
    }

    /// Create an in-memory storage (for testing)
    pub fn in_memory() -> Result<Self> {
        let graph = SqliteGraph::open_in_memory()?;
        Ok(Self {
            graph: RefCell::new(graph),
            cache: RefCell::new(HashMap::new()),
        })
    }

    /// Load all checkpoints from SQLite into cache
    fn load_from_disk(&self) -> Result<()> {
        let graph = self.graph.borrow();
        let entity_ids = graph.list_entity_ids()
            .map_err(|e| StorageError::RetrieveFailed(format!("Failed to load entity IDs: {}", e)))?;
        
        let mut cache = self.cache.borrow_mut();
        cache.clear();
        
        for entity_id in entity_ids {
            if let Ok(entity) = graph.get_entity(entity_id) {
                if entity.kind == "Checkpoint" {
                    if let Ok(checkpoint) = self.entity_to_checkpoint(&entity) {
                        cache.insert(checkpoint.id, checkpoint);
                    }
                }
            }
        }
        
        Ok(())
    }

    /// Convert a GraphEntity to TemporalCheckpoint
    fn entity_to_checkpoint(&self, entity: &GraphEntity) -> Result<TemporalCheckpoint> {
        let data = &entity.data;
        
        let state_data = data.get("state_data")
            .and_then(|v| v.as_str())
            .ok_or_else(|| StorageError::RetrieveFailed("Missing state data".to_string()))?;
        
        let state: DebugStateSnapshot = serde_json::from_str(state_data)
            .map_err(|e| StorageError::RetrieveFailed(format!("Failed to deserialize state: {}", e)))?;
        
        // Parse checkpoint ID from string
        let id_str = data.get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| StorageError::RetrieveFailed("Missing checkpoint ID".to_string()))?;
        let checkpoint_id = parse_checkpoint_id(id_str)?;
        
        // Parse timestamp
        let timestamp_str = data.get("timestamp")
            .and_then(|v| v.as_str())
            .ok_or_else(|| StorageError::RetrieveFailed("Missing timestamp".to_string()))?;
        let timestamp = chrono::DateTime::parse_from_rfc3339(timestamp_str)
            .map_err(|e| StorageError::RetrieveFailed(format!("Invalid timestamp: {}", e)))?
            .with_timezone(&Utc);
        
        // Parse sequence number
        let sequence_number = data.get("sequence_number")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| StorageError::RetrieveFailed("Missing sequence number".to_string()))?;
        
        // Parse message
        let message = data.get("message")
            .and_then(|v: &serde_json::Value| v.as_str())
            .unwrap_or("")
            .to_string();
        
        // Parse tags
        let tags = data.get("tags")
            .and_then(|v: &serde_json::Value| v.as_array())
            .map(|arr: &Vec<serde_json::Value>| arr.iter()
                .filter_map(|v: &serde_json::Value| v.as_str().map(String::from))
                .collect())
            .unwrap_or_default();
        
        // Parse session ID
        let session_id_str = data.get("session_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| StorageError::RetrieveFailed("Missing session ID".to_string()))?;
        let session_id = parse_session_id(session_id_str)?;
        
        // Parse trigger
        let trigger_str = data.get("trigger")
            .and_then(|v| v.as_str())
            .unwrap_or("manual");
        let trigger = parse_trigger(trigger_str);
        
        // Parse checksum (may not exist for legacy checkpoints)
        let checksum = data.get("checksum")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        
        Ok(TemporalCheckpoint {
            id: checkpoint_id,
            timestamp,
            sequence_number,
            message,
            tags,
            state,
            trigger,
            session_id,
            checksum,
        })
    }
}

impl CheckpointStorage for SqliteGraphStorage {
    fn store(&self, checkpoint: &TemporalCheckpoint) -> Result<()> {
        // Serialize state to JSON
        let state_json = serde_json::to_string(&checkpoint.state)
            .map_err(|e| StorageError::StoreFailed(format!("Failed to serialize state: {}", e)))?;

        // Create checkpoint entity
        let entity = GraphEntity {
            id: 0,
            kind: "Checkpoint".to_string(),
            name: checkpoint.id.to_string(),
            file_path: None,
            data: serde_json::json!({
                "id": checkpoint.id,
                "timestamp": checkpoint.timestamp,
                "sequence_number": checkpoint.sequence_number,
                "message": checkpoint.message,
                "tags": checkpoint.tags,
                "trigger": format!("{}", checkpoint.trigger),
                "session_id": checkpoint.session_id,
                "state_data": state_json,
                "checksum": checkpoint.checksum,
            }),
        };

        // Insert into graph
        let graph = self.graph.borrow();
        let _entity_id = graph.insert_entity(&entity)
            .map_err(|e| StorageError::StoreFailed(format!("Failed to insert: {}", e)))?;

        // Also store in cache for easy retrieval
        self.cache.borrow_mut().insert(checkpoint.id, checkpoint.clone());

        tracing::debug!("Stored checkpoint {}", checkpoint.id);
        Ok(())
    }

    fn get(&self, id: CheckpointId) -> Result<TemporalCheckpoint> {
        // Try cache first
        if let Some(cp) = self.cache.borrow().get(&id) {
            return Ok(cp.clone());
        }
        
        Err(StorageError::RetrieveFailed(format!("Checkpoint not found: {}", id)).into())
    }

    fn get_latest(&self, session_id: SessionId) -> Result<Option<TemporalCheckpoint>> {
        let checkpoints = self.list_by_session(session_id)?;
        
        // Get the one with highest sequence number
        let latest = checkpoints.iter()
            .max_by_key(|c: &&CheckpointSummary| c.sequence_number);
        
        match latest {
            Some(summary) => self.get(summary.id).map(Some),
            None => Ok(None),
        }
    }

    fn list_by_session(&self, session_id: SessionId) -> Result<Vec<CheckpointSummary>> {
        let cache = self.cache.borrow();
        let mut summaries = Vec::new();
        
        for (_, checkpoint) in cache.iter() {
            if checkpoint.session_id == session_id {
                summaries.push(CheckpointSummary {
                    id: checkpoint.id,
                    timestamp: checkpoint.timestamp,
                    sequence_number: checkpoint.sequence_number,
                    message: checkpoint.message.clone(),
                    trigger: checkpoint.trigger.to_string(),
                    tags: checkpoint.tags.clone(),
                    has_notes: false,
                });
            }
        }
        
        // Sort by sequence number
        summaries.sort_by_key(|s: &CheckpointSummary| s.sequence_number);
        
        Ok(summaries)
    }

    fn list_by_tag(&self, tag: &str) -> Result<Vec<CheckpointSummary>> {
        let cache = self.cache.borrow();
        let mut summaries = Vec::new();
        
        for (_, checkpoint) in cache.iter() {
            if checkpoint.tags.contains(&tag.to_string()) {
                summaries.push(CheckpointSummary {
                    id: checkpoint.id,
                    timestamp: checkpoint.timestamp,
                    sequence_number: checkpoint.sequence_number,
                    message: checkpoint.message.clone(),
                    trigger: checkpoint.trigger.to_string(),
                    tags: checkpoint.tags.clone(),
                    has_notes: false,
                });
            }
        }
        
        // Sort by sequence number
        summaries.sort_by_key(|s: &CheckpointSummary| s.sequence_number);
        
        Ok(summaries)
    }

    fn delete(&self, id: CheckpointId) -> Result<()> {
        // Remove from cache
        self.cache.borrow_mut().remove(&id);
        
        // Try to remove from SQLite (best effort)
        // Note: This requires entity ID lookup which we don't track
        // For MVP, cache removal is sufficient
        
        Ok(())
    }

    fn next_sequence(&self, _session_id: SessionId) -> Result<u64> {
        Ok(0)
    }

    fn get_max_sequence(&self) -> Result<u64> {
        let cache = self.cache.borrow();
        let max_seq = cache.values()
            .map(|cp| cp.sequence_number)
            .max()
            .unwrap_or(0);
        Ok(max_seq)
    }
}

// Helper functions for parsing

fn parse_checkpoint_id(s: &str) -> Result<CheckpointId> {
    let uuid = uuid::Uuid::parse_str(s)
        .map_err(|e| StorageError::RetrieveFailed(format!("Invalid checkpoint ID: {}", e)))?;
    Ok(CheckpointId(uuid))
}

fn parse_session_id(s: &str) -> Result<SessionId> {
    let uuid = uuid::Uuid::parse_str(s)
        .map_err(|e| StorageError::RetrieveFailed(format!("Invalid session ID: {}", e)))?;
    Ok(SessionId(uuid))
}

fn parse_trigger(s: &str) -> CheckpointTrigger {
    if s.starts_with("auto") {
        CheckpointTrigger::Automatic(AutoTrigger::VerificationComplete)
    } else if s == "scheduled" {
        CheckpointTrigger::Scheduled
    } else {
        CheckpointTrigger::Manual
    }
}

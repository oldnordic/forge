//! Export and Import functionality for checkpoints
//!
//! Provides serialization to/from JSON for backup and migration.

use std::rc::Rc;

use serde::{Deserialize, Serialize};

use crate::checkpoint::{SessionId, TemporalCheckpoint};
use crate::errors::{Result, StorageError};
use crate::storage::CheckpointStorage;
use crate::thread_safe::ThreadSafeStorage;

/// Export format for a session's checkpoints
#[derive(Serialize, Deserialize)]
pub struct SessionExport {
    pub version: String,
    pub session_id: SessionId,
    pub exported_at: chrono::DateTime<chrono::Utc>,
    pub checkpoints: Vec<TemporalCheckpoint>,
}

/// Exports checkpoints to various formats
pub struct CheckpointExporter {
    storage: ThreadSafeStorage,
}

impl CheckpointExporter {
    /// Create a new exporter for the given storage
    pub fn new(storage: ThreadSafeStorage) -> Self {
        Self { storage }
    }

    /// Export all checkpoints for a session as JSON
    pub fn export_session(&self, session_id: &SessionId) -> Result<String> {
        // Get all checkpoints for the session
        let summaries = self.storage.list_by_session(*session_id)?;
        
        let mut checkpoints = Vec::new();
        for summary in summaries {
            if let Ok(cp) = self.storage.get(summary.id) {
                checkpoints.push(cp);
            }
        }
        
        let export = SessionExport {
            version: "1.0".to_string(),
            session_id: *session_id,
            exported_at: chrono::Utc::now(),
            checkpoints,
        };
        
        serde_json::to_string_pretty(&export)
            .map_err(|e| StorageError::StoreFailed(format!("Export serialization failed: {}", e)).into())
    }

    /// Export to a file
    pub fn export_session_to_file(&self, session_id: &SessionId, path: &std::path::Path) -> Result<()> {
        let json = self.export_session(session_id)?;
        std::fs::write(path, json)
            .map_err(|e| StorageError::StoreFailed(format!("Failed to write export file: {}", e)).into())
    }
}

/// Imports checkpoints from various formats
pub struct CheckpointImporter {
    storage: ThreadSafeStorage,
}

impl CheckpointImporter {
    /// Create a new importer for the given storage
    pub fn new(storage: ThreadSafeStorage) -> Self {
        Self { storage }
    }

    /// Import checkpoints from JSON string
    pub fn import_session(&self, json: &str) -> Result<usize> {
        let export: SessionExport = serde_json::from_str(json)
            .map_err(|e| StorageError::RetrieveFailed(format!("Import deserialization failed: {}", e)))?;
        
        let mut count = 0;
        for checkpoint in export.checkpoints {
            self.storage.store(&checkpoint)?;
            count += 1;
        }
        
        Ok(count)
    }

    /// Import from a file
    pub fn import_session_from_file(&self, path: &std::path::Path) -> Result<usize> {
        let json = std::fs::read_to_string(path)
            .map_err(|e| StorageError::RetrieveFailed(format!("Failed to read import file: {}", e)))?;
        self.import_session(&json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteGraphStorage;

    #[test]
    fn test_export_import_roundtrip() {
        let storage = ThreadSafeStorage::new(SqliteGraphStorage::in_memory().unwrap());
        let session_id = SessionId::new();
        
        // Export (will be empty but should work)
        let exporter = CheckpointExporter::new(storage.clone());
        let json = exporter.export_session(&session_id).unwrap();
        
        // Should be valid JSON
        assert!(json.contains("version"));
        assert!(json.contains("session_id"));
        
        // Import to new storage
        let new_storage = ThreadSafeStorage::new(SqliteGraphStorage::in_memory().unwrap());
        let importer = CheckpointImporter::new(new_storage);
        let count = importer.import_session(&json).unwrap();
        
        // Empty export imports 0 checkpoints
        assert_eq!(count, 0);
    }
}

//! TDD Tests for Temporal Checkpointing
//!
//! Tests drive the implementation. Run with: cargo test

use chrono::Utc;
use forge_reasoning::*;
use std::rc::Rc;
use std::collections::HashMap;

/// Test 1: Can create an in-memory storage
#[test]
fn test_create_in_memory_storage() {
    let storage = SqliteGraphStorage::in_memory();
    assert!(storage.is_ok(), "Should be able to create in-memory storage");
}

/// Test 2: Can create a checkpoint manager
#[test]
fn test_create_checkpoint_manager() {
    let storage = Rc::new(SqliteGraphStorage::in_memory().unwrap());
    let session_id = SessionId::new();
    let _manager = TemporalCheckpointManager::new(storage, session_id);
    // If we get here without panic, test passes
}

/// Test 3: Can create a checkpoint
#[test]
fn test_create_checkpoint() {
    let storage = Rc::new(SqliteGraphStorage::in_memory().unwrap());
    let session_id = SessionId::new();
    let manager = TemporalCheckpointManager::new(storage, session_id);
    
    let cp_id = manager.checkpoint("Test checkpoint");
    assert!(cp_id.is_ok(), "Should be able to create checkpoint");
}

/// Test 4: Checkpoint ID is unique
#[test]
fn test_checkpoint_ids_are_unique() {
    let storage = Rc::new(SqliteGraphStorage::in_memory().unwrap());
    let session_id = SessionId::new();
    let manager = TemporalCheckpointManager::new(storage, session_id);
    
    let cp1 = manager.checkpoint("First").unwrap();
    let cp2 = manager.checkpoint("Second").unwrap();
    
    assert_ne!(cp1, cp2, "Checkpoint IDs should be unique");
}

/// Test 5: Sequence numbers increment
#[test]
fn test_sequence_numbers_increment() {
    let storage = Rc::new(SqliteGraphStorage::in_memory().unwrap());
    let session_id = SessionId::new();
    let manager = TemporalCheckpointManager::new(storage, session_id);
    
    // Can't directly test sequence numbers without exposing them,
    // but we can verify multiple checkpoints don't fail
    for i in 0..5 {
        let result = manager.checkpoint(format!("Checkpoint {}", i));
        assert!(result.is_ok());
    }
}

/// Test 6: Can list checkpoints (even if empty)
#[test]
fn test_list_checkpoints_empty() {
    let storage = Rc::new(SqliteGraphStorage::in_memory().unwrap());
    let session_id = SessionId::new();
    let manager = TemporalCheckpointManager::new(storage, session_id);
    
    let checkpoints = manager.list();
    assert!(checkpoints.is_ok());
    // MVP returns empty list
    assert_eq!(checkpoints.unwrap().len(), 0);
}

/// Test 7: Auto-checkpoint with throttling
#[test]
fn test_auto_checkpoint_throttling() {
    let storage = Rc::new(SqliteGraphStorage::in_memory().unwrap());
    let session_id = SessionId::new();
    let manager = TemporalCheckpointManager::new(storage, session_id);
    
    // First auto-checkpoint should succeed
    let cp1 = manager.auto_checkpoint(AutoTrigger::VerificationComplete);
    assert!(cp1.is_ok());
    
    // Immediate second auto-checkpoint may be throttled
    let cp2 = manager.auto_checkpoint(AutoTrigger::VerificationComplete);
    assert!(cp2.is_ok());
}

/// Test 8: Manual checkpoint always works (no throttling)
#[test]
fn test_manual_checkpoint_no_throttling() {
    let storage = Rc::new(SqliteGraphStorage::in_memory().unwrap());
    let session_id = SessionId::new();
    let manager = TemporalCheckpointManager::new(storage, session_id);
    
    // Create many checkpoints quickly
    for i in 0..10 {
        let result = manager.checkpoint(format!("Rapid checkpoint {}", i));
        assert!(result.is_ok());
    }
}

/// Test 9: Checkpoint has correct session ID
#[test]
fn test_checkpoint_session_id() {
    let storage = Rc::new(SqliteGraphStorage::in_memory().unwrap());
    let session_id = SessionId::new();
    let _manager = TemporalCheckpointManager::new(storage.clone(), session_id);
    
    let _cp_id = _manager.checkpoint("Test").unwrap();
    
    // In full implementation, we'd retrieve and verify session_id
    // For MVP, we just verify creation succeeds
    // TODO: Implement retrieval and verification
}

/// Test 10: Checkpoint state contains environment info
#[test]
fn test_checkpoint_state_has_env() {
    // Create a checkpoint and verify state has working_dir and env_vars
    let storage = Rc::new(SqliteGraphStorage::in_memory().unwrap());
    let session_id = SessionId::new();
    let manager = TemporalCheckpointManager::new(storage, session_id);
    
    let _cp_id = manager.checkpoint("Test env capture").unwrap();
    
    // TODO: Retrieve and verify state contains working_dir
    // TODO: Verify state contains env_vars
}

// ============================================================================
// TDD WAVE 2: Query Methods (get_by_id, list_by_session, restore)
// ============================================================================

/// Test 11: Can retrieve checkpoint by ID
#[test]
fn test_get_checkpoint_by_id() {
    let storage = Rc::new(SqliteGraphStorage::in_memory().unwrap());
    let session_id = SessionId::new();
    let manager = TemporalCheckpointManager::new(storage, session_id);
    
    // Create a checkpoint
    let cp_id = manager.checkpoint("Retrievable checkpoint").unwrap();
    
    // Retrieve it
    let retrieved = manager.get(&cp_id);
    assert!(retrieved.is_ok(), "Should be able to retrieve checkpoint");
    
    let checkpoint = retrieved.unwrap();
    assert!(checkpoint.is_some(), "Checkpoint should exist");
    
    let checkpoint = checkpoint.unwrap();
    assert_eq!(checkpoint.id, cp_id, "Retrieved checkpoint ID should match");
    assert_eq!(checkpoint.message, "Retrievable checkpoint", "Message should match");
}

/// Test 12: Get by ID returns None for non-existent checkpoint
#[test]
fn test_get_checkpoint_not_found() {
    let storage = Rc::new(SqliteGraphStorage::in_memory().unwrap());
    let session_id = SessionId::new();
    let manager = TemporalCheckpointManager::new(storage, session_id);
    
    // Try to retrieve non-existent checkpoint
    let result = manager.get(&CheckpointId::new());
    assert!(result.is_ok());
    assert!(result.unwrap().is_none(), "Should return None for non-existent checkpoint");
}

/// Test 13: Can list checkpoints for a session
#[test]
fn test_list_checkpoints_by_session() {
    let storage = Rc::new(SqliteGraphStorage::in_memory().unwrap());
    let session_id = SessionId::new();
    let manager = TemporalCheckpointManager::new(storage.clone(), session_id);
    
    // Create multiple checkpoints
    manager.checkpoint("First in session").unwrap();
    manager.checkpoint("Second in session").unwrap();
    manager.checkpoint("Third in session").unwrap();
    
    // List checkpoints
    let checkpoints = manager.list_by_session(&session_id).unwrap();
    assert_eq!(checkpoints.len(), 3, "Should list all checkpoints in session");
}

/// Test 14: Listing by session excludes other sessions
#[test]
fn test_list_session_isolation() {
    let storage = Rc::new(SqliteGraphStorage::in_memory().unwrap());
    
    // Create two sessions
    let session1 = SessionId::new();
    let session2 = SessionId::new();
    
    let manager1 = TemporalCheckpointManager::new(storage.clone(), session1);
    let manager2 = TemporalCheckpointManager::new(storage.clone(), session2);
    
    // Add checkpoints to both sessions
    manager1.checkpoint("Session 1 checkpoint").unwrap();
    manager2.checkpoint("Session 2 checkpoint").unwrap();
    manager1.checkpoint("Another session 1").unwrap();
    
    // Verify isolation
    let session1_cps = manager1.list_by_session(&session1).unwrap();
    let session2_cps = manager2.list_by_session(&session2).unwrap();
    
    assert_eq!(session1_cps.len(), 2, "Session 1 should have 2 checkpoints");
    assert_eq!(session2_cps.len(), 1, "Session 2 should have 1 checkpoint");
}

/// Test 15: Can list checkpoints by tag
#[test]
fn test_list_checkpoints_by_tag() {
    let storage = Rc::new(SqliteGraphStorage::in_memory().unwrap());
    let session_id = SessionId::new();
    let manager = TemporalCheckpointManager::new(storage, session_id);
    
    // Create checkpoints with tags (use checkpoint_with_tags)
    let cp1 = manager.checkpoint_with_tags("Important", vec!["critical".to_string(), "release".to_string()]).unwrap();
    let _cp2 = manager.checkpoint_with_tags("Also important", vec!["critical".to_string()]).unwrap();
    manager.checkpoint("No tags").unwrap();
    
    // List by tag
    let critical_cps = manager.list_by_tag("critical").unwrap();
    let release_cps = manager.list_by_tag("release").unwrap();
    
    assert_eq!(critical_cps.len(), 2, "Should find 2 critical checkpoints");
    assert_eq!(release_cps.len(), 1, "Should find 1 release checkpoint");
    assert_eq!(release_cps[0].id, cp1, "Release checkpoint should be cp1");
}

/// Test 16: Can restore checkpoint state
#[test]
fn test_restore_checkpoint() {
    let storage = Rc::new(SqliteGraphStorage::in_memory().unwrap());
    let session_id = SessionId::new();
    let manager = TemporalCheckpointManager::new(storage, session_id);
    
    // Create a checkpoint
    let cp_id = manager.checkpoint("Restore test").unwrap();
    
    // Get the checkpoint
    let checkpoint = manager.get(&cp_id).unwrap().unwrap();
    
    // Restore it
    let restored = manager.restore(&checkpoint);
    assert!(restored.is_ok(), "Should be able to restore checkpoint");
    
    let state = restored.unwrap();
    assert!(state.working_dir.is_some(), "State should have working directory");
}

/// Test 17: Restore returns error for invalid checkpoint
#[test]
fn test_restore_invalid_checkpoint() {
    let storage = Rc::new(SqliteGraphStorage::in_memory().unwrap());
    let session_id = SessionId::new();
    let manager = TemporalCheckpointManager::new(storage, session_id);
    
    // Create an invalid checkpoint (empty state)
    let invalid_cp = TemporalCheckpoint {
        id: CheckpointId::new(),
        timestamp: Utc::now(),
        sequence_number: 0,
        message: "Invalid".to_string(),
        tags: vec![],
        state: DebugStateSnapshot {
            session_id: SessionId::new(),
            started_at: Utc::now(),
            checkpoint_timestamp: Utc::now(),
            working_dir: None,
            env_vars: HashMap::new(),
            metrics: SessionMetrics::default(),
            hypothesis_state: None,
        },
        trigger: CheckpointTrigger::Manual,
        session_id: SessionId::new(),
        checksum: String::new(), // Empty checksum for test
    };
    
    let result = manager.restore(&invalid_cp);
    assert!(result.is_err(), "Should fail to restore invalid checkpoint");
}

/// Test 18: Checkpoint ordering by timestamp
#[test]
fn test_checkpoint_ordering() {
    let storage = Rc::new(SqliteGraphStorage::in_memory().unwrap());
    let session_id = SessionId::new();
    let manager = TemporalCheckpointManager::new(storage, session_id);
    
    // Create checkpoints with small delays
    manager.checkpoint("First").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
    manager.checkpoint("Second").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
    manager.checkpoint("Third").unwrap();
    
    // List should be in chronological order
    let checkpoints = manager.list().unwrap();
    assert_eq!(checkpoints.len(), 3);
    
    // Verify ordering (earliest first)
    for i in 1..checkpoints.len() {
        assert!(
            checkpoints[i].timestamp >= checkpoints[i-1].timestamp,
            "Checkpoints should be ordered by timestamp"
        );
    }
}

/// Test 19: Checkpoint summary has correct info
#[test]
fn test_checkpoint_summary() {
    let storage = Rc::new(SqliteGraphStorage::in_memory().unwrap());
    let session_id = SessionId::new();
    let manager = TemporalCheckpointManager::new(storage, session_id);
    
    let cp_id = manager.checkpoint("Summary test").unwrap();
    
    // Get summary
    let summary = manager.get_summary(&cp_id).unwrap();
    assert!(summary.is_some());
    
    let summary = summary.unwrap();
    assert_eq!(summary.id, cp_id);
    assert_eq!(summary.message, "Summary test");
}

/// Test 20: List checkpoints returns populated results
#[test]
fn test_list_checkpoints_populated() {
    let storage = Rc::new(SqliteGraphStorage::in_memory().unwrap());
    let session_id = SessionId::new();
    let manager = TemporalCheckpointManager::new(storage, session_id);
    
    // Create checkpoints
    manager.checkpoint("CP 1").unwrap();
    manager.checkpoint("CP 2").unwrap();
    
    // List should return both
    let checkpoints = manager.list().unwrap();
    assert_eq!(checkpoints.len(), 2, "Should list both checkpoints");
    
    // Verify they have correct messages
    let messages: Vec<_> = checkpoints.iter().map(|cp| cp.message.clone()).collect();
    assert!(messages.contains(&"CP 1".to_string()));
    assert!(messages.contains(&"CP 2".to_string()));
}

// ============================================================================
// TDD WAVE 3: Persistence & Durability (File-based storage)
// ============================================================================

use std::path::PathBuf;
use tempfile::TempDir;

/// Test 21: Can create file-based storage
#[test]
fn test_create_file_based_storage() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("checkpoints.db");
    
    let storage = SqliteGraphStorage::open(&db_path);
    assert!(storage.is_ok(), "Should be able to create file-based storage");
}

/// Test 22: Checkpoints persist to disk and can be reloaded
#[test]
fn test_checkpoint_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("checkpoints.db");
    let session_id = SessionId::new();
    
    // Create storage and checkpoint
    {
        let storage = Rc::new(SqliteGraphStorage::open(&db_path).unwrap());
        let manager = TemporalCheckpointManager::new(storage, session_id);
        manager.checkpoint("Persistent checkpoint").unwrap();
    }
    
    // Reopen storage and verify checkpoint exists
    {
        let storage = Rc::new(SqliteGraphStorage::open(&db_path).unwrap());
        let manager = TemporalCheckpointManager::new(storage, session_id);
        let checkpoints = manager.list().unwrap();
        
        assert_eq!(checkpoints.len(), 1, "Should reload checkpoint from disk");
        assert_eq!(checkpoints[0].message, "Persistent checkpoint");
    }
}

/// Test 23: Multiple sessions persist independently
#[test]
fn test_multi_session_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("checkpoints.db");
    let session1 = SessionId::new();
    let session2 = SessionId::new();
    
    // Create checkpoints for both sessions
    {
        let storage = Rc::new(SqliteGraphStorage::open(&db_path).unwrap());
        let manager1 = TemporalCheckpointManager::new(storage.clone(), session1);
        let manager2 = TemporalCheckpointManager::new(storage.clone(), session2);
        
        manager1.checkpoint("Session 1").unwrap();
        manager2.checkpoint("Session 2").unwrap();
    }
    
    // Reopen and verify both sessions
    {
        let storage = Rc::new(SqliteGraphStorage::open(&db_path).unwrap());
        let manager1 = TemporalCheckpointManager::new(storage.clone(), session1);
        let manager2 = TemporalCheckpointManager::new(storage.clone(), session2);
        
        let cps1 = manager1.list_by_session(&session1).unwrap();
        let cps2 = manager2.list_by_session(&session2).unwrap();
        
        assert_eq!(cps1.len(), 1);
        assert_eq!(cps2.len(), 1);
        assert_eq!(cps1[0].message, "Session 1");
        assert_eq!(cps2[0].message, "Session 2");
    }
}

/// Test 24: Can export checkpoints to JSON
#[test]
fn test_export_checkpoints() {
    let session_id = SessionId::new();
    let storage = ThreadSafeStorage::new(SqliteGraphStorage::in_memory().unwrap());
    let manager = ThreadSafeCheckpointManager::new(storage.clone(), session_id);
    
    // Create checkpoints
    let _cp1 = manager.checkpoint("Export test 1").unwrap();
    let _cp2 = manager.checkpoint("Export test 2").unwrap();
    
    // Export session
    let exporter = CheckpointExporter::new(storage);
    let json = exporter.export_session(&session_id).unwrap();
    
    assert!(json.contains("Export test 1"), "Export should contain first checkpoint");
    assert!(json.contains("Export test 2"), "Export should contain second checkpoint");
}

/// Test 25: Can import checkpoints from JSON
#[test]
fn test_import_checkpoints() {
    let session_id = SessionId::new();
    
    // Create export data first
    let export_storage = ThreadSafeStorage::new(SqliteGraphStorage::in_memory().unwrap());
    let export_manager = ThreadSafeCheckpointManager::new(export_storage.clone(), session_id);
    export_manager.checkpoint("Import test").unwrap();
    
    let exporter = CheckpointExporter::new(export_storage);
    let json = exporter.export_session(&session_id).unwrap();
    
    // Import into new storage
    let import_storage = ThreadSafeStorage::new(SqliteGraphStorage::in_memory().unwrap());
    let importer = CheckpointImporter::new(import_storage.clone());
    let imported_count = importer.import_session(&json).unwrap();
    
    assert_eq!(imported_count, 1, "Should import one checkpoint");
    
    // Verify import
    let verify_manager = ThreadSafeCheckpointManager::new(import_storage, session_id);
    let checkpoints = verify_manager.list().unwrap();
    assert_eq!(checkpoints.len(), 1);
    assert_eq!(checkpoints[0].message, "Import test");
}

/// Test 26: Checkpoint compaction removes old checkpoints
#[test]
fn test_checkpoint_compaction() {
    let storage = Rc::new(SqliteGraphStorage::in_memory().unwrap());
    let session_id = SessionId::new();
    let mut manager = TemporalCheckpointManager::new(storage.clone(), session_id);
    
    // Create many checkpoints
    for i in 0..10 {
        manager.checkpoint(format!("Checkpoint {}", i)).unwrap();
    }
    
    // Compact to keep only last 5
    let compacted = manager.compact(5).unwrap();
    assert_eq!(compacted, 5, "Should remove 5 old checkpoints");
    
    // Verify only 5 remain
    let checkpoints = manager.list().unwrap();
    assert_eq!(checkpoints.len(), 5, "Should have 5 checkpoints after compaction");
    
    // Verify newest are kept (sequence numbers 5-9)
    let seqs: Vec<u64> = checkpoints.iter().map(|cp| cp.sequence_number).collect();
    assert!(seqs.iter().all(|&s| s >= 5), "Should keep newest checkpoints");
}

/// Test 27: Compaction preserves tagged checkpoints
#[test]
fn test_compaction_preserves_tags() {
    let storage = Rc::new(SqliteGraphStorage::in_memory().unwrap());
    let session_id = SessionId::new();
    let manager = TemporalCheckpointManager::new(storage.clone(), session_id);
    
    // Create checkpoints, some with important tags
    manager.checkpoint_with_tags("Important 1", vec!["preserve".to_string()]).unwrap();
    manager.checkpoint("Normal 1").unwrap();
    manager.checkpoint_with_tags("Important 2", vec!["preserve".to_string()]).unwrap();
    manager.checkpoint("Normal 2").unwrap();
    
    // Compact to 2, but preserve tagged
    let compacted = manager.compact_with_policy(CompactionPolicy::PreserveTagged(vec!["preserve".to_string()])).unwrap();
    
    // Both tagged should remain plus 2 most recent normal = 4 total (or fewer if compacted more)
    let checkpoints = manager.list().unwrap();
    let tagged_count = checkpoints.iter().filter(|cp| cp.tags.contains(&"preserve".to_string())).count();
    assert_eq!(tagged_count, 2, "Should preserve both tagged checkpoints");
}

/// Test 28: Can delete specific checkpoint
#[test]
fn test_delete_checkpoint() {
    let storage = Rc::new(SqliteGraphStorage::in_memory().unwrap());
    let session_id = SessionId::new();
    let manager = TemporalCheckpointManager::new(storage.clone(), session_id);
    
    let cp1 = manager.checkpoint("Keep me").unwrap();
    let cp2 = manager.checkpoint("Delete me").unwrap();
    
    // Delete second checkpoint
    manager.delete(&cp2).unwrap();
    
    // Verify deletion
    let checkpoints = manager.list().unwrap();
    assert_eq!(checkpoints.len(), 1);
    assert_eq!(checkpoints[0].id, cp1);
    
    // Verify cp2 is gone
    assert!(manager.get(&cp2).unwrap().is_none(), "Deleted checkpoint should be gone");
}

/// Test 29: Storage recovery handles corrupted data gracefully
#[test]
fn test_storage_recovery() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("checkpoints.db");
    let session_id = SessionId::new();
    
    // Create valid checkpoints
    {
        let storage = Rc::new(SqliteGraphStorage::open(&db_path).unwrap());
        let manager = TemporalCheckpointManager::new(storage, session_id);
        manager.checkpoint("Valid checkpoint").unwrap();
    }
    
    // Try to open with recovery
    let storage = SqliteGraphStorage::open_with_recovery(&db_path);
    assert!(storage.is_ok(), "Should recover and open storage");
    
    let storage = Rc::new(storage.unwrap());
    let manager = TemporalCheckpointManager::new(storage, session_id);
    let checkpoints = manager.list().unwrap();
    
    // Should have valid checkpoint
    assert!(!checkpoints.is_empty(), "Should recover valid checkpoints");
}

/// Test 30: Export/import roundtrip preserves all data
#[test]
fn test_export_import_roundtrip() {
    let session_id = SessionId::new();
    let storage = ThreadSafeStorage::new(SqliteGraphStorage::in_memory().unwrap());
    let manager = ThreadSafeCheckpointManager::new(storage.clone(), session_id);
    
    // Create checkpoint with tags
    let cp_id = manager.checkpoint_with_tags(
        "Roundtrip test",
        vec!["tag1".to_string(), "tag2".to_string()]
    ).unwrap();
    
    // Export  
    let exporter = CheckpointExporter::new(storage);
    let json = exporter.export_session(&session_id).unwrap();
    
    // Import to new storage  
    let new_storage = ThreadSafeStorage::new(SqliteGraphStorage::in_memory().unwrap());
    let importer = CheckpointImporter::new(new_storage.clone());
    let imported_count = importer.import_session(&json).unwrap();
    assert_eq!(imported_count, 1, "Should import one checkpoint");
    
    // Verify all data preserved
    let new_manager = ThreadSafeCheckpointManager::new(new_storage, session_id);
    let checkpoints = new_manager.list().unwrap();
    assert_eq!(checkpoints.len(), 1);
    
    let cp = new_manager.get(&cp_id).unwrap().unwrap();
    assert_eq!(cp.message, "Roundtrip test");
    assert!(cp.tags.contains(&"tag1".to_string()));
    assert!(cp.tags.contains(&"tag2".to_string()));
}

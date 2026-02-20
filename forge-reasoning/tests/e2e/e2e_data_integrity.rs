//! E2E Test: Data Integrity and Validation Workflows
//!
//! Tests for checksum validation, corruption detection, and recovery.

use forge_reasoning::*;
use std::sync::Arc;

/// E2E Test 7: Full validation workflow
#[test]
fn e2e_validation_workflow() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    let session = service.create_session("integrity-test").unwrap();
    
    // Create checkpoints
    for i in 0..5 {
        let _ = service.checkpoint(&session, &format!("CP-{}", i)).unwrap();
    }
    
    // Validate all checkpoints
    let report = service.validate_all_checkpoints().unwrap();
    
    assert_eq!(report.total(), 5, "Should check all 5 checkpoints");
    assert_eq!(report.valid, 5, "All should be valid");
    assert_eq!(report.invalid, 0, "None should be invalid");
    assert!(report.all_valid(), "Report should indicate all valid");
}

/// E2E Test 8: Health check with validation
#[test]
fn e2e_health_check_with_validation() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    let session = service.create_session("health-test").unwrap();
    
    // Create some checkpoints
    for i in 0..10 {
        let _ = service.checkpoint(&session, &format!("CP-{}", i)).unwrap();
    }
    
    // Health check with validation
    let health = service.health_check_with_validation().unwrap();
    
    assert!(health.healthy, "Health check should pass");
    assert!(health.message.contains("validated"), "Should mention validation");
}

/// E2E Test 9: Export/Import with integrity verification
#[test]
fn e2e_export_import_with_integrity() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    let session = service.create_session("export-test").unwrap();
    
    // Create checkpoints with different tags
    let _ = service.execute(CheckpointCommand::Create {
        session_id: session,
        message: "Before fix".to_string(),
        tags: vec!["baseline".to_string()],
    }).unwrap();
    
    let _ = service.execute(CheckpointCommand::Create {
        session_id: session,
        message: "After fix".to_string(),
        tags: vec!["fixed".to_string()],
    }).unwrap();
    
    // Export
    let export_data = service.export_all_checkpoints().unwrap();
    
    // Verify export contains checksums
    assert!(export_data.contains("checksum"), "Export should contain checksums");
    assert!(export_data.contains("Before fix"), "Export should contain checkpoint data");
    assert!(export_data.contains("After fix"), "Export should contain checkpoint data");
    
    // Import to new service
    let new_storage = ThreadSafeStorage::in_memory().unwrap();
    let new_service = Arc::new(CheckpointService::new(new_storage));
    
    let result = new_service.import_checkpoints(&export_data).unwrap();
    assert_eq!(result.imported, 2, "Should import 2 checkpoints");
    
    // Note: Imported checkpoints are stored but may not be visible in list_checkpoints
    // because they belong to sessions that don't exist in the new service.
    // The import works at storage level.
    
    // Verify global sequence was restored
    assert_eq!(new_service.global_sequence(), 2, "Global sequence should be restored");
}

/// E2E Test 10: Concurrent checkpoint creation with integrity
#[test]
fn e2e_concurrent_integrity() {
    use std::thread;
    
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    let session = service.create_session("concurrent-test").unwrap();
    
    // Spawn threads creating checkpoints
    let mut handles = Vec::new();
    for thread_id in 0..4 {
        let svc = Arc::clone(&service);
        let sess = session;
        
        let handle = thread::spawn(move || {
            for i in 0..10 {
                let msg = format!("Thread-{}-CP-{}", thread_id, i);
                let id = svc.checkpoint(&sess, &msg).unwrap();
                
                // Verify each checkpoint immediately
                assert!(svc.validate_checkpoint(&id).unwrap(), 
                       "Checkpoint {} should be valid", id);
            }
        });
        
        handles.push(handle);
    }
    
    // Wait for all threads
    for h in handles {
        h.join().unwrap();
    }
    
    // Final validation
    let report = service.validate_all_checkpoints().unwrap();
    assert_eq!(report.total(), 40, "Should have 40 total checkpoints");
    assert_eq!(report.valid, 40, "All should be valid");
}

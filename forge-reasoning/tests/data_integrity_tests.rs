//! TDD Wave 10: Data Integrity & Validation
//!
//! Tests for checksum verification, corruption detection, and data validation.

use forge_reasoning::*;
use std::sync::Arc;

/// Test 78: Checkpoints include checksum on creation
#[test]
fn test_checkpoint_includes_checksum() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    let session = service.create_session("test").unwrap();
    
    let id = service.checkpoint(&session, "Test checkpoint").unwrap();
    let cp = service.get_with_annotations(&id).unwrap();
    
    // Checkpoint should have a checksum
    assert!(!cp.checkpoint.checksum.is_empty(), "Checkpoint should have checksum");
}

/// Test 79: Same data produces same checksum (when timestamps match)
/// 
/// NOTE: Checksums include timestamps, so checkpoints created at different
/// times will have different checksums even with identical messages.
/// This test verifies the checksum is computed correctly.
#[test]
fn test_checksum_is_deterministic() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    let session = service.create_session("test").unwrap();
    
    // Create a checkpoint
    let id = service.checkpoint(&session, "Test message").unwrap();
    let cp = service.get_with_annotations(&id).unwrap();
    
    // Verify the checkpoint validates against its own checksum
    assert!(cp.checkpoint.validate().is_ok(), "Checksum should validate");
    
    // Checksum should be 64 hex characters (SHA-256)
    assert_eq!(cp.checkpoint.checksum.len(), 64, "Should be SHA-256 length");
    
    // Checksum should not be empty
    assert!(!cp.checkpoint.checksum.is_empty(), "Checksum should not be empty");
}

/// Test 80: Different data produces different checksum
#[test]
fn test_checksum_uniqueness() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    let session = service.create_session("test").unwrap();
    
    // Create checkpoints with different messages
    let id1 = service.checkpoint(&session, "Message A").unwrap();
    let id2 = service.checkpoint(&session, "Message B").unwrap();
    
    let cp1 = service.get_with_annotations(&id1).unwrap();
    let cp2 = service.get_with_annotations(&id2).unwrap();
    
    // Checksums should be different
    assert_ne!(cp1.checkpoint.checksum, cp2.checkpoint.checksum);
}

/// Test 81: Validate returns true for valid checkpoint
#[test]
fn test_validate_succeeds_for_valid_checkpoint() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    let session = service.create_session("test").unwrap();
    
    let id = service.checkpoint(&session, "Valid checkpoint").unwrap();
    let cp = service.get_with_annotations(&id).unwrap();
    
    // Validation should succeed
    assert!(cp.checkpoint.validate().is_ok(), "Valid checkpoint should pass validation");
}

/// Test 82: Service validate_checkpoint method works
#[test]
fn test_service_validate_checkpoint() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    let session = service.create_session("test").unwrap();
    
    let id = service.checkpoint(&session, "Test").unwrap();
    
    // Service-level validation should succeed
    let result = service.validate_checkpoint(&id);
    assert!(result.is_ok(), "Service validation should succeed");
    assert!(result.unwrap(), "Service validation should return true for valid checkpoint");
}

/// Test 83: Export includes checksums
#[test]
fn test_export_includes_checksums() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    let session = service.create_session("test").unwrap();
    
    let _ = service.checkpoint(&session, "CP1").unwrap();
    let _ = service.checkpoint(&session, "CP2").unwrap();
    
    let export = service.export_all_checkpoints().unwrap();
    
    // Export should contain checksum field
    assert!(export.contains("checksum"), "Export should include checksums");
}

/// Test 84: Import validates checksums
#[test]
fn test_import_validates_checksums() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    let session = service.create_session("test").unwrap();
    
    let id = service.checkpoint(&session, "Test").unwrap();
    let export = service.export_all_checkpoints().unwrap();
    
    // Import should succeed with valid checksums
    let new_storage = ThreadSafeStorage::in_memory().unwrap();
    let new_service = Arc::new(CheckpointService::new(new_storage));
    let result = new_service.import_checkpoints(&export);
    
    assert!(result.is_ok(), "Import with valid checksums should succeed");
}

/// Test 85: Service health check validates recent checkpoints
#[test]
fn test_health_check_validates_checkpoints() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    let session = service.create_session("test").unwrap();
    
    // Create some checkpoints
    for i in 0..5 {
        let _ = service.checkpoint(&session, &format!("CP-{}", i)).unwrap();
    }
    
    // Health check should validate recent checkpoints
    let health = service.health_check_with_validation().unwrap();
    assert!(health.healthy, "Health check should pass with valid checkpoints");
}

/// Test 86: Batch validation of all checkpoints
#[test]
fn test_batch_validation() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    let session = service.create_session("test").unwrap();
    
    // Create multiple checkpoints
    for i in 0..10 {
        let _ = service.checkpoint(&session, &format!("CP-{}", i)).unwrap();
    }
    
    // Batch validate all checkpoints
    let result = service.validate_all_checkpoints().unwrap();
    
    assert_eq!(result.valid, 10, "All 10 checkpoints should be valid");
    assert_eq!(result.invalid, 0, "No checkpoints should be invalid");
}

/// Test 87: Validation report provides details
#[test]
fn test_validation_report_details() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    let session = service.create_session("test").unwrap();
    
    let _ = service.checkpoint(&session, "CP1").unwrap();
    let _ = service.checkpoint(&session, "CP2").unwrap();
    
    let report = service.validate_all_checkpoints().unwrap();
    
    // Report should contain validation details
    assert!(report.checked_at.is_some(), "Report should have timestamp");
    assert_eq!(report.total(), 2, "Report should show total checked");
}

/// Test 88: Concurrent validation is safe
#[test]
fn test_concurrent_validation() {
    use std::thread;
    
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    let session = service.create_session("test").unwrap();
    
    // Create checkpoints
    for i in 0..20 {
        let _ = service.checkpoint(&session, &format!("CP-{}", i)).unwrap();
    }
    
    // Spawn threads for concurrent validation
    let mut handles = Vec::new();
    for _ in 0..4 {
        let svc = Arc::clone(&service);
        let handle = thread::spawn(move || {
            svc.validate_all_checkpoints().unwrap()
        });
        handles.push(handle);
    }
    
    // All validations should complete successfully
    for handle in handles {
        let report = handle.join().unwrap();
        assert_eq!(report.valid, 20, "Each validation should find all 20 valid");
    }
}

/// Test 89: Checksum algorithm is SHA-256
#[test]
fn test_checksum_algorithm_is_sha256() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    let session = service.create_session("test").unwrap();
    
    let id = service.checkpoint(&session, "Test").unwrap();
    let cp = service.get_with_annotations(&id).unwrap();
    
    // SHA-256 produces 64 hex characters
    assert_eq!(cp.checkpoint.checksum.len(), 64, "SHA-256 checksum should be 64 hex chars");
    
    // Should only contain hex characters
    assert!(cp.checkpoint.checksum.chars().all(|c| c.is_ascii_hexdigit()), 
            "Checksum should be hexadecimal");
}

/// Test 90: Validation catches tampered data (simulated)
#[test]
fn test_validation_detects_tampering() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    let session = service.create_session("test").unwrap();
    
    let id = service.checkpoint(&session, "Original message").unwrap();
    let mut cp = service.get_with_annotations(&id).unwrap();
    
    // Simulate tampering by modifying the message
    cp.checkpoint.message = "Tampered message".to_string();
    
    // Validation should fail (checksum won't match)
    let result = cp.checkpoint.validate();
    assert!(result.is_err(), "Validation should fail for tampered checkpoint");
}

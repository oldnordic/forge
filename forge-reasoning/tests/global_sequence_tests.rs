//! TDD Wave 9: Global Sequence Numbers
//!
//! Tests for globally monotonic checkpoint sequences across all sessions.

use forge_reasoning::*;
use std::sync::Arc;

/// Test 68: Global sequence starts at 1
#[test]
fn test_global_sequence_starts_at_one() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    let session = service.create_session("test").unwrap();
    
    let id = service.checkpoint(&session, "First checkpoint").unwrap();
    let checkpoints = service.list_checkpoints(&session).unwrap();
    
    assert_eq!(checkpoints.len(), 1);
    assert_eq!(checkpoints[0].sequence_number, 1);
}

/// Test 69: Sequences increment globally across sessions
#[test]
fn test_global_sequence_increments_across_sessions() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    
    let session_a = service.create_session("session-a").unwrap();
    let session_b = service.create_session("session-b").unwrap();
    
    // Create checkpoints alternating between sessions
    let _ = service.checkpoint(&session_a, "A1").unwrap();
    let _ = service.checkpoint(&session_b, "B1").unwrap();
    let _ = service.checkpoint(&session_a, "A2").unwrap();
    let _ = service.checkpoint(&session_b, "B2").unwrap();
    
    // Verify sequences are globally ordered
    let cps_a = service.list_checkpoints(&session_a).unwrap();
    let cps_b = service.list_checkpoints(&session_b).unwrap();
    
    // Session A: sequences 1 and 3
    assert_eq!(cps_a[0].sequence_number, 1);
    assert_eq!(cps_a[1].sequence_number, 3);
    
    // Session B: sequences 2 and 4
    assert_eq!(cps_b[0].sequence_number, 2);
    assert_eq!(cps_b[1].sequence_number, 4);
}

/// Test 70: Global sequence persists across service restarts
#[test]
fn test_global_sequence_persists_across_restarts() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("checkpoints.db");
    
    // First service instance
    {
        let storage = ThreadSafeStorage::open(&db_path).unwrap();
        let service = Arc::new(CheckpointService::new(storage));
        let session = service.create_session("test").unwrap();
        
        let _ = service.checkpoint(&session, "CP1").unwrap();
        let _ = service.checkpoint(&session, "CP2").unwrap();
        let _ = service.checkpoint(&session, "CP3").unwrap();
        // Last sequence should be 3
    }
    
    // Second service instance (simulated restart)
    {
        let storage = ThreadSafeStorage::open(&db_path).unwrap();
        let service = Arc::new(CheckpointService::new(storage));
        let session = service.create_session("test2").unwrap();
        
        // Next checkpoint should continue from 4
        let _ = service.checkpoint(&session, "CP4").unwrap();
        
        let checkpoints = service.list_checkpoints(&session).unwrap();
        assert_eq!(checkpoints[0].sequence_number, 4);
    }
}

/// Test 71: Concurrent checkpoints get unique global sequences
#[test]
fn test_concurrent_global_sequences_are_unique() {
    use std::thread;
    
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    
    // Create multiple sessions
    let mut sessions = Vec::new();
    for i in 0..5 {
        sessions.push(service.create_session(&format!("session-{}", i)).unwrap());
    }
    
    // Spawn threads creating checkpoints concurrently
    let mut handles = Vec::new();
    for (idx, session) in sessions.iter().enumerate() {
        let service_clone = Arc::clone(&service);
        let session_clone = *session;
        
        let handle = thread::spawn(move || {
            let mut ids = Vec::new();
            for i in 0..10 {
                let id = service_clone
                    .checkpoint(&session_clone, &format!("Thread-{}-CP-{}", idx, i))
                    .unwrap();
                ids.push(id);
            }
            ids
        });
        handles.push(handle);
    }
    
    // Collect all checkpoint IDs
    let mut all_ids = Vec::new();
    for handle in handles {
        all_ids.extend(handle.join().unwrap());
    }
    
    // Collect all sequence numbers
    let mut sequences = Vec::new();
    for session in &sessions {
        let cps = service.list_checkpoints(session).unwrap();
        for cp in cps {
            sequences.push(cp.sequence_number);
        }
    }
    
    // Should have 50 unique sequences (5 sessions × 10 checkpoints)
    assert_eq!(sequences.len(), 50);
    
    // All sequences should be unique
    sequences.sort_unstable();
    sequences.dedup();
    assert_eq!(sequences.len(), 50);
    
    // Sequences should be 1 through 50
    for (i, seq) in sequences.iter().enumerate() {
        assert_eq!(*seq, (i + 1) as u64, "Expected sequence {}, got {}", i + 1, seq);
    }
}

/// Test 72: Global sequence is monotonic under load
#[test]
fn test_global_sequence_monotonic_under_load() {
    use std::thread;
    
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    let session = service.create_session("load-test").unwrap();
    
    // Create many checkpoints rapidly
    let service_clone = Arc::clone(&service);
    let session_clone = session;
    
    let handle = thread::spawn(move || {
        let mut sequences = Vec::new();
        for i in 0..100 {
            let id = service_clone
                .checkpoint(&session_clone, &format!("Load-CP-{}", i))
                .unwrap();
            
            // Get the checkpoint to verify sequence
            let cps = service_clone.list_checkpoints(&session_clone).unwrap();
            let cp = cps.iter().find(|c| c.id == id).unwrap();
            sequences.push(cp.sequence_number);
        }
        sequences
    });
    
    let sequences = handle.join().unwrap();
    
    // Verify strictly increasing
    for i in 1..sequences.len() {
        assert!(
            sequences[i] > sequences[i - 1],
            "Sequence not monotonic: {} followed by {}",
            sequences[i - 1],
            sequences[i]
        );
    }
}

/// Test 73: Service exposes current global sequence
#[test]
fn test_service_exposes_global_sequence() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    
    // Initially should be 0 (no checkpoints yet)
    let initial_seq = service.global_sequence();
    assert_eq!(initial_seq, 0);
    
    let session = service.create_session("test").unwrap();
    
    // After one checkpoint
    let _ = service.checkpoint(&session, "CP1").unwrap();
    assert_eq!(service.global_sequence(), 1);
    
    // After second checkpoint
    let _ = service.checkpoint(&session, "CP2").unwrap();
    assert_eq!(service.global_sequence(), 2);
}

/// Test 74: Global sequence with multiple services (edge case)
/// 
/// NOTE: Each CheckpointService maintains its own sequence counter.
/// For truly global sequences across multiple service instances, 
/// use a single shared service instance.
#[test]
fn test_global_sequence_with_service_per_session() {
    // Each service has its own sequence counter
    let storage = ThreadSafeStorage::in_memory().unwrap();
    
    let service1 = Arc::new(CheckpointService::new(storage.clone()));
    let service2 = Arc::new(CheckpointService::new(storage));
    
    let session1 = service1.create_session("s1").unwrap();
    let session2 = service2.create_session("s2").unwrap();
    
    // Create checkpoints via different services
    // Each service maintains its own counter starting at 1
    let _ = service1.checkpoint(&session1, "Via Service 1").unwrap();
    let _ = service2.checkpoint(&session2, "Via Service 2").unwrap();
    
    // Each service sees its own sequence counter
    assert_eq!(service1.global_sequence(), 1);
    assert_eq!(service2.global_sequence(), 1);
    
    // Second checkpoint in service1 continues its sequence
    let _ = service1.checkpoint(&session1, "Via Service 1 again").unwrap();
    assert_eq!(service1.global_sequence(), 2);
}

/// Test 75: Sequence survives compaction
#[test]
fn test_global_sequence_survives_compaction() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    let session = service.create_session("test").unwrap();
    
    // Create 10 checkpoints
    for i in 0..10 {
        let _ = service.checkpoint(&session, &format!("CP-{}", i)).unwrap();
    }
    
    assert_eq!(service.global_sequence(), 10);
    
    // Compact to keep only 3
    let result = service.execute(CheckpointCommand::Compact {
        session_id: session,
        keep_recent: 3,
    }).unwrap();
    
    match result {
        CommandResult::Compacted(deleted) => assert_eq!(deleted, 7),
        _ => panic!("Expected Compacted result"),
    }
    
    // Global sequence should continue from 11 (not reset)
    let _ = service.checkpoint(&session, "After compaction").unwrap();
    
    let cps = service.list_checkpoints(&session).unwrap();
    let last_cp = cps.iter().max_by_key(|c| c.sequence_number).unwrap();
    assert_eq!(last_cp.sequence_number, 11);
}

/// Test 76: Query checkpoints by sequence range
#[test]
fn test_query_by_sequence_range() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    let session_a = service.create_session("session-a").unwrap();
    let session_b = service.create_session("session-b").unwrap();
    
    // Create interleaved checkpoints
    let _ = service.checkpoint(&session_a, "A1").unwrap(); // seq 1
    let _ = service.checkpoint(&session_b, "B1").unwrap(); // seq 2
    let _ = service.checkpoint(&session_a, "A2").unwrap(); // seq 3
    let _ = service.checkpoint(&session_b, "B2").unwrap(); // seq 4
    let _ = service.checkpoint(&session_a, "A3").unwrap(); // seq 5
    
    // Query by sequence range
    let range_cps = service.list_by_sequence_range(2, 4).unwrap();
    
    assert_eq!(range_cps.len(), 3);
    assert_eq!(range_cps[0].sequence_number, 2);
    assert_eq!(range_cps[1].sequence_number, 3);
    assert_eq!(range_cps[2].sequence_number, 4);
}

/// Test 77: Export/import preserves global sequences
/// 
/// NOTE: After import, the global sequence counter is restored to the
/// maximum sequence from imported checkpoints, allowing new checkpoints
/// to continue the sequence.
#[test]
fn test_export_import_preserves_global_sequences() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    
    let session_a = service.create_session("session-a").unwrap();
    let session_b = service.create_session("session-b").unwrap();
    
    // Create checkpoints with known sequences
    let _ = service.checkpoint(&session_a, "A1").unwrap(); // seq 1
    let _ = service.checkpoint(&session_b, "B1").unwrap(); // seq 2
    let _ = service.checkpoint(&session_a, "A2").unwrap(); // seq 3
    
    // Export
    let export_data = service.export_all_checkpoints().unwrap();
    
    // Verify export contains the expected data
    assert!(export_data.contains("A1"));
    assert!(export_data.contains("B1"));
    assert!(export_data.contains("sequence_number\": 3"));
    
    // Create new service and import
    let new_storage = ThreadSafeStorage::in_memory().unwrap();
    let new_service = Arc::new(CheckpointService::new(new_storage));
    let result = new_service.import_checkpoints(&export_data).unwrap();
    
    // Should have imported 3 checkpoints
    assert_eq!(result.imported, 3);
    
    // Verify global sequence was restored
    assert_eq!(new_service.global_sequence(), 3);
    
    // New checkpoints should continue from 4
    let session_c = new_service.create_session("session-c").unwrap();
    let _ = new_service.checkpoint(&session_c, "C1").unwrap();
    
    assert_eq!(new_service.global_sequence(), 4);
}

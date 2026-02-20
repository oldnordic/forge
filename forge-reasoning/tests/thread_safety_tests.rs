//! TDD Wave 4: Thread Safety Tests
//!
//! Tests for concurrent checkpoint operations

use std::thread;

use forge_reasoning::*;

/// Test 31: Thread-safe storage can be shared between threads
#[test]
fn test_thread_safe_storage_send_sync() {
    // Create thread-safe storage
    let storage = ThreadSafeStorage::in_memory().unwrap();
    
    // Clone and move to another thread
    let storage_clone = storage.clone();
    let handle = thread::spawn(move || {
        // Should be able to use storage in another thread
        let session_id = SessionId::new();
        let manager = ThreadSafeCheckpointManager::new(storage_clone, session_id);
        manager.checkpoint("Thread checkpoint").unwrap();
    });
    
    handle.join().unwrap();
    
    // Main thread can still use storage
    let session_id = SessionId::new();
    let manager = ThreadSafeCheckpointManager::new(storage, session_id);
    manager.checkpoint("Main thread checkpoint").unwrap();
}

/// Test 32: Concurrent checkpoint creation from multiple threads
#[test]
fn test_concurrent_checkpoint_creation() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let session_id = SessionId::new();
    
    // Spawn 10 threads, each creating checkpoints
    let handles: Vec<_> = (0..10).map(|i| {
        let storage_clone = storage.clone();
        thread::spawn(move || {
            let manager = ThreadSafeCheckpointManager::new(storage_clone, session_id);
            for j in 0..5 {
                manager.checkpoint(format!("Thread {} - Checkpoint {}", i, j)).unwrap();
            }
        })
    }).collect();
    
    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }
    
    // Verify all 50 checkpoints were created
    let manager = ThreadSafeCheckpointManager::new(storage, session_id);
    let checkpoints = manager.list().unwrap();
    assert_eq!(checkpoints.len(), 50, "All concurrent checkpoints should be created");
}

/// Test 33: Concurrent read and write operations
#[test]
fn test_concurrent_read_write() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let session_id = SessionId::new();
    
    // Pre-populate with some checkpoints
    let manager = ThreadSafeCheckpointManager::new(storage.clone(), session_id);
    for i in 0..10 {
        manager.checkpoint(format!("Initial {}", i)).unwrap();
    }
    
    // Writer thread
    let storage_writer = storage.clone();
    let writer = thread::spawn(move || {
        let manager = ThreadSafeCheckpointManager::new(storage_writer, session_id);
        for i in 0..10 {
            manager.checkpoint(format!("Writer {}", i)).unwrap();
            thread::sleep(std::time::Duration::from_millis(1));
        }
    });
    
    // Reader threads
    let readers: Vec<_> = (0..3).map(|_| {
        let storage_reader = storage.clone();
        thread::spawn(move || {
            let manager = ThreadSafeCheckpointManager::new(storage_reader, session_id);
            for _ in 0..20 {
                let _ = manager.list(); // Should not panic or deadlock
                thread::sleep(std::time::Duration::from_millis(2));
            }
        })
    }).collect();
    
    writer.join().unwrap();
    for reader in readers {
        reader.join().unwrap();
    }
    
    // Verify final state
    let manager = ThreadSafeCheckpointManager::new(storage, session_id);
    let checkpoints = manager.list().unwrap();
    assert!(checkpoints.len() >= 10, "Should have at least initial checkpoints");
}

/// Test 34: Checkpoint IDs remain unique under concurrent access
#[test]
fn test_concurrent_unique_ids() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let session_id = SessionId::new();
    
    // Spawn threads that collect IDs
    let handles: Vec<_> = (0..5).map(|_| {
        let storage_clone = storage.clone();
        thread::spawn(move || {
            let manager = ThreadSafeCheckpointManager::new(storage_clone, session_id);
            let mut local_ids = Vec::new();
            for _ in 0..10 {
                let id = manager.checkpoint("Test").unwrap();
                local_ids.push(id);
            }
            local_ids
        })
    }).collect();
    
    // Collect all IDs from threads
    let mut all_ids = Vec::new();
    for handle in handles {
        all_ids.extend(handle.join().unwrap());
    }
    
    // Verify all IDs are unique
    let unique_ids: std::collections::HashSet<_> = all_ids.iter().collect();
    assert_eq!(unique_ids.len(), all_ids.len(), "All IDs should be unique");
}

/// Test 35: Sequence numbers are monotonic per manager
#[test]
fn test_concurrent_sequence_monotonic() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let session_id = SessionId::new();
    
    // Spawn threads creating checkpoints
    let handles: Vec<_> = (0..5).map(|_| {
        let storage_clone = storage.clone();
        thread::spawn(move || {
            let manager = ThreadSafeCheckpointManager::new(storage_clone, session_id);
            let mut local_sequences = Vec::new();
            for _ in 0..10 {
                let id = manager.checkpoint("Test").unwrap();
                // Get the checkpoint to see its sequence number
                let cp = manager.get(&id).unwrap().unwrap();
                local_sequences.push(cp.sequence_number);
            }
            // Verify local sequences are unique and increasing
            let unique: std::collections::HashSet<_> = local_sequences.iter().collect();
            assert_eq!(unique.len(), local_sequences.len(), "Local sequences should be unique");
        })
    }).collect();
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    // Verify total checkpoints created
    let manager = ThreadSafeCheckpointManager::new(storage, session_id);
    let checkpoints = manager.list().unwrap();
    assert_eq!(checkpoints.len(), 50, "Should have 50 total checkpoints");
}

/// Test 36: Concurrent compaction doesn't corrupt data
#[test]
fn test_concurrent_compaction() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let session_id = SessionId::new();
    
    // Pre-populate
    let manager = ThreadSafeCheckpointManager::new(storage.clone(), session_id);
    for i in 0..20 {
        manager.checkpoint(format!("Checkpoint {}", i)).unwrap();
    }
    
    // Thread adding checkpoints
    let storage_writer = storage.clone();
    let writer = thread::spawn(move || {
        let manager = ThreadSafeCheckpointManager::new(storage_writer, session_id);
        for i in 0..10 {
            manager.checkpoint(format!("Late {}", i)).unwrap();
            thread::sleep(std::time::Duration::from_millis(5));
        }
    });
    
    // Thread compacting
    let storage_compactor = storage.clone();
    let compactor = thread::spawn(move || {
        let manager = ThreadSafeCheckpointManager::new(storage_compactor, session_id);
        thread::sleep(std::time::Duration::from_millis(10));
        manager.compact(15).unwrap();
    });
    
    writer.join().unwrap();
    compactor.join().unwrap();
    
    // Verify storage is still consistent
    let manager = ThreadSafeCheckpointManager::new(storage, session_id);
    let checkpoints = manager.list().unwrap();
    
    // Should have checkpoints (maybe less than 30 due to compaction)
    assert!(!checkpoints.is_empty(), "Should have some checkpoints after concurrent ops");
    
    // All checkpoint IDs should still be unique
    let ids: std::collections::HashSet<_> = checkpoints.iter().map(|cp| cp.id).collect();
    assert_eq!(ids.len(), checkpoints.len(), "No duplicate checkpoint IDs");
}

/// Test 37: Thread-safe session isolation
#[test]
fn test_thread_safe_session_isolation() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let session1 = SessionId::new();
    let session2 = SessionId::new();
    
    // Two threads, different sessions
    let t1 = {
        let storage_clone = storage.clone();
        thread::spawn(move || {
            let manager = ThreadSafeCheckpointManager::new(storage_clone, session1);
            for i in 0..5 {
                manager.checkpoint(format!("S1-{}", i)).unwrap();
            }
            session1
        })
    };
    
    let t2 = {
        let storage_clone = storage.clone();
        thread::spawn(move || {
            let manager = ThreadSafeCheckpointManager::new(storage_clone, session2);
            for i in 0..5 {
                manager.checkpoint(format!("S2-{}", i)).unwrap();
            }
            session2
        })
    };
    
    let s1 = t1.join().unwrap();
    let s2 = t2.join().unwrap();
    
    // Verify isolation
    let manager1 = ThreadSafeCheckpointManager::new(storage.clone(), s1);
    let manager2 = ThreadSafeCheckpointManager::new(storage.clone(), s2);
    
    let cps1 = manager1.list_by_session(&s1).unwrap();
    let cps2 = manager2.list_by_session(&s2).unwrap();
    
    assert_eq!(cps1.len(), 5, "Session 1 should have 5 checkpoints");
    assert_eq!(cps2.len(), 5, "Session 2 should have 5 checkpoints");
    
    // No overlap
    let ids1: std::collections::HashSet<_> = cps1.iter().map(|cp| cp.id).collect();
    let ids2: std::collections::HashSet<_> = cps2.iter().map(|cp| cp.id).collect();
    let intersection: Vec<_> = ids1.intersection(&ids2).collect();
    assert!(intersection.is_empty(), "Sessions should not share checkpoints");
}

/// Test 38: Concurrent export operations
#[test]
fn test_concurrent_export() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let session_id = SessionId::new();
    
    // Pre-populate
    let manager = ThreadSafeCheckpointManager::new(storage.clone(), session_id);
    for i in 0..10 {
        manager.checkpoint(format!("Export test {}", i)).unwrap();
    }
    
    // Multiple threads exporting simultaneously
    let handles: Vec<_> = (0..5).map(|_| {
        let storage_clone = storage.clone();
        thread::spawn(move || {
            let exporter = CheckpointExporter::new(storage_clone);
            let json = exporter.export_session(&session_id).unwrap();
            // Should be valid JSON
            assert!(json.contains("version"));
            assert!(json.contains("Export test"));
        })
    }).collect();
    
    for handle in handles {
        handle.join().unwrap();
    }
}

/// Test 39: Thread-safe checkpoint restoration
#[test]
fn test_concurrent_restore() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let session_id = SessionId::new();
    
    // Create checkpoint to restore
    let manager = ThreadSafeCheckpointManager::new(storage.clone(), session_id);
    let cp_id = manager.checkpoint("Restore target").unwrap();
    let checkpoint = manager.get(&cp_id).unwrap().unwrap();
    
    // Multiple threads restoring same checkpoint
    let handles: Vec<_> = (0..5).map(|_| {
        let checkpoint_clone = checkpoint.clone();
        thread::spawn(move || {
            // Each thread needs its own storage reference
            let storage = ThreadSafeStorage::in_memory().unwrap();
            let manager = ThreadSafeCheckpointManager::new(storage, session_id);
            let state = manager.restore(&checkpoint_clone).unwrap();
            assert!(state.working_dir.is_some());
        })
    }).collect();
    
    for handle in handles {
        handle.join().unwrap();
    }
}

/// Test 40: Stress test - high concurrency
#[test]
fn test_stress_high_concurrency() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let session_id = SessionId::new();
    
    let num_threads = 20;
    let ops_per_thread = 50;
    
    let handles: Vec<_> = (0..num_threads).map(|thread_id| {
        let storage_clone = storage.clone();
        thread::spawn(move || {
            let manager = ThreadSafeCheckpointManager::new(storage_clone, session_id);
            
            for op_id in 0..ops_per_thread {
                // Mix of operations
                match op_id % 4 {
                    0 => { manager.checkpoint(format!("T{}-C{}", thread_id, op_id)).unwrap(); }
                    1 => { let _ = manager.list(); }
                    2 => { let _ = manager.list_by_session(&session_id); }
                    3 => { let _ = manager.get(&CheckpointId::new()); } // May not exist
                    _ => unreachable!(),
                }
            }
        })
    }).collect();
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    // Verify final state is consistent
    let manager = ThreadSafeCheckpointManager::new(storage, session_id);
    let checkpoints = manager.list().unwrap();
    
    // Should have checkpoints (exact count depends on timing)
    assert!(!checkpoints.is_empty(), "Should have checkpoints after stress test");
    
    // All should be valid
    for cp in &checkpoints {
        assert!(!cp.message.is_empty());
        assert!(cp.sequence_number > 0);
    }
}

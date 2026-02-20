//! E2E Test: Complete Checkpoint Workflow
//!
//! Simulates a user performing a debugging session with checkpoints.

use forge_reasoning::*;
use std::sync::Arc;

/// E2E Test 1: Complete debugging workflow with checkpoints
#[test]
fn e2e_complete_debugging_session() {
    // Setup: User starts a debugging session
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    let session = service.create_session("debug-rocm-issue-123").unwrap();
    
    // Step 1: Initial investigation checkpoint
    let cp1 = service.checkpoint(&session, "Initial state - tensor dims look wrong").unwrap();
    
    // Step 2: Add tags to categorize
    let _ = service.execute(CheckpointCommand::Create {
        session_id: session,
        message: "After checking GGUF header - offset=128".to_string(),
        tags: vec!["investigation".to_string(), "gguf".to_string()],
    }).unwrap();
    
    // Step 3: List checkpoints to review progress
    let checkpoints = service.list_checkpoints(&session).unwrap();
    assert_eq!(checkpoints.len(), 2, "Should have 2 checkpoints");
    
    // Step 4: User adds annotation to first checkpoint
    let annotation = CheckpointAnnotation {
        note: "This is where I first noticed the issue".to_string(),
        severity: AnnotationSeverity::Info,
        timestamp: chrono::Utc::now(),
    };
    service.annotate(&cp1, annotation).unwrap();
    
    // Step 5: Verify annotated checkpoint
    let annotated = service.get_with_annotations(&cp1).unwrap();
    assert_eq!(annotated.annotations.len(), 1);
    
    // Step 6: Export session for sharing
    let export = service.export_all_checkpoints().unwrap();
    assert!(!export.is_empty());
    
    // Step 7: Health check
    let health = service.health_check().unwrap();
    assert!(health.healthy);
}

/// E2E Test 2: Multi-session debugging with cross-session queries
#[test]
fn e2e_multi_session_debugging() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    
    // User works on issue #1
    let session1 = service.create_session("issue-1").unwrap();
    let _ = service.checkpoint(&session1, "Issue 1 - initial").unwrap();
    let _ = service.checkpoint(&session1, "Issue 1 - progress").unwrap();
    
    // User switches to issue #2
    let session2 = service.create_session("issue-2").unwrap();
    let _ = service.checkpoint(&session2, "Issue 2 - initial").unwrap();
    
    // Back to issue #1
    let _ = service.checkpoint(&session1, "Issue 1 - resolution").unwrap();
    
    // Query session isolation
    let cps1 = service.list_checkpoints(&session1).unwrap();
    let cps2 = service.list_checkpoints(&session2).unwrap();
    
    assert_eq!(cps1.len(), 3, "Session 1 should have 3 checkpoints");
    assert_eq!(cps2.len(), 1, "Session 2 should have 1 checkpoint");
    
    // Global sequence should be monotonic across sessions
    let all_seqs: Vec<u64> = cps1.iter().chain(cps2.iter())
        .map(|cp| cp.sequence_number)
        .collect();
    
    // All sequences should be unique (global ordering)
    let unique_seqs: std::collections::HashSet<_> = all_seqs.iter().cloned().collect();
    assert_eq!(unique_seqs.len(), all_seqs.len(), "Sequences should be unique");
}

/// E2E Test 3: Checkpoint compaction workflow
#[test]
fn e2e_compaction_workflow() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    let session = service.create_session("compaction-test").unwrap();
    
    // Create many checkpoints during investigation
    for i in 0..20 {
        let tags = if i % 5 == 0 {
            vec!["milestone".to_string()]
        } else {
            vec![]
        };
        
        let _ = service.execute(CheckpointCommand::Create {
            session_id: session,
            message: format!("Step {}", i),
            tags,
        }).unwrap();
    }
    
    // Verify 20 checkpoints exist
    let before = service.list_checkpoints(&session).unwrap();
    assert_eq!(before.len(), 20);
    
    // Compact keeping only recent 5 + tagged
    let result = service.execute(CheckpointCommand::Compact {
        session_id: session,
        keep_recent: 5,
    }).unwrap();
    
    match result {
        CommandResult::Compacted(deleted) => {
            assert!(deleted > 0, "Should have deleted some checkpoints");
        }
        _ => panic!("Expected Compacted result"),
    }
    
    // Verify compaction worked
    let after = service.list_checkpoints(&session).unwrap();
    assert!(after.len() <= 10, "Should have fewer checkpoints after compaction");
}

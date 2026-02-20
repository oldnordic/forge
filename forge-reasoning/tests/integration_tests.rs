//! TDD Wave 5: Integration Tests
//!
//! Tests for Forge agent loop integration and API interface

use std::sync::Arc;
use std::time::Duration;

use forge_reasoning::*;

/// Test 41: CheckpointService can be created and used
#[test]
fn test_checkpoint_service_creation() {
    let service = CheckpointService::new(ThreadSafeStorage::in_memory().unwrap());
    assert!(service.is_running());
}

/// Test 42: Service can create checkpoints for multiple sessions
#[test]
fn test_service_multi_session() {
    let service = Arc::new(CheckpointService::new(ThreadSafeStorage::in_memory().unwrap()));
    
    let session1 = service.create_session("session-1").unwrap();
    let session2 = service.create_session("session-2").unwrap();
    
    // Create checkpoints via service
    service.checkpoint(&session1, "Session 1 checkpoint").unwrap();
    service.checkpoint(&session2, "Session 2 checkpoint").unwrap();
    
    // Verify isolation
    let cps1 = service.list_checkpoints(&session1).unwrap();
    let cps2 = service.list_checkpoints(&session2).unwrap();
    
    assert_eq!(cps1.len(), 1);
    assert_eq!(cps2.len(), 1);
}

/// Test 43: Service supports auto-checkpointing
#[test]
fn test_service_auto_checkpoint() {
    let service = Arc::new(CheckpointService::new(ThreadSafeStorage::in_memory().unwrap()));
    let session = service.create_session("auto-test").unwrap();
    
    // Enable auto-checkpointing
    service.enable_auto_checkpoint(&session, AutoCheckpointConfig {
        interval_seconds: 1,
        on_error: true,
        on_tool_call: false,
    }).unwrap();
    
    // Trigger an auto-checkpoint event
    service.trigger_auto_checkpoint(&session, AutoTrigger::VerificationComplete).unwrap();
    
    let cps = service.list_checkpoints(&session).unwrap();
    assert!(cps.len() >= 1, "Should have at least one checkpoint");
}

/// Test 44: Service can restore checkpoints
#[test]
fn test_service_restore() {
    let service = Arc::new(CheckpointService::new(ThreadSafeStorage::in_memory().unwrap()));
    let session = service.create_session("restore-test").unwrap();
    
    // Create and restore
    let cp_id = service.checkpoint(&session, "Restore me").unwrap();
    let state = service.restore(&session, &cp_id).unwrap();
    
    assert!(state.working_dir.is_some());
}

/// Test 45: Service supports checkpoint streaming
#[tokio::test]
async fn test_checkpoint_streaming() {
    let service = Arc::new(CheckpointService::new(ThreadSafeStorage::in_memory().unwrap()));
    let session = service.create_session("stream-test").unwrap();
    
    let mut receiver = service.subscribe(&session).unwrap();
    
    // Create checkpoint should trigger event
    service.checkpoint(&session, "Stream test").unwrap();
    
    // Should receive event (use tokio's timeout)
    let event = tokio::time::timeout(
        Duration::from_secs(1),
        receiver.recv()
    ).await;
    
    assert!(event.is_ok(), "Should receive checkpoint event within timeout");
    let event = event.unwrap();
    assert!(event.is_some(), "Should receive event, not None");
    
    match event.unwrap() {
        CheckpointEvent::Created { checkpoint_id, .. } => {
            assert!(!checkpoint_id.to_string().is_empty());
        }
        _ => panic!("Expected Created event"),
    }
}

/// Test 46: Service handles API commands
#[test]
fn test_service_api_commands() {
    let service = Arc::new(CheckpointService::new(ThreadSafeStorage::in_memory().unwrap()));
    let session = service.create_session("api-test").unwrap();
    
    // Create via API command
    let cmd = CheckpointCommand::Create {
        session_id: session,
        message: "API checkpoint".to_string(),
        tags: vec!["api".to_string()],
    };
    
    let result = service.execute(cmd).unwrap();
    match result {
        CommandResult::Created(cp_id) => {
            assert!(!cp_id.to_string().is_empty());
        }
        _ => panic!("Expected Created result"),
    }
    
    // List via API command
    let list_cmd = CheckpointCommand::List { session_id: session };
    let list_result = service.execute(list_cmd).unwrap();
    
    match list_result {
        CommandResult::List(checkpoints) => {
            assert_eq!(checkpoints.len(), 1);
        }
        _ => panic!("Expected List result"),
    }
}

/// Test 47: Background persistence works
#[test]
fn test_background_persistence() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let db_path = temp_dir.path().join("persistent.db");
    
    let storage = ThreadSafeStorage::open(&db_path).unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    let session = service.create_session("bg-test").unwrap();
    
    // Create checkpoint
    service.checkpoint(&session, "Background test").unwrap();
    
    // Trigger background sync
    service.sync_to_disk().unwrap();
    
    // Verify by reopening
    let storage2 = ThreadSafeStorage::open(&db_path).unwrap();
    let service2 = CheckpointService::new(storage2);
    let cps = service2.list_checkpoints(&session).unwrap();
    
    assert_eq!(cps.len(), 1);
    assert_eq!(cps[0].message, "Background test");
}

/// Test 48: Service supports checkpoint annotations
#[test]
fn test_checkpoint_annotations() {
    let service = Arc::new(CheckpointService::new(ThreadSafeStorage::in_memory().unwrap()));
    let session = service.create_session("annotation-test").unwrap();
    
    let cp_id = service.checkpoint(&session, "Annotated").unwrap();
    
    // Add annotation
    service.annotate(&cp_id, CheckpointAnnotation {
        note: "Important milestone".to_string(),
        severity: AnnotationSeverity::Info,
        timestamp: chrono::Utc::now(),
    }).unwrap();
    
    // Retrieve with annotations
    let cp = service.get_with_annotations(&cp_id).unwrap();
    assert_eq!(cp.annotations.len(), 1);
    assert_eq!(cp.annotations[0].note, "Important milestone");
}

/// Test 49: Service handles concurrent sessions
#[test]
fn test_service_concurrent_sessions() {
    let service = Arc::new(CheckpointService::new(ThreadSafeStorage::in_memory().unwrap()));
    
    let mut handles = vec![];
    
    // Spawn threads with different sessions
    for i in 0..5 {
        let service_clone = Arc::clone(&service);
        let handle = std::thread::spawn(move || {
            let session = service_clone.create_session(&format!("concurrent-{}", i)).unwrap();
            for j in 0..5 {
                service_clone.checkpoint(&session, format!("CP-{}", j)).unwrap();
            }
            service_clone.list_checkpoints(&session).unwrap().len()
        });
        handles.push(handle);
    }
    
    let mut total = 0;
    for handle in handles {
        total += handle.join().unwrap();
    }
    
    assert_eq!(total, 25, "Should have 25 total checkpoints");
}

/// Test 50: Service metrics and health check
#[test]
fn test_service_metrics() {
    let service = Arc::new(CheckpointService::new(ThreadSafeStorage::in_memory().unwrap()));
    
    let session = service.create_session("metrics-test").unwrap();
    service.checkpoint(&session, "Metrics 1").unwrap();
    service.checkpoint(&session, "Metrics 2").unwrap();
    
    let metrics = service.metrics().unwrap();
    assert_eq!(metrics.total_checkpoints, 2);
    assert_eq!(metrics.active_sessions, 1);
    
    // Health check
    let health = service.health_check().unwrap();
    assert!(health.healthy);
}

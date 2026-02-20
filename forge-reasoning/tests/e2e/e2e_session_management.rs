//! E2E Test: Session Management Workflows
//!
//! Tests for creating, managing, and switching between debugging sessions.

use forge_reasoning::*;
use std::sync::Arc;

/// E2E Test 4: Session lifecycle - create, use, and retire
#[test]
fn e2e_session_lifecycle() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    
    // Create multiple sessions
    let s1 = service.create_session("feature-x").unwrap();
    let s2 = service.create_session("bugfix-y").unwrap();
    let s3 = service.create_session("refactor-z").unwrap();
    
    // Add checkpoints to each
    let _ = service.checkpoint(&s1, "Feature X start").unwrap();
    let _ = service.checkpoint(&s2, "Bugfix Y start").unwrap();
    let _ = service.checkpoint(&s3, "Refactor Z start").unwrap();
    
    // Get metrics
    let metrics = service.metrics().unwrap();
    assert_eq!(metrics.active_sessions, 3);
    assert_eq!(metrics.total_checkpoints, 3);
    
    // Sessions are isolated
    let cps1 = service.list_checkpoints(&s1).unwrap();
    let cps2 = service.list_checkpoints(&s2).unwrap();
    
    assert_eq!(cps1.len(), 1);
    assert_eq!(cps2.len(), 1);
    assert_ne!(cps1[0].id, cps2[0].id);
}

/// E2E Test 5: Auto-checkpoint configuration and triggering
#[test]
fn e2e_auto_checkpoint_workflow() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    let session = service.create_session("auto-test").unwrap();
    
    // Configure auto-checkpointing
    let config = AutoCheckpointConfig {
        interval_seconds: 300,
        on_error: true,
        on_tool_call: true,
    };
    service.enable_auto_checkpoint(&session, config).unwrap();
    
    // Simulate auto-trigger
    let result = service.trigger_auto_checkpoint(&session, AutoTrigger::VerificationComplete).unwrap();
    assert!(result.is_some(), "Auto-checkpoint should be created");
    
    // Verify checkpoint exists
    let cps = service.list_checkpoints(&session).unwrap();
    assert_eq!(cps.len(), 1);
    assert!(cps[0].trigger.contains("auto"), "Should be auto-triggered");
}

/// E2E Test 6: Session with annotations and notes
#[test]
fn e2e_session_with_annotations() {
    let storage = ThreadSafeStorage::in_memory().unwrap();
    let service = Arc::new(CheckpointService::new(storage));
    let session = service.create_session("annotated-session").unwrap();
    
    // Create checkpoint
    let cp_id = service.checkpoint(&session, "Checkpoint with notes").unwrap();
    
    // Add multiple annotations
    for i in 0..3 {
        let severity = match i {
            0 => AnnotationSeverity::Info,
            1 => AnnotationSeverity::Warning,
            _ => AnnotationSeverity::Critical,
        };
        
        let annotation = CheckpointAnnotation {
            note: format!("Note {} - severity {:?}", i, severity),
            severity,
            timestamp: chrono::Utc::now(),
        };
        
        service.annotate(&cp_id, annotation).unwrap();
    }
    
    // Retrieve annotated checkpoint
    let annotated = service.get_with_annotations(&cp_id).unwrap();
    assert_eq!(annotated.annotations.len(), 3);
    
    // Verify annotations are ordered by severity
    assert_eq!(annotated.annotations[0].severity, AnnotationSeverity::Info);
    assert_eq!(annotated.annotations[1].severity, AnnotationSeverity::Warning);
}

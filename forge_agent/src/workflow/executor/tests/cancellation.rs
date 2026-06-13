use super::*;

#[test]
fn test_executor_without_cancellation_source() {
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));

    let executor = WorkflowExecutor::new(workflow);

    assert!(executor.cancellation_token().is_none());

    executor.cancel();
}

#[test]
fn test_executor_cancellation_token_access() {
    use crate::workflow::cancellation::CancellationTokenSource;

    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));

    let source = CancellationTokenSource::new();
    let executor = WorkflowExecutor::new(workflow).with_cancellation_source(source);

    assert!(executor.cancellation_token().is_some());
    let token = executor.cancellation_token().unwrap();
    assert!(!token.is_cancelled());
}

#[tokio::test]
async fn test_executor_cancel_stops_execution() {
    use crate::workflow::cancellation::CancellationTokenSource;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B")));
    workflow.add_task(Box::new(MockTask::new("c", "Task C")));

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let cancel_flag_clone = cancel_flag.clone();

    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        cancel_flag_clone.store(true, Ordering::SeqCst);
    });

    let source = CancellationTokenSource::new();
    let mut executor = WorkflowExecutor::new(workflow).with_cancellation_source(source);

    executor.cancel();

    let result = executor.execute().await.unwrap();

    assert!(!result.success);
    assert_eq!(result.completed_tasks.len(), 0);
    assert!(result.error.unwrap().contains("cancelled"));
}

#[tokio::test]
async fn test_cancellation_recorded_in_audit() {
    use crate::workflow::cancellation::CancellationTokenSource;

    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));

    let source = CancellationTokenSource::new();
    let mut executor = WorkflowExecutor::new(workflow).with_cancellation_source(source);

    executor.cancel();

    executor.execute().await.unwrap();

    let events = executor.audit_log().replay();

    assert!(events
        .iter()
        .any(|e| matches!(e, crate::audit::AuditEvent::WorkflowCancelled { .. })));
}

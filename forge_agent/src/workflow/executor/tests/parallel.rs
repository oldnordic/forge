use super::*;

#[tokio::test]
async fn test_execute_parallel_single_task() {
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));

    let mut executor = WorkflowExecutor::new(workflow);
    let result = executor.execute_parallel().await;

    assert!(result.is_ok());
    let workflow_result = result.unwrap();
    assert!(workflow_result.success);
    assert_eq!(workflow_result.completed_tasks.len(), 1);
    assert!(workflow_result.completed_tasks.contains(&TaskId::new("a")));
}

#[tokio::test]
async fn test_execute_parallel_two_independent_tasks() {
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B")));

    let mut executor = WorkflowExecutor::new(workflow);
    let result = executor.execute_parallel().await;

    assert!(result.is_ok());
    let workflow_result = result.unwrap();
    assert!(workflow_result.success);
    assert_eq!(workflow_result.completed_tasks.len(), 2);
    assert!(workflow_result.completed_tasks.contains(&TaskId::new("a")));
    assert!(workflow_result.completed_tasks.contains(&TaskId::new("b")));
}

#[tokio::test]
async fn test_execute_parallel_diamond_pattern() {
    let mut workflow = Workflow::new();

    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B")));
    workflow.add_task(Box::new(MockTask::new("c", "Task C")));
    workflow.add_task(Box::new(MockTask::new("d", "Task D")));

    workflow.add_dependency("a", "b").unwrap();
    workflow.add_dependency("a", "c").unwrap();
    workflow.add_dependency("b", "d").unwrap();
    workflow.add_dependency("c", "d").unwrap();

    let mut executor = WorkflowExecutor::new(workflow);
    let result = executor.execute_parallel().await;

    assert!(result.is_ok());
    let workflow_result = result.unwrap();
    assert!(workflow_result.success);
    assert_eq!(workflow_result.completed_tasks.len(), 4);

    let audit_events = executor.audit_log.replay();

    let parallel_started_events: Vec<_> = audit_events
        .iter()
        .filter(|e| {
            matches!(
                e,
                crate::audit::AuditEvent::WorkflowTaskParallelStarted { .. }
            )
        })
        .collect();

    assert_eq!(parallel_started_events.len(), 3);
}

#[tokio::test]
async fn test_execute_parallel_with_cancellation() {
    use crate::workflow::cancellation::CancellationTokenSource;

    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B")));

    let source = CancellationTokenSource::new();
    let mut executor = WorkflowExecutor::new(workflow).with_cancellation_source(source);

    executor.cancel();

    let result = executor.execute_parallel().await;

    assert!(result.is_ok());
    let workflow_result = result.unwrap();
    assert!(!workflow_result.success);
    assert_eq!(workflow_result.completed_tasks.len(), 0);
    assert_eq!(
        workflow_result.error,
        Some("Workflow cancelled".to_string())
    );
}

#[tokio::test]
async fn test_execute_parallel_empty_workflow() {
    let workflow = Workflow::new();
    let mut executor = WorkflowExecutor::new(workflow);

    let result = executor.execute_parallel().await;

    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(crate::workflow::WorkflowError::EmptyWorkflow)
    ));
}

#[tokio::test]
async fn test_execute_parallel_audit_events() {
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B")));

    let mut executor = WorkflowExecutor::new(workflow);
    let result = executor.execute_parallel().await;

    assert!(result.is_ok());

    let audit_events = executor.audit_log.replay();

    assert!(audit_events
        .iter()
        .any(|e| matches!(e, crate::audit::AuditEvent::WorkflowStarted { .. })));

    let parallel_started: Vec<_> = audit_events
        .iter()
        .filter(|e| {
            matches!(
                e,
                crate::audit::AuditEvent::WorkflowTaskParallelStarted { .. }
            )
        })
        .collect();

    assert!(!parallel_started.is_empty());

    let parallel_completed: Vec<_> = audit_events
        .iter()
        .filter(|e| {
            matches!(
                e,
                crate::audit::AuditEvent::WorkflowTaskParallelCompleted { .. }
            )
        })
        .collect();

    assert!(!parallel_completed.is_empty());

    assert!(audit_events
        .iter()
        .any(|e| matches!(e, crate::audit::AuditEvent::WorkflowCompleted { .. })));

    assert!(audit_events
        .iter()
        .any(|e| matches!(e, crate::audit::AuditEvent::WorkflowDeadlockCheck { .. })));
}

#[tokio::test]
async fn test_deadlock_check_before_execution() {
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B")));
    workflow.add_task(Box::new(MockTask::new("c", "Task C")));

    workflow.add_dependency("a", "b").unwrap();
    workflow.add_dependency("b", "c").unwrap();

    let a_idx = workflow.task_map.get(&TaskId::new("a")).copied().unwrap();
    let c_idx = workflow.task_map.get(&TaskId::new("c")).copied().unwrap();
    workflow.graph.add_edge(c_idx, a_idx, ());

    let mut executor = WorkflowExecutor::new(workflow);
    let result = executor.execute_parallel().await;

    assert!(result.is_err());
    match result {
        Err(crate::workflow::WorkflowError::CycleDetected(cycle)) => {
            assert!(!cycle.is_empty());
        }
        _ => panic!("Expected CycleDetected error, got: {:?}", result),
    }
}

#[tokio::test]
async fn test_parallel_state_updates() {
    let mut workflow = Workflow::new();

    for i in 0..10 {
        workflow.add_task(Box::new(MockTask::new(
            format!("task-{}", i),
            &format!("Task {}", i),
        )));
    }

    let mut executor = WorkflowExecutor::new(workflow);
    let result = executor.execute_parallel().await;

    assert!(result.is_ok());
    let workflow_result = result.unwrap();
    assert!(workflow_result.success);
    assert_eq!(workflow_result.completed_tasks.len(), 10);

    for i in 0..10 {
        assert!(workflow_result
            .completed_tasks
            .contains(&TaskId::new(format!("task-{}", i))));
    }
}

#[tokio::test]
async fn test_deadlock_timeout_abort() {
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));

    let mut executor = WorkflowExecutor::new(workflow)
        .with_deadlock_timeout(std::time::Duration::from_millis(100));

    let result = executor.execute_parallel().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_deadlock_timeout_disabled() {
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));

    let executor = WorkflowExecutor::new(workflow).without_deadlock_timeout();

    assert!(executor.deadlock_timeout.is_none());
}

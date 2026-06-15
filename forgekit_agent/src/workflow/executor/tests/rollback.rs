use super::*;

#[tokio::test]
async fn test_failure_triggers_rollback() {
    let mut workflow = Workflow::new();

    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(
        MockTask::new("b", "Task B").with_dep("a").with_failure(),
    ));
    workflow.add_task(Box::new(MockTask::new("c", "Task C").with_dep("b")));

    workflow.add_dependency("a", "b").unwrap();
    workflow.add_dependency("b", "c").unwrap();

    let mut executor = WorkflowExecutor::new(workflow);
    let result = executor.execute().await.unwrap();

    assert!(!result.success);
    assert_eq!(result.failed_tasks.len(), 1);
    assert_eq!(result.failed_tasks[0], TaskId::new("b"));

    assert!(result.rollback_report.is_some());
    let rollback_report = result.rollback_report.unwrap();

    assert_eq!(rollback_report.rolled_back_tasks.len(), 1);
    assert!(rollback_report
        .rolled_back_tasks
        .contains(&TaskId::new("a")));
    assert_eq!(rollback_report.skipped_tasks.len(), 1);
    assert!(rollback_report.skipped_tasks.contains(&TaskId::new("b")));

    let events = executor.audit_log().replay();
    assert!(events
        .iter()
        .any(|e| matches!(e, crate::audit::AuditEvent::WorkflowTaskRolledBack { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, crate::audit::AuditEvent::WorkflowRolledBack { .. })));
}

#[tokio::test]
async fn test_rollback_strategy_configurable() {
    let mut workflow = Workflow::new();

    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(
        MockTask::new("b", "Task B").with_dep("a").with_failure(),
    ));

    workflow.add_dependency("a", "b").unwrap();

    let mut executor =
        WorkflowExecutor::new(workflow).with_rollback_strategy(RollbackStrategy::FailedOnly);
    assert_eq!(executor.rollback_strategy(), RollbackStrategy::FailedOnly);

    let result = executor.execute().await.unwrap();

    assert!(result.rollback_report.is_some());
    assert_eq!(
        result
            .rollback_report
            .as_ref()
            .unwrap()
            .rolled_back_tasks
            .len(),
        0
    );
    assert_eq!(
        result.rollback_report.as_ref().unwrap().skipped_tasks.len(),
        1
    );
}

#[tokio::test]
async fn test_partial_rollback_diamond_pattern() {
    let mut workflow = Workflow::new();

    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a")));
    workflow.add_task(Box::new(MockTask::new("c", "Task C").with_dep("a")));
    workflow.add_task(Box::new(
        MockTask::new("d", "Task D")
            .with_dep("b")
            .with_dep("c")
            .with_failure(),
    ));

    workflow.add_dependency("a", "b").unwrap();
    workflow.add_dependency("a", "c").unwrap();
    workflow.add_dependency("b", "d").unwrap();
    workflow.add_dependency("c", "d").unwrap();

    let mut executor = WorkflowExecutor::new(workflow);
    let result = executor.execute().await.unwrap();

    assert!(!result.success);
    assert_eq!(result.failed_tasks[0], TaskId::new("d"));

    assert!(result.rollback_report.is_some());
    let rollback_report = result.rollback_report.unwrap();

    assert_eq!(rollback_report.rolled_back_tasks.len(), 3);
    assert!(rollback_report
        .rolled_back_tasks
        .contains(&TaskId::new("a")));
    assert!(rollback_report
        .rolled_back_tasks
        .contains(&TaskId::new("b")));
    assert!(rollback_report
        .rolled_back_tasks
        .contains(&TaskId::new("c")));
    assert_eq!(rollback_report.skipped_tasks.len(), 1);
    assert!(rollback_report.skipped_tasks.contains(&TaskId::new("d")));

    assert!(result.completed_tasks.contains(&TaskId::new("a")));
    assert!(result.completed_tasks.contains(&TaskId::new("b")));
    assert!(result.completed_tasks.contains(&TaskId::new("c")));
}

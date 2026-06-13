use super::*;

#[test]
fn test_executor_register_compensation() {
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B")));

    let mut executor = WorkflowExecutor::new(workflow);

    executor.register_compensation(
        TaskId::new("a"),
        ToolCompensation::skip("Test compensation"),
    );

    assert!(executor
        .compensation_registry
        .has_compensation(&TaskId::new("a")));
    assert!(!executor
        .compensation_registry
        .has_compensation(&TaskId::new("b")));

    let comp = executor.compensation_registry.get(&TaskId::new("a"));
    assert!(comp.is_some());
    assert_eq!(comp.unwrap().description, "Test compensation");
}

#[test]
fn test_executor_register_file_compensation() {
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));

    let mut executor = WorkflowExecutor::new(workflow);

    executor.register_file_compensation(TaskId::new("a"), "/tmp/test.txt");

    assert!(executor
        .compensation_registry
        .has_compensation(&TaskId::new("a")));

    let comp = executor.compensation_registry.get(&TaskId::new("a"));
    assert!(comp.is_some());
    assert!(comp.unwrap().description.contains("Delete file"));
}

#[test]
fn test_executor_validate_compensation_coverage() {
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B")));
    workflow.add_task(Box::new(MockTask::new("c", "Task C")));

    let mut executor = WorkflowExecutor::new(workflow);

    executor.register_compensation(
        TaskId::new("a"),
        ToolCompensation::skip("Test compensation"),
    );

    let report = executor.validate_compensation_coverage();

    assert_eq!(report.tasks_with_compensation.len(), 1);
    assert!(report.tasks_with_compensation.contains(&TaskId::new("a")));

    assert_eq!(report.tasks_without_compensation.len(), 2);
    assert!(report
        .tasks_without_compensation
        .contains(&TaskId::new("b")));
    assert!(report
        .tasks_without_compensation
        .contains(&TaskId::new("c")));

    assert!((report.coverage_percentage - 0.333).abs() < 0.01);
}

#[tokio::test]
async fn test_compensation_registry_integration_with_rollback() {
    let mut workflow = Workflow::new();

    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a")));
    workflow.add_task(Box::new(MockTask::new("c", "Task C").with_dep("b")));

    workflow.add_dependency("a", "b").unwrap();
    workflow.add_dependency("b", "c").unwrap();

    let mut executor = WorkflowExecutor::new(workflow);

    let result = executor.execute().await.unwrap();

    assert!(result.success);

    assert!(executor
        .compensation_registry
        .has_compensation(&TaskId::new("a")));
    assert!(executor
        .compensation_registry
        .has_compensation(&TaskId::new("b")));
    assert!(executor
        .compensation_registry
        .has_compensation(&TaskId::new("c")));
}

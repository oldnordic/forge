use super::*;

#[tokio::test]
async fn test_executor_with_checkpoint_service() {
    use crate::workflow::checkpoint::WorkflowCheckpointService;

    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a")));
    workflow.add_task(Box::new(MockTask::new("c", "Task C").with_dep("a")));

    workflow.add_dependency("a", "b").unwrap();
    workflow.add_dependency("a", "c").unwrap();

    let checkpoint_service = WorkflowCheckpointService::new_default();
    let mut executor =
        WorkflowExecutor::new(workflow).with_checkpoint_service(checkpoint_service.clone());

    let result = executor.execute().await.unwrap();

    assert!(result.success);
    assert_eq!(result.completed_tasks.len(), 3);
    assert_eq!(executor.checkpoint_sequence, 3);
}

#[tokio::test]
async fn test_checkpoint_after_each_task() {
    use crate::workflow::checkpoint::WorkflowCheckpointService;

    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a")));

    workflow.add_dependency("a", "b").unwrap();

    let checkpoint_service = WorkflowCheckpointService::new_default();
    let mut executor =
        WorkflowExecutor::new(workflow).with_checkpoint_service(checkpoint_service.clone());

    executor.execute().await.unwrap();

    assert_eq!(executor.checkpoint_sequence, 2);

    let workflow_id = executor.audit_log.tx_id().to_string();
    let latest = checkpoint_service.get_latest(&workflow_id).unwrap();
    assert!(latest.is_some());

    let checkpoint = latest.unwrap();
    assert_eq!(checkpoint.sequence, 1);
    assert_eq!(checkpoint.completed_tasks.len(), 2);
}

#[tokio::test]
async fn test_checkpoint_service_optional() {
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a")));

    workflow.add_dependency("a", "b").unwrap();

    let mut executor = WorkflowExecutor::new(workflow);

    let result = executor.execute().await.unwrap();

    assert!(result.success);
    assert_eq!(executor.checkpoint_sequence, 0);
}

#[tokio::test]
async fn test_checkpoint_created_after_task_success() {
    use crate::workflow::checkpoint::WorkflowCheckpointService;

    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a")));

    workflow.add_dependency("a", "b").unwrap();

    let checkpoint_service = WorkflowCheckpointService::new_default();
    let mut executor =
        WorkflowExecutor::new(workflow).with_checkpoint_service(checkpoint_service.clone());

    let result = executor.execute().await.unwrap();

    assert!(result.success);
    assert_eq!(result.completed_tasks.len(), 2);
    assert_eq!(executor.checkpoint_sequence, 2);

    let workflow_id = executor.audit_log.tx_id().to_string();
    let latest = checkpoint_service.get_latest(&workflow_id).unwrap();
    assert!(latest.is_some());

    let checkpoint = latest.unwrap();
    assert_eq!(checkpoint.sequence, 1);
    assert_eq!(checkpoint.completed_tasks.len(), 2);
    assert!(checkpoint.completed_tasks.contains(&TaskId::new("a")));
    assert!(checkpoint.completed_tasks.contains(&TaskId::new("b")));
}

#[tokio::test]
async fn test_restore_state_from_checkpoint() {
    use crate::workflow::checkpoint::WorkflowCheckpointService;

    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a")));
    workflow.add_task(Box::new(MockTask::new("c", "Task C").with_dep("a")));

    workflow.add_dependency("a", "b").unwrap();
    workflow.add_dependency("a", "c").unwrap();

    let checkpoint_service = WorkflowCheckpointService::new_default();
    let mut executor =
        WorkflowExecutor::new(workflow).with_checkpoint_service(checkpoint_service.clone());

    executor.execute().await.unwrap();

    let workflow_id = executor.audit_log.tx_id().to_string();
    let checkpoint = checkpoint_service
        .get_latest(&workflow_id)
        .unwrap()
        .unwrap();

    let mut new_workflow = Workflow::new();
    new_workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    new_workflow.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a")));
    new_workflow.add_task(Box::new(MockTask::new("c", "Task C").with_dep("a")));

    new_workflow.add_dependency("a", "b").unwrap();
    new_workflow.add_dependency("a", "c").unwrap();

    let mut new_executor = WorkflowExecutor::new(new_workflow);

    let result = new_executor.restore_checkpoint_state(&checkpoint);
    assert!(result.is_ok());

    assert_eq!(
        new_executor.completed_tasks.len(),
        checkpoint.completed_tasks.len()
    );
    assert!(new_executor.completed_tasks.contains(&TaskId::new("a")));
    assert!(new_executor.completed_tasks.contains(&TaskId::new("b")));
    assert!(new_executor.completed_tasks.contains(&TaskId::new("c")));
    assert_eq!(new_executor.checkpoint_sequence, checkpoint.sequence + 1);
}

#[tokio::test]
async fn test_state_restoration_idempotent() {
    use crate::workflow::checkpoint::WorkflowCheckpointService;

    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a")));

    workflow.add_dependency("a", "b").unwrap();

    let checkpoint_service = WorkflowCheckpointService::new_default();
    let mut executor =
        WorkflowExecutor::new(workflow).with_checkpoint_service(checkpoint_service.clone());

    executor.execute().await.unwrap();

    let workflow_id = executor.audit_log.tx_id().to_string();
    let checkpoint = checkpoint_service
        .get_latest(&workflow_id)
        .unwrap()
        .unwrap();

    let mut new_workflow = Workflow::new();
    new_workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    new_workflow.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a")));

    new_workflow.add_dependency("a", "b").unwrap();

    let mut new_executor = WorkflowExecutor::new(new_workflow);

    let result1 = new_executor.restore_checkpoint_state(&checkpoint);
    assert!(result1.is_ok());
    let completed_count_after_first = new_executor.completed_tasks.len();

    let result2 = new_executor.restore_checkpoint_state(&checkpoint);
    assert!(result2.is_ok());
    let completed_count_after_second = new_executor.completed_tasks.len();

    assert_eq!(completed_count_after_first, completed_count_after_second);
    assert_eq!(
        completed_count_after_first,
        checkpoint.completed_tasks.len()
    );
}

#[tokio::test]
async fn test_restore_checkpoint_state_validates_workflow() {
    use crate::workflow::checkpoint::WorkflowCheckpointService;

    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B")));

    let checkpoint_service = WorkflowCheckpointService::new_default();
    let mut executor =
        WorkflowExecutor::new(workflow).with_checkpoint_service(checkpoint_service.clone());

    executor.execute().await.unwrap();

    let workflow_id = executor.audit_log.tx_id().to_string();
    let checkpoint = checkpoint_service
        .get_latest(&workflow_id)
        .unwrap()
        .unwrap();

    let mut different_workflow = Workflow::new();
    different_workflow.add_task(Box::new(MockTask::new("x", "Task X")));
    different_workflow.add_task(Box::new(MockTask::new("y", "Task Y")));

    let mut different_executor = WorkflowExecutor::new(different_workflow);

    let result = different_executor.restore_checkpoint_state(&checkpoint);
    assert!(result.is_err());

    match result {
        Err(crate::workflow::WorkflowError::WorkflowChanged(_)) => {}
        _ => panic!("Expected WorkflowChanged error"),
    }
}

#[tokio::test]
async fn test_can_resume() {
    use crate::workflow::checkpoint::WorkflowCheckpointService;

    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a")));

    workflow.add_dependency("a", "b").unwrap();

    let checkpoint_service = WorkflowCheckpointService::new_default();
    let executor =
        WorkflowExecutor::new(workflow).with_checkpoint_service(checkpoint_service.clone());

    assert!(!executor.can_resume());

    let mut workflow2 = Workflow::new();
    workflow2.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow2.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a")));
    workflow2.add_dependency("a", "b").unwrap();

    let mut executor2 =
        WorkflowExecutor::new(workflow2).with_checkpoint_service(checkpoint_service.clone());
    executor2.execute().await.unwrap();

    assert!(executor2.can_resume());
}

#[tokio::test]
async fn test_can_resume_returns_false_without_service() {
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));

    let executor = WorkflowExecutor::new(workflow);

    assert!(!executor.can_resume());
}

use super::*;

#[tokio::test]
async fn test_resume_from_checkpoint() {
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
    let checkpoint_id = checkpoint.id;

    let mut new_workflow = Workflow::new();
    new_workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    new_workflow.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a")));
    new_workflow.add_task(Box::new(MockTask::new("c", "Task C").with_dep("a")));

    new_workflow.add_dependency("a", "b").unwrap();
    new_workflow.add_dependency("a", "c").unwrap();

    let mut new_executor =
        WorkflowExecutor::new(new_workflow).with_checkpoint_service(checkpoint_service.clone());

    let result = new_executor.resume_from_checkpoint_id(&checkpoint_id).await;

    assert!(result.is_ok());
    let workflow_result = result.unwrap();

    assert!(workflow_result.success);
    assert_eq!(workflow_result.completed_tasks.len(), 3);
}

#[tokio::test]
async fn test_resume_skip_completed() {
    use crate::workflow::checkpoint::WorkflowCheckpointService;

    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a")));
    workflow.add_task(Box::new(MockTask::new("c", "Task C").with_dep("b")));

    workflow.add_dependency("a", "b").unwrap();
    workflow.add_dependency("b", "c").unwrap();

    let checkpoint_service = WorkflowCheckpointService::new_default();
    let mut executor =
        WorkflowExecutor::new(workflow).with_checkpoint_service(checkpoint_service.clone());

    let workflow_id = executor.audit_log.tx_id().to_string();

    executor.completed_tasks.insert(TaskId::new("a"));
    let partial_checkpoint = WorkflowCheckpoint::from_executor(&workflow_id, 0, &executor, 0);
    checkpoint_service.save(&partial_checkpoint).unwrap();

    let checkpoint_id = partial_checkpoint.id;

    let mut new_workflow = Workflow::new();
    new_workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    new_workflow.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a")));
    new_workflow.add_task(Box::new(MockTask::new("c", "Task C").with_dep("b")));

    new_workflow.add_dependency("a", "b").unwrap();
    new_workflow.add_dependency("b", "c").unwrap();

    let mut new_executor =
        WorkflowExecutor::new(new_workflow).with_checkpoint_service(checkpoint_service.clone());

    let result = new_executor
        .resume_from_checkpoint_id(&checkpoint_id)
        .await
        .unwrap();

    assert!(result.success);
    assert_eq!(result.completed_tasks.len(), 3);

    assert!(result.completed_tasks.contains(&TaskId::new("a")));
    assert!(result.completed_tasks.contains(&TaskId::new("b")));
    assert!(result.completed_tasks.contains(&TaskId::new("c")));
}

#[tokio::test]
async fn test_resume_returns_immediately_if_all_completed() {
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
    let checkpoint_id = checkpoint.id;

    let mut new_workflow = Workflow::new();
    new_workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    new_workflow.add_task(Box::new(MockTask::new("b", "Task B")));

    let mut new_executor =
        WorkflowExecutor::new(new_workflow).with_checkpoint_service(checkpoint_service.clone());

    let result = new_executor
        .resume_from_checkpoint_id(&checkpoint_id)
        .await
        .unwrap();

    assert!(result.success);
    assert_eq!(result.completed_tasks.len(), 2);
}

#[tokio::test]
async fn test_resume_fails_with_invalid_checkpoint() {
    use crate::workflow::checkpoint::{CheckpointId, WorkflowCheckpointService};

    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));

    let checkpoint_service = WorkflowCheckpointService::new_default();
    let mut executor =
        WorkflowExecutor::new(workflow).with_checkpoint_service(checkpoint_service.clone());

    let fake_checkpoint_id = CheckpointId::new();
    let result = executor
        .resume_from_checkpoint_id(&fake_checkpoint_id)
        .await;

    assert!(result.is_err());

    match result {
        Err(crate::workflow::WorkflowError::CheckpointNotFound(_)) => {}
        _ => panic!("Expected CheckpointNotFound error"),
    }
}

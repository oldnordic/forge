use super::*;

#[tokio::test]
async fn test_execute_with_validations() {
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B")));

    let mut executor = WorkflowExecutor::new(workflow);
    let result = executor.execute_with_validations().await;

    assert!(result.is_ok());
    let workflow_result = result.unwrap();
    assert!(workflow_result.success);
}

#[tokio::test]
async fn test_validation_config_builder() {
    use crate::workflow::checkpoint::ValidationCheckpoint;

    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));

    let custom_config = ValidationCheckpoint {
        min_confidence: 0.5,
        warning_threshold: 0.8,
        rollback_on_failure: true,
    };

    let executor = WorkflowExecutor::new(workflow).with_validation_config(custom_config);

    assert!(executor.validation_config.is_some());
    let config = executor.validation_config.unwrap();
    assert_eq!(config.min_confidence, 0.5);
    assert_eq!(config.warning_threshold, 0.8);
    assert!(config.rollback_on_failure);
}

#[tokio::test]
async fn test_validation_warning_continues() {
    use crate::workflow::checkpoint::ValidationCheckpoint;

    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B")));

    let config = ValidationCheckpoint {
        min_confidence: 0.4,
        warning_threshold: 0.9,
        rollback_on_failure: false,
    };

    let mut executor = WorkflowExecutor::new(workflow).with_validation_config(config);

    let result = executor.execute().await.unwrap();

    assert!(result.success);
}

#[test]
fn test_validate_task_result_method() {
    use crate::workflow::checkpoint::ValidationCheckpoint;
    use crate::workflow::task::TaskResult;

    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));

    let config = ValidationCheckpoint::default();
    let executor = WorkflowExecutor::new(workflow).with_validation_config(config);

    let result = TaskResult::Success;
    let validation = executor._validate_task_result(&result);

    assert!(validation.is_ok());
    let v = validation.unwrap();
    assert_eq!(v.confidence, 1.0);
    assert_eq!(
        v.status,
        crate::workflow::checkpoint::ValidationStatus::Passed
    );
}

#[test]
fn test_validate_task_result_no_config() {
    use crate::workflow::task::TaskResult;

    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));

    let executor = WorkflowExecutor::new(workflow);

    let result = TaskResult::Success;
    let validation = executor._validate_task_result(&result);

    assert!(validation.is_err());
}

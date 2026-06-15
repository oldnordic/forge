use super::*;

#[test]
fn test_executor_without_timeout_config() {
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));

    let executor = WorkflowExecutor::new(workflow);

    assert!(executor.timeout_config().is_none());
}

#[test]
fn test_executor_with_timeout_config() {
    use crate::workflow::timeout::TimeoutConfig;

    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));

    let config = TimeoutConfig::new();
    let executor = WorkflowExecutor::new(workflow).with_timeout_config(config);

    assert!(executor.timeout_config().is_some());
    let retrieved_config = executor.timeout_config().unwrap();
    assert!(retrieved_config.task_timeout.is_some());
    assert!(retrieved_config.workflow_timeout.is_some());
}

#[tokio::test]
async fn test_executor_with_task_timeout() {
    use crate::workflow::timeout::{TaskTimeout, TimeoutConfig};

    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));

    let config = TimeoutConfig {
        task_timeout: Some(TaskTimeout::from_millis(100)),
        workflow_timeout: None,
    };

    let mut executor = WorkflowExecutor::new(workflow).with_timeout_config(config);

    let result = executor.execute().await;

    assert!(result.is_ok());
    let workflow_result = result.unwrap();
    assert!(workflow_result.success);
}

#[tokio::test]
async fn test_executor_with_workflow_timeout() {
    use crate::workflow::timeout::{TimeoutConfig, WorkflowTimeout};

    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B")));

    let config = TimeoutConfig {
        task_timeout: None,
        workflow_timeout: Some(WorkflowTimeout::from_secs(5)),
    };

    let mut executor = WorkflowExecutor::new(workflow).with_timeout_config(config);

    let result = executor.execute().await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_task_timeout_records_audit_event() {
    use crate::workflow::timeout::{TaskTimeout, TimeoutConfig};

    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));

    let config = TimeoutConfig {
        task_timeout: Some(TaskTimeout::from_millis(100)),
        workflow_timeout: None,
    };

    let mut executor = WorkflowExecutor::new(workflow).with_timeout_config(config);

    let result = executor.execute().await;

    assert!(result.is_ok());
    assert!(executor.timeout_config().is_some());
}

#[tokio::test]
async fn test_workflow_timeout_records_audit_event() {
    use crate::workflow::timeout::{TimeoutConfig, WorkflowTimeout};

    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));

    let config = TimeoutConfig {
        task_timeout: None,
        workflow_timeout: Some(WorkflowTimeout::from_secs(5)),
    };

    let mut executor = WorkflowExecutor::new(workflow).with_timeout_config(config);

    let result = executor.execute_with_timeout().await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_execute_with_timeout_without_config() {
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("a", "Task A")));

    let mut executor = WorkflowExecutor::new(workflow);

    let result = executor.execute_with_timeout().await;

    assert!(result.is_ok());
    assert!(result.unwrap().success);
}

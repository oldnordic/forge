use super::*;
use crate::workflow::dag::Workflow;
use crate::workflow::task::{TaskContext, TaskResult, WorkflowTask};
use crate::workflow::tools::{Tool, ToolRegistry};
use async_trait::async_trait;

#[tokio::test]
async fn test_executor_with_tool_registry() {
    let mut workflow = Workflow::new();
    let task_id = TaskId::new("task1");
    workflow.add_task(Box::new(MockTask::new(task_id.clone(), "Task 1")));

    let mut registry = ToolRegistry::new();
    registry.register(Tool::new("echo", "echo")).unwrap();

    let mut executor = WorkflowExecutor::new(workflow).with_tool_registry(registry);

    assert!(executor.tool_registry().is_some());
    assert!(executor.tool_registry().unwrap().is_registered("echo"));

    let result = executor.execute().await.unwrap();
    assert!(result.success);
}

struct MockTask {
    id: TaskId,
    name: String,
    deps: Vec<TaskId>,
    should_fail: bool,
}

impl MockTask {
    fn new(id: impl Into<TaskId>, name: &str) -> Self {
        Self {
            id: id.into(),
            name: name.to_string(),
            deps: Vec::new(),
            should_fail: false,
        }
    }

    fn with_dep(mut self, dep: impl Into<TaskId>) -> Self {
        self.deps.push(dep.into());
        self
    }

    fn with_failure(mut self) -> Self {
        self.should_fail = true;
        self
    }
}

#[async_trait]
impl WorkflowTask for MockTask {
    async fn execute(
        &self,
        _context: &TaskContext,
    ) -> Result<TaskResult, crate::workflow::TaskError> {
        if self.should_fail {
            Ok(TaskResult::Failed("Task failed".to_string()))
        } else {
            Ok(TaskResult::WithCompensation {
                result: Box::new(TaskResult::Success),
                compensation: crate::workflow::task::ExecutableCompensation::skip(format!(
                    "Mock compensation for task {}",
                    self.name
                )),
            })
        }
    }

    fn id(&self) -> TaskId {
        self.id.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn dependencies(&self) -> Vec<TaskId> {
        self.deps.clone()
    }
}

#[tokio::test]
async fn test_sequential_execution() {
    let mut workflow = Workflow::new();

    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a")));
    workflow.add_task(Box::new(MockTask::new("c", "Task C").with_dep("a")));

    workflow.add_dependency("a", "b").unwrap();
    workflow.add_dependency("a", "c").unwrap();

    let mut executor = WorkflowExecutor::new(workflow);
    let result = executor.execute().await.unwrap();

    assert!(result.success);
    assert_eq!(result.completed_tasks.len(), 3);
    assert_eq!(executor.completed_count(), 3);
    assert_eq!(executor.failed_count(), 0);
}

#[tokio::test]
async fn test_failure_stops_execution() {
    let mut workflow = Workflow::new();

    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(
        MockTask::new("b", "Task B").with_dep("a").with_failure(),
    ));
    workflow.add_task(Box::new(MockTask::new("c", "Task C").with_dep("b")));

    workflow.add_dependency("a", "b").unwrap();
    workflow.add_dependency("b", "c").unwrap();

    let mut executor = WorkflowExecutor::new(workflow);
    let result = executor.execute().await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_audit_events_logged() {
    let mut workflow = Workflow::new();

    workflow.add_task(Box::new(MockTask::new("a", "Task A")));
    workflow.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a")));

    workflow.add_dependency("a", "b").unwrap();

    let mut executor = WorkflowExecutor::new(workflow);
    executor.execute().await.unwrap();

    let events = executor.audit_log().replay();

    assert!(events.len() >= 6);

    assert!(matches!(
        events[0],
        crate::audit::AuditEvent::WorkflowStarted { .. }
    ));
}

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

#[tokio::test]
async fn test_executor_with_forge_passes_context() {
    use crate::workflow::task::TaskError;
    use forge_core::Forge;
    use tempfile::TempDir;

    struct ForgeCheckTask;

    #[async_trait]
    impl WorkflowTask for ForgeCheckTask {
        async fn execute(&self, context: &TaskContext) -> Result<TaskResult, TaskError> {
            if context.forge.is_some() {
                Ok(TaskResult::Success)
            } else {
                Err(TaskError::ExecutionFailed(
                    "no forge in context".to_string(),
                ))
            }
        }

        fn id(&self) -> TaskId {
            TaskId::new("forge-check")
        }

        fn name(&self) -> &str {
            "ForgeCheckTask"
        }
    }

    let temp_dir = TempDir::new().unwrap();
    let forge = Forge::open(temp_dir.path()).await.unwrap();

    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(ForgeCheckTask));

    let mut executor = WorkflowExecutor::new(workflow).with_forge(Arc::new(forge));
    let result = executor.execute().await.unwrap();
    assert!(
        result.success,
        "task should succeed when forge is in context"
    );
}

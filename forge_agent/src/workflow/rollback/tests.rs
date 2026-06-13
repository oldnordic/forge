use super::*;
use crate::audit::AuditLog;
use crate::workflow::dag::Workflow;
use crate::workflow::task::{
    CompensationAction, CompensationType, ExecutableCompensation, TaskContext, TaskError, TaskId,
    TaskResult, WorkflowTask,
};
use async_trait::async_trait;
use std::fs::File;
use std::io::Write;
use std::path::Path;

#[test]
fn test_tool_compensation_creation() {
    let comp = ToolCompensation::new("Test compensation", |_ctx| Ok(TaskResult::Success));
    assert_eq!(comp.description, "Test compensation");
}

#[test]
fn test_tool_compensation_execute() {
    let comp = ToolCompensation::new("Execute test", |_ctx| Ok(TaskResult::Success));
    let context = TaskContext::new("test", TaskId::new("a"));
    let result = comp.execute(&context).unwrap();
    assert_eq!(result, TaskResult::Success);
}

#[test]
fn test_tool_compensation_execute_error() {
    let comp = ToolCompensation::new("Execute test", |_ctx| {
        Err(TaskError::ExecutionFailed("Test error".to_string()))
    });
    let context = TaskContext::new("test", TaskId::new("a"));
    let result = comp.execute(&context);
    assert!(result.is_err());
}

#[test]
fn test_tool_compensation_skip() {
    let comp = ToolCompensation::skip("No action needed");
    let context = TaskContext::new("test", TaskId::new("a"));
    let result = comp.execute(&context).unwrap();
    assert_eq!(result, TaskResult::Skipped);
}

#[test]
fn test_tool_compensation_retry() {
    let comp = ToolCompensation::retry("Retry recommended");
    let context = TaskContext::new("test", TaskId::new("a"));
    let result = comp.execute(&context).unwrap();
    assert_eq!(result, TaskResult::Skipped);
}

#[test]
fn test_tool_compensation_file() {
    let temp_file = "/tmp/test_tool_compensation.txt";
    let mut file = File::create(temp_file).unwrap();
    writeln!(file, "test content").unwrap();
    drop(file);

    assert!(Path::new(temp_file).exists());

    let comp = ToolCompensation::file_compensation(temp_file);
    let context = TaskContext::new("test", TaskId::new("a"));
    let result = comp.execute(&context);

    assert!(result.is_ok());
    assert!(!Path::new(temp_file).exists());
}

#[test]
fn test_tool_compensation_from_compensation_action() {
    let skip_action = CompensationAction::skip("Skip action");
    let skip_comp: ToolCompensation = skip_action.into();
    assert_eq!(skip_comp.description, "Skip action");

    let retry_action = CompensationAction::retry("Retry action");
    let retry_comp: ToolCompensation = retry_action.into();
    assert_eq!(retry_comp.description, "Retry action");

    let undo_action = CompensationAction::undo("Undo action");
    let undo_comp: ToolCompensation = undo_action.into();
    assert!(undo_comp.description.contains("no undo function available"));
}

#[test]
fn test_compensation_registry_new() {
    let registry = CompensationRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
}

#[test]
fn test_compensation_registry_register() {
    let mut registry = CompensationRegistry::new();
    let task_id = TaskId::new("task-1");
    let comp = ToolCompensation::skip("Test");

    registry.register(task_id.clone(), comp);

    assert_eq!(registry.len(), 1);
    assert!(registry.has_compensation(&task_id));
}

#[test]
fn test_compensation_registry_get() {
    let mut registry = CompensationRegistry::new();
    let task_id = TaskId::new("task-1");
    let comp = ToolCompensation::new("Test", |_ctx| Ok(TaskResult::Success));

    registry.register(task_id.clone(), comp);

    let retrieved = registry.get(&task_id);
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().description, "Test");

    let missing = registry.get(&TaskId::new("missing"));
    assert!(missing.is_none());
}

#[test]
fn test_compensation_registry_remove() {
    let mut registry = CompensationRegistry::new();
    let task_id = TaskId::new("task-1");
    let comp = ToolCompensation::skip("Test");

    registry.register(task_id.clone(), comp);
    assert_eq!(registry.len(), 1);

    let removed = registry.remove(&task_id);
    assert!(removed.is_some());
    assert_eq!(registry.len(), 0);
    assert!(!registry.has_compensation(&task_id));

    let removed_again = registry.remove(&task_id);
    assert!(removed_again.is_none());
}

#[test]
fn test_compensation_registry_validate_coverage() {
    let mut registry = CompensationRegistry::new();

    let task1 = TaskId::new("task-1");
    let task2 = TaskId::new("task-2");
    let task3 = TaskId::new("task-3");

    registry.register(task1.clone(), ToolCompensation::skip("Test 1"));
    registry.register(task2.clone(), ToolCompensation::skip("Test 2"));

    let report = registry.validate_coverage(&[task1.clone(), task2.clone(), task3.clone()]);

    assert_eq!(report.tasks_with_compensation.len(), 2);
    assert!(report.tasks_with_compensation.contains(&task1));
    assert!(report.tasks_with_compensation.contains(&task2));

    assert_eq!(report.tasks_without_compensation.len(), 1);
    assert!(report.tasks_without_compensation.contains(&task3));

    assert!((report.coverage_percentage - 0.666).abs() < 0.01);
}

#[test]
fn test_compensation_registry_register_file_creation() {
    let mut registry = CompensationRegistry::new();
    let task_id = TaskId::new("task-1");

    registry.register_file_creation(task_id.clone(), "/tmp/test.txt");

    assert!(registry.has_compensation(&task_id));
    let comp = registry.get(&task_id).unwrap();
    assert!(comp.description.contains("Delete file"));
}

#[test]
fn test_compensation_registry_register_process_spawn() {
    let mut registry = CompensationRegistry::new();
    let task_id = TaskId::new("task-1");

    registry.register_process_spawn(task_id.clone(), 12345);

    assert!(registry.has_compensation(&task_id));
    let comp = registry.get(&task_id).unwrap();
    assert!(comp.description.contains("Terminate process"));
}

#[test]
fn test_compensation_registry_task_ids() {
    let mut registry = CompensationRegistry::new();

    let task1 = TaskId::new("task-1");
    let task2 = TaskId::new("task-2");

    registry.register(task1.clone(), ToolCompensation::skip("Test 1"));
    registry.register(task2.clone(), ToolCompensation::skip("Test 2"));

    let ids = registry.task_ids();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&task1));
    assert!(ids.contains(&task2));
}

#[test]
fn test_compensation_registry_default() {
    let registry = CompensationRegistry::default();
    assert!(registry.is_empty());
}

struct MockTaskWithCompensation {
    id: TaskId,
    name: String,
    deps: Vec<TaskId>,
}

impl MockTaskWithCompensation {
    fn new(id: impl Into<TaskId>, name: &str) -> Self {
        Self {
            id: id.into(),
            name: name.to_string(),
            deps: Vec::new(),
        }
    }
}

#[async_trait]
impl WorkflowTask for MockTaskWithCompensation {
    async fn execute(&self, _context: &TaskContext) -> Result<TaskResult, TaskError> {
        Ok(TaskResult::Success)
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

#[test]
fn test_compensation_action_creation() {
    let skip = CompensationAction::skip("Read-only operation");
    assert_eq!(skip.action_type, CompensationType::Skip);
    assert_eq!(skip.description, "Read-only operation");

    let retry = CompensationAction::retry("Transient network error");
    assert_eq!(retry.action_type, CompensationType::Retry);

    let undo = CompensationAction::undo("Delete file");
    assert_eq!(undo.action_type, CompensationType::UndoFunction);
}

#[test]
fn test_executable_compensation_creation() {
    let skip = ExecutableCompensation::skip("No action needed");
    assert_eq!(skip.action.action_type, CompensationType::Skip);

    let retry = ExecutableCompensation::retry("Retry later");
    assert_eq!(retry.action.action_type, CompensationType::Retry);

    let undo = ExecutableCompensation::with_undo("Execute undo", |_ctx| Ok(TaskResult::Success));
    assert_eq!(undo.action.action_type, CompensationType::UndoFunction);
}

#[test]
fn test_executable_compensation_execute() {
    let skip = ExecutableCompensation::skip("No action needed");
    let context = TaskContext::new("test", TaskId::new("a"));
    let result = skip.execute(&context).unwrap();
    assert_eq!(result, TaskResult::Skipped);

    let retry = ExecutableCompensation::retry("Retry later");
    let result = retry.execute(&context).unwrap();
    assert_eq!(result, TaskResult::Skipped);

    let undo = ExecutableCompensation::with_undo("Execute undo", |_ctx| Ok(TaskResult::Success));
    let result = undo.execute(&context).unwrap();
    assert_eq!(result, TaskResult::Success);
}

#[test]
fn test_rollback_engine_creation() {
    let engine = RollbackEngine::new();
    let _ = &engine;
}

#[tokio::test]
async fn test_rollback_report_creation() {
    let report = RollbackReport::new();
    assert_eq!(report.total_processed(), 0);
    assert!(report.rolled_back_tasks.is_empty());
    assert!(report.skipped_tasks.is_empty());
    assert!(report.failed_compensations.is_empty());
}

#[test]
fn test_compensation_report_calculation() {
    let coverage = CompensationReport::calculate(5, 10);
    assert_eq!(coverage, 0.5);

    let full_coverage = CompensationReport::calculate(10, 10);
    assert_eq!(full_coverage, 1.0);

    let no_tasks = CompensationReport::calculate(0, 0);
    assert_eq!(no_tasks, 1.0);
}

#[test]
fn test_find_prerequisite_tasks() {
    let mut workflow = Workflow::new();

    workflow.add_task(Box::new(MockTaskWithCompensation::new("a", "Task A")));
    workflow.add_task(Box::new(MockTaskWithCompensation::new("b", "Task B")));
    workflow.add_task(Box::new(MockTaskWithCompensation::new("c", "Task C")));
    workflow.add_task(Box::new(MockTaskWithCompensation::new("d", "Task D")));

    workflow.add_dependency("a", "b").unwrap();
    workflow.add_dependency("a", "c").unwrap();
    workflow.add_dependency("b", "d").unwrap();
    workflow.add_dependency("c", "d").unwrap();

    let engine = RollbackEngine::new();
    let failed_idx = *workflow.task_map.get(&TaskId::new("d")).unwrap();

    let predecessors = engine
        .find_prerequisite_tasks(&workflow, failed_idx)
        .unwrap();

    assert_eq!(predecessors.len(), 4);
    assert!(predecessors.contains(&TaskId::new("a")));
    assert!(predecessors.contains(&TaskId::new("b")));
    assert!(predecessors.contains(&TaskId::new("c")));
    assert!(predecessors.contains(&TaskId::new("d")));
}

#[test]
fn test_diamond_dependency_rollback() {
    let mut workflow = Workflow::new();

    workflow.add_task(Box::new(MockTaskWithCompensation::new("a", "Task A")));
    workflow.add_task(Box::new(MockTaskWithCompensation::new("b", "Task B")));
    workflow.add_task(Box::new(MockTaskWithCompensation::new("c", "Task C")));
    workflow.add_task(Box::new(MockTaskWithCompensation::new("d", "Task D")));

    workflow.add_dependency("a", "b").unwrap();
    workflow.add_dependency("a", "c").unwrap();
    workflow.add_dependency("b", "d").unwrap();
    workflow.add_dependency("c", "d").unwrap();

    let engine = RollbackEngine::new();

    let rollback_set = engine
        .find_rollback_set(&workflow, &TaskId::new("d"), RollbackStrategy::AllDependent)
        .unwrap();

    assert_eq!(rollback_set.len(), 4);
    assert_eq!(rollback_set[0], TaskId::new("d"));
    assert_eq!(rollback_set[rollback_set.len() - 1], TaskId::new("a"));
}

#[test]
fn test_reverse_execution_order() {
    let mut workflow = Workflow::new();

    workflow.add_task(Box::new(MockTaskWithCompensation::new("a", "Task A")));
    workflow.add_task(Box::new(MockTaskWithCompensation::new("b", "Task B")));
    workflow.add_task(Box::new(MockTaskWithCompensation::new("c", "Task C")));

    workflow.add_dependency("a", "b").unwrap();
    workflow.add_dependency("b", "c").unwrap();

    let engine = RollbackEngine::new();
    let failed_idx = *workflow.task_map.get(&TaskId::new("c")).unwrap();

    let predecessors = engine
        .find_prerequisite_tasks(&workflow, failed_idx)
        .unwrap();
    let rollback_order = engine
        .reverse_execution_order(&workflow, predecessors)
        .unwrap();

    assert_eq!(rollback_order.len(), 3);
    assert_eq!(rollback_order[0], TaskId::new("c"));
    assert_eq!(rollback_order[1], TaskId::new("b"));
    assert_eq!(rollback_order[2], TaskId::new("a"));
}

#[tokio::test]
async fn test_execute_rollback() {
    let mut workflow = Workflow::new();

    workflow.add_task(Box::new(MockTaskWithCompensation::new("a", "Task A")));
    workflow.add_task(Box::new(MockTaskWithCompensation::new("b", "Task B")));

    workflow.add_dependency("a", "b").unwrap();

    let engine = RollbackEngine::new();
    let mut audit_log = AuditLog::new();
    let registry = CompensationRegistry::new();

    let report = engine
        .execute_rollback(
            &workflow,
            vec![TaskId::new("b")],
            "test_workflow",
            &mut audit_log,
            &registry,
        )
        .await
        .unwrap();

    assert_eq!(report.skipped_tasks.len(), 1);
    assert_eq!(report.skipped_tasks[0], TaskId::new("b"));
    assert!(report.rolled_back_tasks.is_empty());
    assert!(report.failed_compensations.is_empty());

    let events = audit_log.replay();
    assert!(events
        .iter()
        .any(|e| matches!(e, crate::audit::AuditEvent::WorkflowTaskRolledBack { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, crate::audit::AuditEvent::WorkflowRolledBack { .. })));
}

#[tokio::test]
async fn test_execute_rollback_with_compensation() {
    let mut workflow = Workflow::new();

    workflow.add_task(Box::new(MockTaskWithCompensation::new("a", "Task A")));
    workflow.add_task(Box::new(MockTaskWithCompensation::new("b", "Task B")));

    workflow.add_dependency("a", "b").unwrap();

    let engine = RollbackEngine::new();
    let mut audit_log = AuditLog::new();
    let mut registry = CompensationRegistry::new();

    registry.register(
        TaskId::new("b"),
        ToolCompensation::skip("Test compensation"),
    );

    let report = engine
        .execute_rollback(
            &workflow,
            vec![TaskId::new("b")],
            "test_workflow",
            &mut audit_log,
            &registry,
        )
        .await
        .unwrap();

    assert_eq!(report.rolled_back_tasks.len(), 1);
    assert_eq!(report.rolled_back_tasks[0], TaskId::new("b"));
    assert!(report.skipped_tasks.is_empty());
    assert!(report.failed_compensations.is_empty());

    let events = audit_log.replay();
    assert!(events
        .iter()
        .any(|e| matches!(e, crate::audit::AuditEvent::WorkflowTaskRolledBack { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, crate::audit::AuditEvent::WorkflowRolledBack { .. })));
}

#[tokio::test]
async fn test_execute_rollback_mixed_compensation() {
    let mut workflow = Workflow::new();

    workflow.add_task(Box::new(MockTaskWithCompensation::new("a", "Task A")));
    workflow.add_task(Box::new(MockTaskWithCompensation::new("b", "Task B")));
    workflow.add_task(Box::new(MockTaskWithCompensation::new("c", "Task C")));

    workflow.add_dependency("a", "b").unwrap();
    workflow.add_dependency("b", "c").unwrap();

    let engine = RollbackEngine::new();
    let mut audit_log = AuditLog::new();
    let mut registry = CompensationRegistry::new();

    registry.register(
        TaskId::new("a"),
        ToolCompensation::skip("Test compensation"),
    );

    let report = engine
        .execute_rollback(
            &workflow,
            vec![TaskId::new("a"), TaskId::new("b"), TaskId::new("c")],
            "test_workflow",
            &mut audit_log,
            &registry,
        )
        .await
        .unwrap();

    assert_eq!(report.rolled_back_tasks.len(), 1);
    assert_eq!(report.rolled_back_tasks[0], TaskId::new("a"));
    assert_eq!(report.skipped_tasks.len(), 2);
    assert!(report.skipped_tasks.contains(&TaskId::new("b")));
    assert!(report.skipped_tasks.contains(&TaskId::new("c")));
    assert!(report.failed_compensations.is_empty());
}

#[test]
fn test_validate_compensation_coverage() {
    let mut workflow = Workflow::new();

    workflow.add_task(Box::new(MockTaskWithCompensation::new("a", "Task A")));
    workflow.add_task(Box::new(MockTaskWithCompensation::new("b", "Task B")));

    workflow.add_dependency("a", "b").unwrap();

    let mut registry = CompensationRegistry::new();
    registry.register(TaskId::new("a"), ToolCompensation::skip("undo a"));

    let engine = RollbackEngine::new();
    let report = engine.validate_compensation_coverage(&workflow, &registry);

    assert_eq!(report.tasks_with_compensation.len(), 1);
    assert!(report.tasks_with_compensation.contains(&TaskId::new("a")));
    assert_eq!(report.tasks_without_compensation.len(), 1);
    assert!(report
        .tasks_without_compensation
        .contains(&TaskId::new("b")));
    assert!((report.coverage_percentage - 0.5).abs() < 0.001);
}

#[test]
fn test_saga_compensation_traverses_predecessors() {
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTaskWithCompensation::new("a", "Task A")));
    workflow.add_task(Box::new(MockTaskWithCompensation::new("b", "Task B")));
    workflow.add_task(Box::new(MockTaskWithCompensation::new("c", "Task C")));
    workflow.add_dependency("a", "b").unwrap();
    workflow.add_dependency("b", "c").unwrap();

    let engine = RollbackEngine::new();
    let failed_idx = *workflow.task_map.get(&TaskId::new("b")).unwrap();
    let predecessors = engine
        .find_prerequisite_tasks(&workflow, failed_idx)
        .unwrap();

    assert!(
        predecessors.contains(&TaskId::new("b")),
        "failed task must be included"
    );
    assert!(
        predecessors.contains(&TaskId::new("a")),
        "a completed before b — must be rolled back"
    );
    assert!(
        !predecessors.contains(&TaskId::new("c")),
        "c never ran — must not be in rollback set"
    );
}

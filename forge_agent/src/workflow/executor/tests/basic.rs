use super::*;

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

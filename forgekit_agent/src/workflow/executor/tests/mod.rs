use super::*;
use crate::workflow::dag::Workflow;
use crate::workflow::task::{TaskContext, TaskResult, WorkflowTask};
use crate::workflow::tools::{Tool, ToolRegistry};
use async_trait::async_trait;

mod basic;
mod cancellation;
mod checkpoint;
mod compensation;
mod forge_context;
mod parallel;
mod resume;
mod rollback;
mod timeout;
mod validation;

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

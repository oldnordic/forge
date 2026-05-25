use crate::workflow::task::{
    CompensationAction, TaskContext, TaskError, TaskId, TaskResult, WorkflowTask,
};

/// Task that executes an agent loop for AI-driven operations.
///
/// Wraps the AgentLoop as a workflow task for multi-step AI operations.
pub struct AgentLoopTask {
    id: TaskId,
    name: String,
    query: String,
}

impl AgentLoopTask {
    /// Creates a new AgentLoopTask with the given query.
    pub fn new(id: TaskId, name: String, query: impl Into<String>) -> Self {
        Self {
            id,
            name,
            query: query.into(),
        }
    }

    /// Gets the query for this task.
    pub fn query(&self) -> &str {
        &self.query
    }
}

#[async_trait::async_trait]
impl WorkflowTask for AgentLoopTask {
    async fn execute(&self, context: &TaskContext) -> Result<TaskResult, TaskError> {
        let forge = context.forge.as_ref().ok_or_else(|| {
            TaskError::ExecutionFailed("Forge SDK not available in TaskContext".to_string())
        })?;

        let mut agent_loop = crate::r#loop::AgentLoop::new(std::sync::Arc::new(forge.clone()));
        let result = agent_loop
            .run(&self.query)
            .await
            .map_err(|e| TaskError::ExecutionFailed(format!("Agent loop failed: {}", e)))?;

        let _ = result;
        Ok(TaskResult::Success)
    }

    fn id(&self) -> TaskId {
        self.id.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn compensation(&self) -> Option<CompensationAction> {
        Some(CompensationAction::skip(
            "Read-only agent loop - no undo needed in v0.4",
        ))
    }
}

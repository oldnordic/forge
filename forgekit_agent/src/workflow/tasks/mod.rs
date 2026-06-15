//! Built-in task implementations for common workflow operations.
//!
//! Provides pre-built task types for graph queries, agent loops, shell commands,
//! and simple function wrapping.

mod agent_loop;
mod file_edit;
mod graph_query;
mod shell;
mod tool;

pub use agent_loop::AgentLoopTask;
pub use file_edit::FileEditTask;
pub use graph_query::{GraphQueryTask, GraphQueryType};
pub use shell::{ShellCommandConfig, ShellCommandTask};
pub use tool::ToolTask;

use crate::workflow::task::{TaskContext, TaskError, TaskId, TaskResult, WorkflowTask};
use std::future::Future;
use std::pin::Pin;

type AsyncTaskFn = Box<
    dyn Fn(&TaskContext) -> Pin<Box<dyn Future<Output = Result<TaskResult, TaskError>> + Send>>
        + Send
        + Sync,
>;

/// Task that wraps an async function for easy workflow definition.
///
/// Useful for simple workflows without custom task types.
///
/// # Example
///
/// ```ignore
/// use forgekit_agent::workflow::tasks::FunctionTask;
/// use forgekit_agent::workflow::TaskId;
///
/// let task = FunctionTask::new(
///     TaskId::new("my_task"),
///     "My Task".to_string(),
///     |ctx| async {
///         // Do work here
///         Ok(TaskResult::Success)
///     }
/// );
/// ```
pub struct FunctionTask {
    id: TaskId,
    name: String,
    f: AsyncTaskFn,
}

impl FunctionTask {
    /// Creates a new FunctionTask with the given ID, name, and async function.
    pub fn new<F, Fut>(id: TaskId, name: String, f: F) -> Self
    where
        F: Fn(&TaskContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<TaskResult, TaskError>> + Send + 'static,
    {
        Self {
            id,
            name,
            f: Box::new(move |ctx| {
                Box::pin(f(ctx))
                    as Pin<Box<dyn Future<Output = Result<TaskResult, TaskError>> + Send>>
            }),
        }
    }
}

#[async_trait::async_trait]
impl WorkflowTask for FunctionTask {
    async fn execute(&self, context: &TaskContext) -> Result<TaskResult, TaskError> {
        (self.f)(context).await
    }

    fn id(&self) -> TaskId {
        self.id.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests;

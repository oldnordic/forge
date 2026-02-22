//! Task composition helpers for complex workflows.
//!
//! Provides combinators for conditional execution, error recovery,
//! and parallel task patterns.

use crate::workflow::task::{TaskContext, TaskError, TaskId, TaskResult, WorkflowTask};
use async_trait::async_trait;
use std::pin::Pin;
use std::future::Future;

/// Task that executes conditionally based on another task's result.
///
/// The condition task is executed first, then based on its result,
/// either the then_task or else_task is executed.
pub struct ConditionalTask {
    /// Task that determines which branch to execute
    condition_task: Box<dyn WorkflowTask>,
    /// Task to execute if condition succeeds
    then_task: Box<dyn WorkflowTask>,
    /// Optional task to execute if condition fails
    else_task: Option<Box<dyn WorkflowTask>>,
}

impl ConditionalTask {
    /// Creates a new conditional task.
    ///
    /// # Arguments
    ///
    /// * `condition_task` - Task whose result determines the branch
    /// * `then_task` - Task executed on success
    /// * `else_task` - Optional task executed on failure
    ///
    /// # Example
    ///
    /// ```ignore
    /// let condition = Box::new(FunctionTask::new(
    ///     TaskId::new("check"),
    ///     "Check".to_string(),
    ///     |_ctx| async { Ok(TaskResult::Success) }
    /// ));
    /// let then_branch = Box::new(FunctionTask::new(
    ///     TaskId::new("then"),
    ///     "Then".to_string(),
    ///     |_ctx| async { Ok(TaskResult::Success) }
    /// ));
    /// let task = ConditionalTask::new(condition, then_branch, None);
    /// ```
    pub fn new(
        condition_task: Box<dyn WorkflowTask>,
        then_task: Box<dyn WorkflowTask>,
        else_task: Option<Box<dyn WorkflowTask>>,
    ) -> Self {
        Self {
            condition_task,
            then_task,
            else_task,
        }
    }

    /// Creates a conditional task with an else branch.
    pub fn with_else(
        condition_task: Box<dyn WorkflowTask>,
        then_task: Box<dyn WorkflowTask>,
        else_task: Box<dyn WorkflowTask>,
    ) -> Self {
        Self::new(condition_task, then_task, Some(else_task))
    }
}

#[async_trait]
impl WorkflowTask for ConditionalTask {
    async fn execute(&self, context: &TaskContext) -> Result<TaskResult, TaskError> {
        // Execute condition task
        let condition_result = self.condition_task.execute(context).await?;

        match condition_result {
            TaskResult::Success => {
                // Execute then task
                self.then_task.execute(context).await
            }
            TaskResult::Failed(_) | TaskResult::Skipped => {
                // Execute else task if available, otherwise return condition result
                if let Some(else_task) = &self.else_task {
                    else_task.execute(context).await
                } else {
                    Ok(condition_result)
                }
            }
        }
    }

    fn id(&self) -> TaskId {
        self.condition_task.id()
    }

    fn name(&self) -> &str {
        self.condition_task.name()
    }

    fn dependencies(&self) -> Vec<TaskId> {
        self.condition_task.dependencies()
    }
}

/// Task that executes with error recovery.
///
/// The try_task is executed first. If it fails, the catch_task is
/// executed instead, allowing the workflow to continue gracefully.
pub struct TryCatchTask {
    /// Task to attempt
    try_task: Box<dyn WorkflowTask>,
    /// Task to execute on failure
    catch_task: Box<dyn WorkflowTask>,
}

impl TryCatchTask {
    /// Creates a new try-catch task.
    ///
    /// # Arguments
    ///
    /// * `try_task` - Task to attempt
    /// * `catch_task` - Task executed on try_task failure
    ///
    /// # Example
    ///
    /// ```ignore
    /// let try_task = Box::new(FunctionTask::new(
    ///     TaskId::new("risky"),
    ///     "Risky Operation".to_string(),
    ///     |_ctx| async { Ok(TaskResult::Failed("error".to_string())) }
    /// ));
    /// let catch_task = Box::new(FunctionTask::new(
    ///     TaskId::new("recover"),
    ///     "Recovery".to_string(),
    ///     |_ctx| async { Ok(TaskResult::Success) }
    /// ));
    /// let task = TryCatchTask::new(try_task, catch_task);
    /// ```
    pub fn new(try_task: Box<dyn WorkflowTask>, catch_task: Box<dyn WorkflowTask>) -> Self {
        Self {
            try_task,
            catch_task,
        }
    }
}

#[async_trait]
impl WorkflowTask for TryCatchTask {
    async fn execute(&self, context: &TaskContext) -> Result<TaskResult, TaskError> {
        // Execute try task
        let try_result = self.try_task.execute(context).await;

        match try_result {
            Ok(TaskResult::Success) => try_result,
            Ok(TaskResult::Failed(_)) | Ok(TaskResult::Skipped) => {
                // Execute catch task on failure
                self.catch_task.execute(context).await
            }
            Err(_) => {
                // Execute catch task on error
                self.catch_task.execute(context).await
            }
        }
    }

    fn id(&self) -> TaskId {
        self.try_task.id()
    }

    fn name(&self) -> &str {
        self.try_task.name()
    }

    fn dependencies(&self) -> Vec<TaskId> {
        self.try_task.dependencies()
    }
}

/// Task that executes multiple tasks (stub for Phase 12).
///
/// In Phase 8, tasks are executed sequentially. True parallelism
/// will be implemented in Phase 12.
pub struct ParallelTasks {
    /// Tasks to execute
    tasks: Vec<Box<dyn WorkflowTask>>,
}

impl ParallelTasks {
    /// Creates a new parallel tasks wrapper.
    ///
    /// # Arguments
    ///
    /// * `tasks` - Vector of tasks to execute
    ///
    /// # Note
    ///
    /// In Phase 8, tasks execute sequentially. Parallel execution
    /// will be implemented in Phase 12.
    pub fn new(tasks: Vec<Box<dyn WorkflowTask>>) -> Self {
        Self { tasks }
    }
}

#[async_trait]
impl WorkflowTask for ParallelTasks {
    async fn execute(&self, context: &TaskContext) -> Result<TaskResult, TaskError> {
        // Sequential execution in Phase 8 (parallel in Phase 12)
        for task in &self.tasks {
            task.execute(context).await?;
        }
        Ok(TaskResult::Success)
    }

    fn id(&self) -> TaskId {
        TaskId::new("parallel_tasks")
    }

    fn name(&self) -> &str {
        "Parallel Tasks"
    }

    fn dependencies(&self) -> Vec<TaskId> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::tasks::FunctionTask;

    #[tokio::test]
    async fn test_conditional_task_then_branch() {
        let condition = Box::new(FunctionTask::new(
            TaskId::new("check"),
            "Check".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        ));

        let then_task = Box::new(FunctionTask::new(
            TaskId::new("then"),
            "Then".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        ));

        let conditional = ConditionalTask::new(condition, then_task, None);
        let context = TaskContext::new("workflow-1", TaskId::new("check"));

        let result = conditional.execute(&context).await.unwrap();
        assert_eq!(result, TaskResult::Success);
    }

    #[tokio::test]
    async fn test_conditional_task_else_branch() {
        let condition = Box::new(FunctionTask::new(
            TaskId::new("check"),
            "Check".to_string(),
            |_ctx| async { Ok(TaskResult::Failed("error".to_string())) },
        ));

        let then_task = Box::new(FunctionTask::new(
            TaskId::new("then"),
            "Then".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        ));

        let else_task = Box::new(FunctionTask::new(
            TaskId::new("else"),
            "Else".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        ));

        let conditional = ConditionalTask::with_else(condition, then_task, else_task);
        let context = TaskContext::new("workflow-1", TaskId::new("check"));

        let result = conditional.execute(&context).await.unwrap();
        assert_eq!(result, TaskResult::Success);
    }

    #[tokio::test]
    async fn test_conditional_task_no_else_returns_failure() {
        let condition = Box::new(FunctionTask::new(
            TaskId::new("check"),
            "Check".to_string(),
            |_ctx| async { Ok(TaskResult::Failed("error".to_string())) },
        ));

        let then_task = Box::new(FunctionTask::new(
            TaskId::new("then"),
            "Then".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        ));

        let conditional = ConditionalTask::new(condition, then_task, None);
        let context = TaskContext::new("workflow-1", TaskId::new("check"));

        let result = conditional.execute(&context).await.unwrap();
        assert!(matches!(result, TaskResult::Failed(_)));
    }

    #[tokio::test]
    async fn test_try_catch_task_success() {
        let try_task = Box::new(FunctionTask::new(
            TaskId::new("risky"),
            "Risky".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        ));

        let catch_task = Box::new(FunctionTask::new(
            TaskId::new("recover"),
            "Recover".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        ));

        let try_catch = TryCatchTask::new(try_task, catch_task);
        let context = TaskContext::new("workflow-1", TaskId::new("risky"));

        let result = try_catch.execute(&context).await.unwrap();
        assert_eq!(result, TaskResult::Success);
    }

    #[tokio::test]
    async fn test_try_catch_task_failure_recovery() {
        let try_task = Box::new(FunctionTask::new(
            TaskId::new("risky"),
            "Risky".to_string(),
            |_ctx| async { Ok(TaskResult::Failed("error".to_string())) },
        ));

        let catch_task = Box::new(FunctionTask::new(
            TaskId::new("recover"),
            "Recover".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        ));

        let try_catch = TryCatchTask::new(try_task, catch_task);
        let context = TaskContext::new("workflow-1", TaskId::new("risky"));

        let result = try_catch.execute(&context).await.unwrap();
        assert_eq!(result, TaskResult::Success);
    }

    #[tokio::test]
    async fn test_parallel_tasks_sequential_stub() {
        let task1 = Box::new(FunctionTask::new(
            TaskId::new("task1"),
            "Task 1".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        ));

        let task2 = Box::new(FunctionTask::new(
            TaskId::new("task2"),
            "Task 2".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        ));

        let parallel = ParallelTasks::new(vec![task1, task2]);
        let context = TaskContext::new("workflow-1", TaskId::new("parallel_tasks"));

        let result = parallel.execute(&context).await.unwrap();
        assert_eq!(result, TaskResult::Success);
    }

    #[tokio::test]
    async fn test_parallel_tasks_failure_stops() {
        let task1 = Box::new(FunctionTask::new(
            TaskId::new("task1"),
            "Task 1".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        ));

        let task2 = Box::new(FunctionTask::new(
            TaskId::new("task2"),
            "Task 2".to_string(),
            |_ctx| async { Err(TaskError::ExecutionFailed("error".to_string())) },
        ));

        let parallel = ParallelTasks::new(vec![task1, task2]);
        let context = TaskContext::new("workflow-1", TaskId::new("parallel_tasks"));

        let result = parallel.execute(&context).await;
        assert!(result.is_err());
    }
}

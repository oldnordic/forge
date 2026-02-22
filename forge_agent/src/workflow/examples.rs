//! Workflow examples demonstrating common patterns.
//!
//! This module provides example workflows that can be used as documentation
//! and templates for building custom workflows.
//!
//! # Cooperative Cancellation Examples
//!
//! This module includes examples demonstrating cooperative cancellation patterns:
//!
//! - **Polling Pattern**: [`CancellationAwareTask`] shows how to poll the cancellation
//!   token in long-running loops
//! - **Tokio Select Pattern**: [`PollingTask`] demonstrates using `tokio::select!`
//!   to race between work completion and cancellation
//! - **Timeout + Cancellation**: [`TimeoutAndCancellationTask`] shows handling both
//!   timeout and cancellation simultaneously
//!
//! # Best Practices
//!
//! ## When to Use Polling vs. Waiting
//!
//! - **Use polling** (`poll_cancelled()`) in tight loops where you check frequently
//! - **Use waiting** (`wait_cancelled()`) with `tokio::select!` when racing cancellation
//!   against other async operations
//!
//! ## Handling Cleanup on Cancellation
//!
//! When a task is cancelled, it should clean up resources gracefully:
//!
//! ```ignore
//! while !context.cancellation_token().map_or(false, |t| t.poll_cancelled()) {
//!     // Do work
//!     if cancelled {
//!         // Clean up resources
//!         return Ok(TaskResult::Cancelled);
//!     }
//! }
//! ```
//!
//! ## Interaction with Timeouts
//!
//! Tasks can be cancelled by either a timeout or an explicit cancellation signal.
//! Always check `context.cancellation_token()` even when using timeouts.
//!
//! ## Common Pitfalls
//!
//! - **Blocking code**: Long-running synchronous operations prevent cancellation checking
//! - **Forgetting to poll**: If you don't check the token, cancellation won't work
//! - **Assuming cancellation is immediate**: Cooperative cancellation relies on tasks
//!   checking the token voluntarily

use async_trait::async_trait;
use crate::workflow::{
    builder::WorkflowBuilder,
    cancellation::CancellationToken,
    task::{TaskContext, TaskError, TaskId, TaskResult, WorkflowTask},
    tasks::{AgentLoopTask, FunctionTask, GraphQueryTask},
    Workflow,
};

/// Creates a linear workflow that executes tasks sequentially.
///
/// Each task depends on the previous task, creating a straight-line
/// execution path.
///
/// # Example
///
/// ```ignore
/// use forge_agent::workflow::examples::example_linear_workflow;
///
/// let workflow = example_linear_workflow();
/// assert_eq!(workflow.task_count(), 3);
/// ```
#[cfg(doc)]
pub fn example_linear_workflow() -> Result<Workflow, WorkflowError> {
    WorkflowBuilder::sequential(vec![
        Box::new(FunctionTask::new(
            TaskId::new("init"),
            "Initialize".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        )),
        Box::new(FunctionTask::new(
            TaskId::new("process"),
            "Process".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        )),
        Box::new(FunctionTask::new(
            TaskId::new("finalize"),
            "Finalize".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        )),
    ])
}

/// Creates a branching workflow with conditional paths.
///
/// Demonstrates using conditional tasks to create different execution
/// paths based on runtime conditions.
///
/// # Example
///
/// ```ignore
/// use forge_agent::workflow::examples::example_branching_workflow;
///
/// let workflow = example_branching_workflow();
/// assert_eq!(workflow.task_count(), 4);
/// ```
#[cfg(doc)]
pub fn example_branching_workflow() -> Result<Workflow, WorkflowError> {
    // Condition task
    let condition = Box::new(FunctionTask::new(
        TaskId::new("check"),
        "Check Condition".to_string(),
        |_ctx| async { Ok(TaskResult::Success) },
    ));

    // Then branch
    let then_task = Box::new(FunctionTask::new(
        TaskId::new("then_task"),
        "Then Task".to_string(),
        |_ctx| async { Ok(TaskResult::Success) },
    ));

    // Else branch
    let else_task = Box::new(FunctionTask::new(
        TaskId::new("else_task"),
        "Else Task".to_string(),
        |_ctx| async { Ok(TaskResult::Success) },
    ));

    // Final task
    let finalize = Box::new(FunctionTask::new(
        TaskId::new("finalize"),
        "Finalize".to_string(),
        |_ctx| async { Ok(TaskResult::Success) },
    ));

    // Build workflow with conditional execution
    WorkflowBuilder::new()
        .add_task(condition)
        .add_task(then_task)
        .add_task(else_task)
        .add_task(finalize)
        .dependency(TaskId::new("check"), TaskId::new("then_task"))
        .dependency(TaskId::new("check"), TaskId::new("else_task"))
        .dependency(TaskId::new("then_task"), TaskId::new("finalize"))
        .dependency(TaskId::new("else_task"), TaskId::new("finalize"))
        .build()
}

/// Creates a graph-aware workflow that uses the Forge SDK.
///
/// Demonstrates integrating graph queries into workflows for code
/// intelligence operations.
///
/// # Example
///
/// ```ignore
/// use forge_agent::workflow::examples::example_graph_aware_workflow;
///
/// let workflow = example_graph_aware_workflow();
/// assert_eq!(workflow.task_count(), 3);
/// ```
#[cfg(doc)]
pub fn example_graph_aware_workflow() -> Result<Workflow, WorkflowError> {
    WorkflowBuilder::new()
        .add_task(Box::new(GraphQueryTask::find_symbol("process_data")))
        .add_task(Box::new(GraphQueryTask::references("process_data")))
        .add_task(Box::new(FunctionTask::new(
            TaskId::new("analyze"),
            "Analyze Results".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        )))
        .dependency(
            TaskId::new("graph_query_FindSymbol"),
            TaskId::new("graph_query_References"),
        )
        .dependency(TaskId::new("graph_query_References"), TaskId::new("analyze"))
        .build()
}

/// Creates an agent workflow with AI-driven operations.
///
/// Demonstrates integrating the AgentLoop into workflows for
/// autonomous code analysis and modification.
///
/// # Example
///
/// ```ignore
/// use forge_agent::workflow::examples::example_agent_workflow;
///
/// let workflow = example_agent_workflow();
/// assert_eq!(workflow.task_count(), 3);
/// ```
#[cfg(doc)]
pub fn example_agent_workflow() -> Result<Workflow, WorkflowError> {
    let graph_query = Box::new(GraphQueryTask::find_symbol("main"));

    let agent_task = Box::new(AgentLoopTask::new(
        TaskId::new("agent_analysis"),
        "Agent Analysis".to_string(),
        "Analyze the main function and suggest improvements",
    ));

    let report = Box::new(FunctionTask::new(
        TaskId::new("report"),
        "Generate Report".to_string(),
        |_ctx| async { Ok(TaskResult::Success) },
    ));

    WorkflowBuilder::new()
        .add_task(graph_query)
        .add_task(agent_task)
        .add_task(report)
        .dependency(
            TaskId::new("graph_query_FindSymbol"),
            TaskId::new("agent_analysis"),
        )
        .dependency(TaskId::new("agent_analysis"), TaskId::new("report"))
        .build()
}

/// Cancellation-aware task that demonstrates polling pattern.
///
/// This task executes a loop with configurable iterations and delay between
/// each iteration. It polls the cancellation token at each iteration and
/// exits gracefully when cancelled.
///
/// # Example
///
/// ```ignore
/// use forge_agent::workflow::examples::CancellationAwareTask;
/// use forge_agent::workflow::TaskId;
///
/// let task = CancellationAwareTask::new(
///     TaskId::new("task1"),
///     "Long running task".to_string(),
///     100,  // iterations
///     10,   // delay_ms
/// );
/// ```
pub struct CancellationAwareTask {
    id: TaskId,
    name: String,
    iterations: usize,
    delay_ms: u64,
}

impl CancellationAwareTask {
    /// Creates a new cancellation-aware task.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique task identifier
    /// * `name` - Human-readable task name
    /// * `iterations` - Total number of iterations to complete
    /// * `delay_ms` - Delay between iterations in milliseconds
    pub fn new(id: TaskId, name: String, iterations: usize, delay_ms: u64) -> Self {
        Self {
            id,
            name,
            iterations,
            delay_ms,
        }
    }
}

#[async_trait]
impl WorkflowTask for CancellationAwareTask {
    async fn execute(&self, context: &TaskContext) -> Result<TaskResult, TaskError> {
        let mut completed_iterations = 0;

        // Poll cancellation token in each iteration
        while completed_iterations < self.iterations {
            // Check for cancellation
            if let Some(token) = context.cancellation_token() {
                if token.poll_cancelled() {
                    return Ok(TaskResult::Success); // Return success with partial work
                }
            }

            // Simulate work
            tokio::time::sleep(tokio::time::Duration::from_millis(self.delay_ms)).await;
            completed_iterations += 1;
        }

        Ok(TaskResult::Success)
    }

    fn id(&self) -> TaskId {
        self.id.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Polling task that demonstrates tokio::select! pattern.
///
/// This task uses `tokio::select!` to race between work completion and
/// cancellation, showing proper async cancellation handling.
///
/// # Example
///
/// ```ignore
/// use forge_agent::workflow::examples::PollingTask;
/// use forge_agent::workflow::TaskId;
///
/// let task = PollingTask::new(
///     TaskId::new("task1"),
///     "Polling task".to_string(),
///     5000, // total_duration_ms
/// );
/// ```
pub struct PollingTask {
    id: TaskId,
    name: String,
    total_duration_ms: u64,
}

impl PollingTask {
    /// Creates a new polling task.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique task identifier
    /// * `name` - Human-readable task name
    /// * `total_duration_ms` - Total duration of work in milliseconds
    pub fn new(id: TaskId, name: String, total_duration_ms: u64) -> Self {
        Self {
            id,
            name,
            total_duration_ms,
        }
    }
}

#[async_trait]
impl WorkflowTask for PollingTask {
    async fn execute(&self, context: &TaskContext) -> Result<TaskResult, TaskError> {
        // Use tokio::select! to race between work and cancellation
        let work_duration = tokio::time::Duration::from_millis(self.total_duration_ms);
        let work = tokio::time::sleep(work_duration);

        tokio::select! {
            _ = work => {
                // Work completed
                Ok(TaskResult::Success)
            }
            _ = async {
                // Wait for cancellation
                if let Some(token) = context.cancellation_token() {
                    token.wait_until_cancelled().await;
                }
            } => {
                // Cancelled - clean up resources
                Ok(TaskResult::Success)
            }
        }
    }

    fn id(&self) -> TaskId {
        self.id.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Creates a cooperative cancellation example workflow.
///
/// This function creates a workflow with 3 cancellation-aware tasks and
/// demonstrates spawning a cancellation request after a delay.
///
/// # Returns
///
/// A workflow ready for execution with cancellation support.
///
/// # Example
///
/// ```ignore
/// use forge_agent::workflow::examples::cooperative_cancellation_example;
/// use forge_agent::workflow::executor::WorkflowExecutor;
///
/// let workflow = cooperative_cancellation_example();
/// let mut executor = WorkflowExecutor::new(workflow);
/// // ... execute with cancellation source
/// ```
pub fn cooperative_cancellation_example() -> Workflow {
    let task1 = Box::new(CancellationAwareTask::new(
        TaskId::new("task1"),
        "Cancellation Aware Task 1".to_string(),
        100,
        10,
    ));

    let task2 = Box::new(CancellationAwareTask::new(
        TaskId::new("task2"),
        "Cancellation Aware Task 2".to_string(),
        100,
        10,
    ));

    let task3 = Box::new(CancellationAwareTask::new(
        TaskId::new("task3"),
        "Cancellation Aware Task 3".to_string(),
        100,
        10,
    ));

    WorkflowBuilder::sequential(vec![task1, task2, task3]).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_workflow_example() {
        // Verify the example builds correctly (even if not cfg(doc))
        let result = WorkflowBuilder::sequential(vec![
            Box::new(FunctionTask::new(
                TaskId::new("init"),
                "Initialize".to_string(),
                |_ctx| async { Ok(TaskResult::Success) },
            )),
            Box::new(FunctionTask::new(
                TaskId::new("process"),
                "Process".to_string(),
                |_ctx| async { Ok(TaskResult::Success) },
            )),
            Box::new(FunctionTask::new(
                TaskId::new("finalize"),
                "Finalize".to_string(),
                |_ctx| async { Ok(TaskResult::Success) },
            )),
        ]);

        assert!(result.is_ok());
        let workflow = result.unwrap();
        assert_eq!(workflow.task_count(), 3);
    }

    #[test]
    fn test_branching_workflow_example() {
        let condition = Box::new(FunctionTask::new(
            TaskId::new("check"),
            "Check Condition".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        ));

        let then_task = Box::new(FunctionTask::new(
            TaskId::new("then_task"),
            "Then Task".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        ));

        let else_task = Box::new(FunctionTask::new(
            TaskId::new("else_task"),
            "Else Task".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        ));

        let finalize = Box::new(FunctionTask::new(
            TaskId::new("finalize"),
            "Finalize".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        ));

        let result = WorkflowBuilder::new()
            .add_task(condition)
            .add_task(then_task)
            .add_task(else_task)
            .add_task(finalize)
            .dependency(TaskId::new("check"), TaskId::new("then_task"))
            .dependency(TaskId::new("check"), TaskId::new("else_task"))
            .dependency(TaskId::new("then_task"), TaskId::new("finalize"))
            .dependency(TaskId::new("else_task"), TaskId::new("finalize"))
            .build();

        assert!(result.is_ok());
        let workflow = result.unwrap();
        assert_eq!(workflow.task_count(), 4);
    }

    #[test]
    fn test_graph_aware_workflow_example() {
        let result = WorkflowBuilder::new()
            .add_task(Box::new(GraphQueryTask::find_symbol("process_data")))
            .add_task(Box::new(GraphQueryTask::references("process_data")))
            .add_task(Box::new(FunctionTask::new(
                TaskId::new("analyze"),
                "Analyze Results".to_string(),
                |_ctx| async { Ok(TaskResult::Success) },
            )))
            .dependency(
                TaskId::new("graph_query_FindSymbol"),
                TaskId::new("graph_query_References"),
            )
            .dependency(TaskId::new("graph_query_References"), TaskId::new("analyze"))
            .build();

        assert!(result.is_ok());
        let workflow = result.unwrap();
        assert_eq!(workflow.task_count(), 3);
    }

    #[test]
    fn test_agent_workflow_example() {
        let graph_query = Box::new(GraphQueryTask::find_symbol("main"));

        let agent_task = Box::new(AgentLoopTask::new(
            TaskId::new("agent_analysis"),
            "Agent Analysis".to_string(),
            "Analyze the main function and suggest improvements",
        ));

        let report = Box::new(FunctionTask::new(
            TaskId::new("report"),
            "Generate Report".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        ));

        let result = WorkflowBuilder::new()
            .add_task(graph_query)
            .add_task(agent_task)
            .add_task(report)
            .dependency(
                TaskId::new("graph_query_FindSymbol"),
                TaskId::new("agent_analysis"),
            )
            .dependency(TaskId::new("agent_analysis"), TaskId::new("report"))
            .build();

        assert!(result.is_ok());
        let workflow = result.unwrap();
        assert_eq!(workflow.task_count(), 3);
    }

    // Tests for cooperative cancellation examples

    #[tokio::test]
    async fn test_cancellation_aware_task_stops_on_cancel() {
        use crate::workflow::cancellation::CancellationTokenSource;
        use crate::workflow::task::TaskContext;

        let source = CancellationTokenSource::new();
        let task = CancellationAwareTask::new(
            TaskId::new("task1"),
            "Test Task".to_string(),
            1000, // Would take 10 seconds without cancellation
            10,
        );

        // Create context with cancellation token
        let mut context = TaskContext::new("test-workflow", task.id());
        context = context.with_cancellation_token(source.token());

        // Spawn cancellation after 50ms
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            source.cancel();
        });

        // Execute task - should complete quickly due to cancellation
        let start = std::time::Instant::now();
        let result = task.execute(&context).await;
        let elapsed = start.elapsed();

        assert!(result.is_ok());
        assert!(elapsed < tokio::time::Duration::from_millis(500)); // Should exit early
    }

    #[tokio::test]
    async fn test_cancellation_aware_task_completes_without_cancel() {
        use crate::workflow::task::TaskContext;

        let task = CancellationAwareTask::new(
            TaskId::new("task1"),
            "Test Task".to_string(),
            5, // Small number of iterations
            10,
        );

        // Create context without cancellation token
        let context = TaskContext::new("test-workflow", task.id());

        // Execute task - should complete all iterations
        let result = task.execute(&context).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), TaskResult::Success);
    }

    #[tokio::test]
    async fn test_polling_task_with_tokio_select() {
        use crate::workflow::cancellation::CancellationTokenSource;
        use crate::workflow::task::TaskContext;

        let source = CancellationTokenSource::new();
        let task = PollingTask::new(
            TaskId::new("task1"),
            "Polling Task".to_string(),
            5000, // Would take 5 seconds without cancellation
        );

        // Create context with cancellation token
        let mut context = TaskContext::new("test-workflow", task.id());
        context = context.with_cancellation_token(source.token());

        // Spawn cancellation after 100ms
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            source.cancel();
        });

        // Execute task - should complete quickly due to cancellation
        let start = std::time::Instant::now();
        let result = task.execute(&context).await;
        let elapsed = start.elapsed();

        assert!(result.is_ok());
        assert!(elapsed < tokio::time::Duration::from_millis(500)); // Should exit early
    }

    #[tokio::test]
    async fn test_cooperative_cancellation_example() {
        let workflow = cooperative_cancellation_example();

        assert_eq!(workflow.task_count(), 3);
    }
}

//! Workflow orchestration system for multi-step agent operations.
//!
//! The workflow module provides a DAG-based task scheduling system that:
//! - Executes tasks in topological order based on dependencies
//! - Validates workflows for cycles and missing dependencies before execution
//! - Records all task events to the audit log
//! - Supports sequential execution with failure handling
//! - Provides cooperative cancellation for long-running workflows
//! - Supports timeout configuration for tasks and workflows
//!
//! # Architecture
//!
//! The workflow system is built around three core components:
//! - [`DAG`](crate::workflow::dag::Workflow): Directed acyclic graph for task representation
//! - [`WorkflowTask`](crate::workflow::task::WorkflowTask): Async trait for task execution
//! - [`WorkflowExecutor`](crate::workflow::executor::WorkflowExecutor): Sequential task executor
//!
//! # Cancellation and Timeouts
//!
//! ## Cancellation
//!
//! Workflows support cooperative cancellation via [`CancellationToken`]:
//!
//! ```ignore
//! use forge_agent::workflow::{CancellationTokenSource, WorkflowExecutor};
//! use forge_agent::workflow::dag::Workflow;
//!
//! let source = CancellationTokenSource::new();
//! let mut executor = WorkflowExecutor::new(workflow)
//!     .with_cancellation_source(source);
//!
//! // Cancel from anywhere
//! source.cancel();
//! ```
//!
//! Tasks can cooperatively respond to cancellation by polling the token:
//!
//! ```ignore
//! use forge_agent::workflow::task::TaskContext;
//!
//! async fn my_task(context: &TaskContext) -> Result<TaskResult, TaskError> {
//!     while !context.cancellation_token().map_or(false, |t| t.poll_cancelled()) {
//!         // Do work
//!     }
//!     Ok(TaskResult::Cancelled)
//! }
//! ```
//!
//! See [`examples`](crate::workflow::examples) for complete cancellation-aware task examples.
//!
//! ## Timeouts
//!
//! Both tasks and workflows can have timeout limits:
//!
//! ```ignore
//! use std::time::Duration;
//! use forge_agent::workflow::{WorkflowExecutor, WorkflowTimeout};
//!
//! let mut executor = WorkflowExecutor::new(workflow)
//!     .with_workflow_timeout(WorkflowTimeout::from_secs(300));
//! ```
//!
//! See [`timeout`](crate::workflow::timeout) module for timeout configuration options.
//!
//! # Quick Start
//!
//! ```ignore
//! use forge_agent::{Workflow, WorkflowExecutor, MockTask};
//!
//! let mut workflow = Workflow::new();
//! workflow.add_task(MockTask::new("a", "Task A"));
//! workflow.add_task(MockTask::new("b", "Task B").depends_on("a"));
//!
//! let mut executor = WorkflowExecutor::new(workflow);
//! let result = executor.execute().await?;
//! ```
//!
//! # Workflow Validation
//!
//! Workflows are validated before execution to detect:
//! - Cycles in the dependency graph
//! - Missing dependencies (references to non-existent tasks)
//! - Orphan tasks (disconnected from the main graph)
//!
//! # Execution Model
//!
//! The executor processes tasks sequentially in topological order:
//! 1. Validate workflow structure
//! 2. Calculate execution order via topological sort
//! 3. Execute each task with audit logging
//! 4. Stop on first failure (rollback is deferred to phase 08-05)

pub mod auto_detect;
pub mod builder;
pub mod cancellation;
pub mod checkpoint;
pub mod combinators;
pub mod dag;
pub mod deadlock;
pub mod examples;
pub mod executor;
pub mod rollback;
pub mod state;
pub mod task;
pub mod tasks;
pub mod timeout;
pub mod tools;
pub mod validate;
pub mod yaml;

// Re-export core types for public API
pub use auto_detect::{
    AutoDetectConfig, DependencyAnalyzer, DependencyReason, DependencySuggestion, SuggestedTaskType,
    TaskSuggestion,
};
pub use builder::WorkflowBuilder;
pub use cancellation::{CancellationToken, CancellationTokenSource, ChildToken};
pub use checkpoint::{
    can_proceed, extract_confidence, requires_rollback, validate_checkpoint, CheckpointId,
    CheckpointSummary, RollbackRecommendation, ValidationCheckpoint, ValidationResult, ValidationStatus,
    WorkflowCheckpoint, WorkflowCheckpointService,
};
pub use combinators::{ConditionalTask, ParallelTasks, TryCatchTask};
pub use dag::{Workflow, WorkflowError};
pub use deadlock::{DeadlockDetector, DeadlockError, DeadlockWarning, DeadlockWarningType};
pub use executor::{WorkflowExecutor, WorkflowResult};
pub use examples::{
    CancellationAwareTask, PollingTask, TimeoutAndCancellationTask,
    cooperative_cancellation_example, timeout_cancellation_example,
};
pub use rollback::{
    CompensationReport, ExecutableCompensation, RollbackEngine, RollbackError, RollbackReport,
    RollbackStrategy,
};
pub use state::{TaskStatus, TaskSummary, WorkflowState, WorkflowStatus};
pub use task::{CompensationAction, CompensationType, Dependency, TaskContext, TaskError, TaskId, TaskResult, WorkflowTask};
pub use tasks::{AgentLoopTask, FileEditTask, FunctionTask, GraphQueryTask, GraphQueryType, ShellCommandTask};
pub use timeout::{TaskTimeout, TimeoutConfig, TimeoutError, WorkflowTimeout};
pub use tools::{ProcessGuard, Tool, ToolError, ToolInvocation, ToolInvocationResult, ToolRegistry, ToolResult};
pub use validate::{ValidationReport, WorkflowValidator};
pub use yaml::{YamlWorkflow, YamlTask, YamlTaskParams, YamlTaskType, YamlWorkflowError};

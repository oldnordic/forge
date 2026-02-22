//! Workflow orchestration system for multi-step agent operations.
//!
//! The workflow module provides a DAG-based task scheduling system that:
//! - Executes tasks in topological order based on dependencies
//! - Validates workflows for cycles and missing dependencies before execution
//! - Records all task events to the audit log
//! - Supports sequential execution with failure handling
//!
//! # Architecture
//!
//! The workflow system is built around three core components:
//! - [`DAG`](crate::workflow::dag::Workflow): Directed acyclic graph for task representation
//! - [`WorkflowTask`](crate::workflow::task::WorkflowTask): Async trait for task execution
//! - [`WorkflowExecutor`](crate::workflow::executor::WorkflowExecutor): Sequential task executor
//!
//! # Example
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

pub mod builder;
pub mod dag;
pub mod executor;
pub mod task;
pub mod tasks;
pub mod validate;

// Re-export core types for public API
pub use builder::WorkflowBuilder;
pub use dag::{Workflow, WorkflowError};
pub use executor::{WorkflowExecutor, WorkflowResult};
pub use task::{Dependency, TaskContext, TaskError, TaskId, TaskResult, WorkflowTask};
pub use tasks::{AgentLoopTask, FunctionTask, GraphQueryTask, GraphQueryType, ShellCommandTask};
pub use validate::{ValidationReport, WorkflowValidator};

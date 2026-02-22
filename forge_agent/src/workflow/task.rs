//! Task abstraction and execution trait for workflow system.
//!
//! Defines the core task interface that all workflow tasks must implement,
//! along with supporting types for task identification, execution context,
//! and result reporting.

use async_trait::async_trait;
use forge_core::Forge;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for a workflow task.
///
/// TaskId wraps a string identifier and implements the necessary traits
/// for use as a HashMap key and graph node identifier.
#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct TaskId(String);

impl TaskId {
    /// Creates a new TaskId from a string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the underlying string identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the TaskId and returns the underlying string.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for TaskId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for TaskId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Dependency strength between tasks.
///
/// Hard dependencies must complete successfully before the dependent
/// task can execute. Soft dependencies represent preference but not
/// requirements (not yet enforced in v0.1).
#[derive(Clone, Debug, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dependency {
    /// Task must complete successfully (blocking dependency)
    Hard,
    /// Task should complete if possible (non-blocking, planned for v0.2)
    Soft,
}

/// Execution result for a workflow task.
///
/// Captures the outcome of task execution for audit logging and
/// workflow coordination.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskResult {
    /// Task completed successfully
    Success,
    /// Task failed with an error message
    Failed(String),
    /// Task was skipped (e.g., due to failed hard dependency)
    Skipped,
}

/// Execution context provided to workflow tasks.
///
/// Provides access to the Forge SDK for graph operations and
/// metadata about the current workflow execution.
pub struct TaskContext {
    /// Optional Forge instance for graph queries
    pub forge: Option<Forge>,
    /// Workflow identifier for this execution
    pub workflow_id: String,
    /// Task identifier for this execution
    pub task_id: TaskId,
}

impl TaskContext {
    /// Creates a new TaskContext.
    pub fn new(workflow_id: impl Into<String>, task_id: TaskId) -> Self {
        Self {
            forge: None,
            workflow_id: workflow_id.into(),
            task_id,
        }
    }

    /// Sets the Forge instance for graph operations.
    pub fn with_forge(mut self, forge: Forge) -> Self {
        self.forge = Some(forge);
        self
    }
}

/// Error types for task execution.
#[derive(thiserror::Error, Debug)]
pub enum TaskError {
    /// Task execution failed with a message
    #[error("Task execution failed: {0}")]
    ExecutionFailed(String),

    /// Task dependency failed
    #[error("Dependency {dependency} failed: {reason}")]
    DependencyFailed {
        dependency: String,
        reason: String,
    },

    /// Task was skipped due to workflow state
    #[error("Task skipped: {0}")]
    Skipped(String),

    /// I/O error during task execution
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Generic error wrapper
    #[error("Task error: {0}")]
    Other(#[from] anyhow::Error),
}

/// Trait for workflow task execution.
///
/// All workflow tasks must implement this trait to enable execution
/// by the WorkflowExecutor. Tasks are executed asynchronously in
/// topological order based on their dependencies.
#[async_trait]
pub trait WorkflowTask: Send + Sync {
    /// Executes the task with the provided context.
    ///
    /// # Arguments
    ///
    /// * `context` - Execution context with Forge instance and metadata
    ///
    /// # Returns
    ///
    /// Returns `Ok(TaskResult)` on success, or `Err(TaskError)` on failure.
    async fn execute(&self, context: &TaskContext) -> Result<TaskResult, TaskError>;

    /// Returns the unique task identifier.
    fn id(&self) -> TaskId;

    /// Returns the human-readable task name.
    fn name(&self) -> &str;

    /// Returns the list of task dependencies.
    ///
    /// Default implementation returns an empty vector (no dependencies).
    fn dependencies(&self) -> Vec<TaskId> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_id_equality() {
        let id1 = TaskId::new("task-1");
        let id2 = TaskId::new("task-1");
        let id3 = TaskId::new("task-2");

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_task_id_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();

        set.insert(TaskId::new("task-1"));
        set.insert(TaskId::new("task-1"));
        set.insert(TaskId::new("task-2"));

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_task_id_from_string() {
        let id1: TaskId = "task-1".into();
        let id2: TaskId = TaskId::from(String::from("task-1"));

        assert_eq!(id1, id2);
        assert_eq!(id1.as_str(), "task-1");
    }

    #[test]
    fn test_task_id_display() {
        let id = TaskId::new("task-1");
        assert_eq!(format!("{}", id), "task-1");
    }

    #[test]
    fn test_dependency_variants() {
        let hard = Dependency::Hard;
        let soft = Dependency::Soft;

        assert_ne!(hard, soft);
    }

    #[test]
    fn test_task_result_variants() {
        let success = TaskResult::Success;
        let failed = TaskResult::Failed("error".to_string());
        let skipped = TaskResult::Skipped;

        assert_eq!(success, TaskResult::Success);
        assert_eq!(failed, TaskResult::Failed("error".to_string()));
        assert_eq!(skipped, TaskResult::Skipped);
    }

    #[test]
    fn test_task_context_creation() {
        let task_id = TaskId::new("task-1");
        let context = TaskContext::new("workflow-1", task_id.clone());

        assert_eq!(context.workflow_id, "workflow-1");
        assert_eq!(context.task_id, task_id);
        assert!(context.forge.is_none());
    }

    // Mock task for testing WorkflowTask trait
    struct MockTask {
        id: TaskId,
        name: String,
    }

    #[async_trait]
    impl WorkflowTask for MockTask {
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
            Vec::new()
        }
    }

    #[tokio::test]
    async fn test_task_trait() {
        let task = MockTask {
            id: TaskId::new("task-1"),
            name: "Test Task".to_string(),
        };

        assert_eq!(task.id(), TaskId::new("task-1"));
        assert_eq!(task.name(), "Test Task");
        assert!(task.dependencies().is_empty());

        let context = TaskContext::new("workflow-1", task.id());
        let result = task.execute(&context).await.unwrap();
        assert_eq!(result, TaskResult::Success);
    }

    #[tokio::test]
    async fn test_task_with_dependencies() {
        struct TaskWithDeps {
            id: TaskId,
            name: String,
            deps: Vec<TaskId>,
        }

        #[async_trait]
        impl WorkflowTask for TaskWithDeps {
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

        let task = TaskWithDeps {
            id: TaskId::new("task-b"),
            name: "Task B".to_string(),
            deps: vec![TaskId::new("task-a")],
        };

        assert_eq!(task.dependencies().len(), 1);
        assert_eq!(task.dependencies()[0], TaskId::new("task-a"));
    }
}

//! Task abstraction and execution trait for workflow system.
//!
//! Defines the core task interface that all workflow tasks must implement,
//! along with supporting types for task identification, execution context,
//! and result reporting.

use async_trait::async_trait;
use forge_core::Forge;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;

// Import ToolRegistry for task-level tool access
use crate::workflow::tools::ToolRegistry;

// Import AuditLog for audit event recording
use crate::audit::AuditLog;

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
    /// Task result with compensation action (for Saga rollback)
    WithCompensation {
        /// The actual result of task execution
        result: Box<TaskResult>,
        /// Compensation action to undo task side effects
        compensation: CompensationAction,
    },
}

/// Execution context provided to workflow tasks.
///
/// Provides access to the Forge SDK for graph operations,
/// metadata about the current workflow execution, and cancellation token.
pub struct TaskContext {
    /// Optional Forge instance for graph queries
    pub forge: Option<Forge>,
    /// Workflow identifier for this execution
    pub workflow_id: String,
    /// Task identifier for this execution
    pub task_id: TaskId,
    /// Optional cancellation token for cooperative cancellation
    cancellation_token: Option<crate::workflow::cancellation::CancellationToken>,
    /// Optional task timeout duration
    pub task_timeout: Option<std::time::Duration>,
    /// Optional tool registry for tool invocation
    pub tool_registry: Option<Arc<ToolRegistry>>,
    /// Optional audit log for recording events (cloned from executor)
    pub audit_log: Option<AuditLog>,
}

impl TaskContext {
    /// Creates a new TaskContext.
    pub fn new(workflow_id: impl Into<String>, task_id: TaskId) -> Self {
        Self {
            forge: None,
            workflow_id: workflow_id.into(),
            task_id,
            cancellation_token: None,
            task_timeout: None,
            tool_registry: None,
            audit_log: None,
        }
    }

    /// Sets the Forge instance for graph operations.
    pub fn with_forge(mut self, forge: Forge) -> Self {
        self.forge = Some(forge);
        self
    }

    /// Sets the cancellation token for cooperative cancellation.
    ///
    /// # Arguments
    ///
    /// * `token` - The cancellation token to check during task execution
    ///
    /// # Returns
    ///
    /// The context with cancellation token set (for builder pattern)
    ///
    /// # Example
    ///
    /// ```ignore
    /// use forge_agent::workflow::{CancellationTokenSource, TaskContext};
    ///
    /// let source = CancellationTokenSource::new();
    /// let context = TaskContext::new("workflow-1", task_id)
    ///     .with_cancellation_token(source.token());
    /// ```
    pub fn with_cancellation_token(
        mut self,
        token: crate::workflow::cancellation::CancellationToken,
    ) -> Self {
        self.cancellation_token = Some(token);
        self
    }

    /// Returns a reference to the cancellation token if set.
    ///
    /// Tasks can use this to check for cancellation during execution.
    ///
    /// # Example
    ///
    /// ```ignore
    /// async fn execute(&self, context: &TaskContext) -> Result<TaskResult, TaskError> {
    ///     if let Some(token) = context.cancellation_token() {
    ///         if token.is_cancelled() {
    ///             return Ok(TaskResult::Skipped);
    ///         }
    ///     }
    ///     // ... do work
    /// }
    /// ```
    pub fn cancellation_token(&self) -> Option<&crate::workflow::cancellation::CancellationToken> {
        self.cancellation_token.as_ref()
    }

    /// Sets the task timeout for this context.
    ///
    /// # Arguments
    ///
    /// * `timeout` - The timeout duration for the task
    ///
    /// # Returns
    ///
    /// The context with task timeout set (for builder pattern)
    ///
    /// # Example
    ///
    /// ```ignore
    /// use std::time::Duration;
    ///
    /// let context = TaskContext::new("workflow-1", task_id)
    ///     .with_task_timeout(Duration::from_secs(30));
    /// ```
    pub fn with_task_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.task_timeout = Some(timeout);
        self
    }

    /// Sets the tool registry for tool invocation.
    ///
    /// # Arguments
    ///
    /// * `registry` - The tool registry to use for tool invocation
    ///
    /// # Returns
    ///
    /// The context with tool registry set (for builder pattern)
    ///
    /// # Example
    ///
    /// ```ignore
    /// use forge_agent::workflow::tools::ToolRegistry;
    /// use std::sync::Arc;
    ///
    /// let registry = Arc::new(ToolRegistry::new());
    /// let context = TaskContext::new("workflow-1", task_id)
    ///     .with_tool_registry(registry);
    /// ```
    pub fn with_tool_registry(mut self, registry: Arc<ToolRegistry>) -> Self {
        self.tool_registry = Some(registry);
        self
    }

    /// Returns a reference to the tool registry if set.
    ///
    /// # Returns
    ///
    /// - `Some(&Arc<ToolRegistry>)` if tool registry is set
    /// - `None` if no tool registry
    ///
    /// # Example
    ///
    /// ```ignore
    /// if let Some(registry) = context.tool_registry() {
    ///     // Use tool registry
    /// }
    /// ```
    pub fn tool_registry(&self) -> Option<&Arc<ToolRegistry>> {
        self.tool_registry.as_ref()
    }

    /// Sets the audit log for event recording.
    ///
    /// # Arguments
    ///
    /// * `audit_log` - The audit log to use for event recording
    ///
    /// # Returns
    ///
    /// The context with audit log set (for builder pattern)
    ///
    /// # Example
    ///
    /// ```ignore
    /// use forge_agent::audit::AuditLog;
    ///
    /// let audit_log = AuditLog::new();
    /// let context = TaskContext::new("workflow-1", task_id)
    ///     .with_audit_log(audit_log);
    /// ```
    pub fn with_audit_log(mut self, audit_log: AuditLog) -> Self {
        self.audit_log = Some(audit_log);
        self
    }

    /// Returns a mutable reference to the audit log if set.
    ///
    /// # Returns
    ///
    /// - `Some(&mut AuditLog)` if audit log is set
    /// - `None` if no audit log
    ///
    /// # Example
    ///
    /// ```ignore
    /// if let Some(audit_log) = context.audit_log_mut() {
    ///     // Use audit log
    /// }
    /// ```
    pub fn audit_log_mut(&mut self) -> Option<&mut AuditLog> {
        self.audit_log.as_mut()
    }

    /// Returns the task timeout duration if set.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if let Some(timeout) = context.task_timeout() {
    ///     println!("Task timeout: {:?}", timeout);
    /// }
    /// ```
    pub fn task_timeout(&self) -> Option<std::time::Duration> {
        self.task_timeout
    }
}

/// Compensation action that undoes task side effects.
///
/// Describes how to compensate a task during workflow rollback using the
/// Saga pattern. This is a simplified version for use in TaskResult.
/// The full implementation with undo functions is in the rollback module.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompensationAction {
    /// Type of compensation action
    pub action_type: CompensationType,
    /// Human-readable description of the compensation
    pub description: String,
}

impl CompensationAction {
    /// Creates a new CompensationAction.
    pub fn new(action_type: CompensationType, description: impl Into<String>) -> Self {
        Self {
            action_type,
            description: description.into(),
        }
    }

    /// Creates a Skip compensation (no undo needed).
    pub fn skip(description: impl Into<String>) -> Self {
        Self::new(CompensationType::Skip, description)
    }

    /// Creates a Retry compensation (recommends retry instead of undo).
    pub fn retry(description: impl Into<String>) -> Self {
        Self::new(CompensationType::Retry, description)
    }

    /// Creates an UndoFunction compensation.
    pub fn undo(description: impl Into<String>) -> Self {
        Self::new(CompensationType::UndoFunction, description)
    }
}

/// Type of compensation action for task rollback.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompensationType {
    /// Execute undo function to compensate (e.g., delete created file)
    UndoFunction,
    /// No compensation needed (read-only operation)
    Skip,
    /// Recommend retry instead of compensation (transient failure)
    Retry,
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

    /// Task exceeded time limit
    #[error("Task timeout: {0}")]
    Timeout(String),

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

    /// Returns the compensation action for this task (if any).
    ///
    /// Compensation actions are used during workflow rollback to undo
    /// task side effects using the Saga pattern. Tasks that don't have
    /// side effects (e.g., read-only queries) should return None.
    ///
    /// Default implementation returns None (no compensation).
    ///
    /// # Returns
    ///
    /// - `Some(CompensationAction)` - Task can be compensated
    /// - `None` - Task has no compensation (will be skipped during rollback)
    fn compensation(&self) -> Option<CompensationAction> {
        None
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

    #[test]
    fn test_context_without_cancellation_token() {
        use crate::workflow::cancellation::CancellationToken;

        let task_id = TaskId::new("task-1");
        let context = TaskContext::new("workflow-1", task_id);

        // Cancellation token should be None by default
        assert!(context.cancellation_token().is_none());
    }

    #[test]
    fn test_context_with_cancellation_token() {
        use crate::workflow::cancellation::CancellationTokenSource;

        let task_id = TaskId::new("task-1");
        let source = CancellationTokenSource::new();
        let token = source.token();

        let context = TaskContext::new("workflow-1", task_id)
            .with_cancellation_token(token.clone());

        // Cancellation token should be accessible
        assert!(context.cancellation_token().is_some());
        let retrieved_token = context.cancellation_token().unwrap();
        assert!(!retrieved_token.is_cancelled());

        // Cancel source
        source.cancel();

        // Retrieved token should see cancellation
        assert!(retrieved_token.is_cancelled());
    }

    #[test]
    fn test_context_builder_pattern() {
        use crate::workflow::cancellation::CancellationTokenSource;

        let task_id = TaskId::new("task-1");
        let source = CancellationTokenSource::new();

        // Test builder pattern chaining
        let context = TaskContext::new("workflow-1", task_id)
            .with_cancellation_token(source.token());

        assert!(context.cancellation_token().is_some());
        assert_eq!(context.workflow_id, "workflow-1");
    }

    #[test]
    fn test_context_without_task_timeout() {
        let task_id = TaskId::new("task-1");
        let context = TaskContext::new("workflow-1", task_id);

        // Task timeout should be None by default
        assert!(context.task_timeout().is_none());
    }

    #[test]
    fn test_context_with_task_timeout() {
        use std::time::Duration;

        let task_id = TaskId::new("task-1");
        let timeout = Duration::from_secs(30);

        let context = TaskContext::new("workflow-1", task_id)
            .with_task_timeout(timeout);

        // Task timeout should be accessible
        assert!(context.task_timeout().is_some());
        assert_eq!(context.task_timeout().unwrap(), timeout);
    }

    #[test]
    fn test_context_task_timeout_accessor() {
        use std::time::Duration;

        let task_id = TaskId::new("task-1");
        let timeout = Duration::from_millis(5000);

        let context = TaskContext::new("workflow-1", task_id)
            .with_task_timeout(timeout);

        // Verify accessor returns correct value
        assert_eq!(context.task_timeout, Some(timeout));
        assert_eq!(context.task_timeout().unwrap(), Duration::from_millis(5000));
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

    #[tokio::test]
    async fn test_task_compensation_integration() {
        use crate::workflow::rollback::ToolCompensation;

        // Test task with compensation
        struct TaskWithCompensation {
            id: TaskId,
            name: String,
            compensation: Option<CompensationAction>,
        }

        #[async_trait]
        impl WorkflowTask for TaskWithCompensation {
            async fn execute(&self, _context: &TaskContext) -> Result<TaskResult, crate::workflow::TaskError> {
                Ok(TaskResult::Success)
            }

            fn id(&self) -> TaskId {
                self.id.clone()
            }

            fn name(&self) -> &str {
                &self.name
            }

            fn compensation(&self) -> Option<CompensationAction> {
                self.compensation.clone()
            }
        }

        // Create task with skip compensation
        let task = TaskWithCompensation {
            id: TaskId::new("task-1"),
            name: "Test Task".to_string(),
            compensation: Some(CompensationAction::skip("No action needed")),
        };

        // Verify compensation is accessible
        let comp = task.compensation();
        assert!(comp.is_some());
        assert_eq!(comp.unwrap().action_type, CompensationType::Skip);

        // Create task with no compensation
        let task_no_comp = TaskWithCompensation {
            id: TaskId::new("task-2"),
            name: "Task Without Compensation".to_string(),
            compensation: None,
        };

        assert!(task_no_comp.compensation().is_none());
    }

    #[test]
    fn test_compensation_action_to_tool_compensation() {
        use crate::workflow::rollback::ToolCompensation;

        // Test conversion from CompensationAction to ToolCompensation
        let skip_action = CompensationAction::skip("Skip this");
        let tool_comp: ToolCompensation = skip_action.into();
        assert_eq!(tool_comp.description, "Skip this");

        let retry_action = CompensationAction::retry("Retry later");
        let tool_comp: ToolCompensation = retry_action.into();
        assert_eq!(tool_comp.description, "Retry later");

        let undo_action = CompensationAction::undo("Delete file");
        let tool_comp: ToolCompensation = undo_action.into();
        // Undo becomes skip with note about no undo function
        assert!(tool_comp.description.contains("no undo function available"));
    }
}

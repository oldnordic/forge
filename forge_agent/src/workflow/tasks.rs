//! Built-in task implementations for common workflow operations.
//!
//! Provides pre-built task types for graph queries, agent loops, shell commands,
//! and simple function wrapping.

use crate::workflow::task::{CompensationAction, TaskContext, TaskError, TaskId, TaskResult, WorkflowTask};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::future::Future;
use std::time::Duration;

/// Task that wraps an async function for easy workflow definition.
///
/// Useful for simple workflows without custom task types.
///
/// # Example
///
/// ```ignore
/// use forge_agent::workflow::tasks::FunctionTask;
/// use forge_agent::workflow::TaskId;
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
    f: Box<dyn Fn(&TaskContext) -> Pin<Box<dyn Future<Output = Result<TaskResult, TaskError>> + Send>> + Send + Sync>,
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
            f: Box::new(move |ctx| Box::pin(f(ctx)) as Pin<Box<dyn Future<Output = Result<TaskResult, TaskError>> + Send>>),
        }
    }
}

#[async_trait]
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

/// Types of graph queries supported by GraphQueryTask.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum GraphQueryType {
    /// Find a symbol by name
    FindSymbol,
    /// Find references to a symbol
    References,
    /// Analyze impact of changes to a symbol
    ImpactAnalysis,
}

/// Task that executes graph queries using the Forge SDK.
///
/// Queries the code graph for symbols, references, or impact analysis.
pub struct GraphQueryTask {
    id: TaskId,
    name: String,
    query_type: GraphQueryType,
    target: String,
}

impl GraphQueryTask {
    /// Creates a new GraphQueryTask for finding a symbol.
    pub fn find_symbol(target: impl Into<String>) -> Self {
        Self::new(GraphQueryType::FindSymbol, target)
    }

    /// Creates a new GraphQueryTask for finding references.
    pub fn references(target: impl Into<String>) -> Self {
        Self::new(GraphQueryType::References, target)
    }

    /// Creates a new GraphQueryTask for impact analysis.
    pub fn impact_analysis(target: impl Into<String>) -> Self {
        Self::new(GraphQueryType::ImpactAnalysis, target)
    }

    fn new(query_type: GraphQueryType, target: impl Into<String>) -> Self {
        let target_str = target.into();
        Self {
            id: TaskId::new(format!("graph_query_{:?}", query_type)),
            name: format!("Graph Query: {:?}", query_type),
            query_type,
            target: target_str,
        }
    }

    /// Creates a GraphQueryTask with a custom ID.
    pub fn with_id(id: TaskId, query_type: GraphQueryType, target: impl Into<String>) -> Self {
        Self {
            id,
            name: format!("Graph Query: {:?}", query_type),
            query_type,
            target: target.into(),
        }
    }
}

#[async_trait]
impl WorkflowTask for GraphQueryTask {
    async fn execute(&self, _context: &TaskContext) -> Result<TaskResult, TaskError> {
        // Phase 8 stub - all graph queries return success
        // Actual Forge SDK integration will be in Phase 10
        match self.query_type {
            GraphQueryType::FindSymbol => {
                Ok(TaskResult::Success)
            }
            GraphQueryType::References => {
                Ok(TaskResult::Success)
            }
            GraphQueryType::ImpactAnalysis => {
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

    fn compensation(&self) -> Option<CompensationAction> {
        // Graph queries are read-only operations with no side effects
        Some(CompensationAction::skip("Read-only graph query - no undo needed"))
    }
}

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

#[async_trait]
impl WorkflowTask for AgentLoopTask {
    async fn execute(&self, _context: &TaskContext) -> Result<TaskResult, TaskError> {
        // Stub implementation - actual AgentLoop integration in Phase 10
        // For now, just return success to indicate the task structure is valid
        Ok(TaskResult::Success)
    }

    fn id(&self) -> TaskId {
        self.id.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn compensation(&self) -> Option<CompensationAction> {
        // AgentLoop is read-only in v0.4 - no compensation needed
        // Future versions may implement undo for mutations
        Some(CompensationAction::skip("Read-only agent loop - no undo needed in v0.4"))
    }
}

/// Configuration for shell command execution.
///
/// Provides configurable working directory, environment variables,
/// and timeout settings for shell command tasks.
#[derive(Clone, Debug, PartialEq)]
pub struct ShellCommandConfig {
    /// The command to execute
    pub command: String,
    /// Command arguments
    pub args: Vec<String>,
    /// Optional working directory for command execution
    pub working_dir: Option<PathBuf>,
    /// Environment variables to set for the command
    pub env: HashMap<String, String>,
    /// Optional timeout for command execution
    pub timeout: Option<Duration>,
}

impl ShellCommandConfig {
    /// Creates a new ShellCommandConfig with the given command.
    ///
    /// # Arguments
    ///
    /// * `command` - The command to execute (e.g., "echo", "ls", "cargo")
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
            working_dir: None,
            env: HashMap::new(),
            timeout: None,
        }
    }

    /// Sets the command arguments.
    ///
    /// # Arguments
    ///
    /// * `args` - Vector of argument strings
    ///
    /// # Returns
    ///
    /// Self for builder pattern chaining
    pub fn args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    /// Sets the working directory for command execution.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the working directory
    ///
    /// # Returns
    ///
    /// Self for builder pattern chaining
    pub fn working_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(path.into());
        self
    }

    /// Adds an environment variable for the command.
    ///
    /// # Arguments
    ///
    /// * `key` - Environment variable name
    /// * `value` - Environment variable value
    ///
    /// # Returns
    ///
    /// Self for builder pattern chaining
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Sets the timeout for command execution.
    ///
    /// # Arguments
    ///
    /// * `duration` - Timeout duration
    ///
    /// # Returns
    ///
    /// Self for builder pattern chaining
    pub fn timeout(mut self, duration: Duration) -> Self {
        self.timeout = Some(duration);
        self
    }
}

/// Task that executes shell commands using tokio::process.
///
/// Executes external shell commands with configurable working directory,
/// environment variables, and timeout settings. Supports process
/// compensation for rollback operations.
pub struct ShellCommandTask {
    id: TaskId,
    name: String,
    config: ShellCommandConfig,
    /// Last spawned process ID (for compensation)
    last_pid: Arc<std::sync::Mutex<Option<u32>>>,
}

impl ShellCommandTask {
    /// Creates a new ShellCommandTask with the given command.
    ///
    /// # Arguments
    ///
    /// * `id` - Task identifier
    /// * `name` - Human-readable task name
    /// * `command` - Command to execute (e.g., "echo", "ls", "cargo")
    pub fn new(id: TaskId, name: String, command: impl Into<String>) -> Self {
        Self {
            id,
            name,
            config: ShellCommandConfig::new(command),
            last_pid: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Creates a new ShellCommandTask with a ShellCommandConfig.
    ///
    /// # Arguments
    ///
    /// * `id` - Task identifier
    /// * `name` - Human-readable task name
    /// * `config` - Shell command configuration
    pub fn with_config(id: TaskId, name: String, config: ShellCommandConfig) -> Self {
        Self {
            id,
            name,
            config,
            last_pid: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Sets the arguments for the shell command.
    ///
    /// # Deprecated
    ///
    /// Use `with_config()` and `ShellCommandConfig::args()` instead.
    #[deprecated(since = "0.4.0", note = "Use with_config() instead for better configurability")]
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.config.args = args;
        self
    }

    /// Gets the command for this task.
    pub fn command(&self) -> &str {
        &self.config.command
    }

    /// Gets the arguments for this task.
    pub fn args(&self) -> &[String] {
        &self.config.args
    }

    /// Gets the configuration for this task.
    pub fn config(&self) -> &ShellCommandConfig {
        &self.config
    }
}

#[async_trait]
impl WorkflowTask for ShellCommandTask {
    async fn execute(&self, _context: &TaskContext) -> Result<TaskResult, TaskError> {
        // Build the tokio process command
        let mut cmd = tokio::process::Command::new(&self.config.command);

        // Apply arguments
        cmd.args(&self.config.args);

        // Apply working directory if configured
        if let Some(ref working_dir) = self.config.working_dir {
            cmd.current_dir(working_dir);
        }

        // Apply environment variables
        for (key, value) in &self.config.env {
            cmd.env(key, value);
        }

        // Spawn the process
        let child = cmd.spawn().map_err(|e| TaskError::Io(e))?;

        // Store the process ID for compensation
        if let Some(pid) = child.id() {
            let mut last_pid = self.last_pid.lock().unwrap();
            *last_pid = Some(pid);
        }

        // Wait for output with optional timeout
        let output = if let Some(timeout) = self.config.timeout {
            tokio::time::timeout(timeout, child.wait_with_output())
                .await
                .map_err(|_| TaskError::Timeout(format!("Command timed out after {:?}", timeout)))?
                .map_err(TaskError::Io)?
        } else {
            child.wait_with_output().await.map_err(TaskError::Io)?
        };

        // Check exit status
        if output.status.success() {
            Ok(TaskResult::Success)
        } else {
            let exit_code = output.status.code().unwrap_or(-1);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let error_msg = if !stderr.is_empty() {
                format!("exit code: {}, stderr: {}", exit_code, stderr)
            } else {
                format!("exit code: {}", exit_code)
            };
            Ok(TaskResult::Failed(error_msg))
        }
    }

    fn id(&self) -> TaskId {
        self.id.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn compensation(&self) -> Option<CompensationAction> {
        // Check if we spawned a process
        let pid_guard = self.last_pid.lock().unwrap();
        if let Some(pid) = *pid_guard {
            // Return undo compensation for process termination
            Some(CompensationAction::undo(format!(
                "Terminate spawned process: {}",
                pid
            )))
        } else {
            // No process was spawned
            Some(CompensationAction::skip("No process was spawned"))
        }
    }
}

/// Task that edits a file (stub for Phase 11).
///
/// Demonstrates the Saga compensation pattern with undo functionality.
/// In Phase 11, this will be implemented with actual file editing.
pub struct FileEditTask {
    id: TaskId,
    name: String,
    file_path: PathBuf,
    original_content: String,
    new_content: String,
}

impl FileEditTask {
    /// Creates a new FileEditTask.
    ///
    /// # Arguments
    ///
    /// * `id` - Task identifier
    /// * `name` - Human-readable task name
    /// * `file_path` - Path to the file to edit
    /// * `original_content` - Original content (for rollback)
    /// * `new_content` - New content to write
    pub fn new(
        id: TaskId,
        name: String,
        file_path: PathBuf,
        original_content: String,
        new_content: String,
    ) -> Self {
        Self {
            id,
            name,
            file_path,
            original_content,
            new_content,
        }
    }

    /// Gets the file path.
    pub fn file_path(&self) -> &PathBuf {
        &self.file_path
    }

    /// Gets the original content.
    pub fn original_content(&self) -> &str {
        &self.original_content
    }

    /// Gets the new content.
    pub fn new_content(&self) -> &str {
        &self.new_content
    }
}

#[async_trait]
impl WorkflowTask for FileEditTask {
    async fn execute(&self, _context: &TaskContext) -> Result<TaskResult, TaskError> {
        // Phase 8 stub - actual file editing will be implemented in Phase 11
        // For now, return Success to indicate the task structure is valid
        Ok(TaskResult::Success)
    }

    fn id(&self) -> TaskId {
        self.id.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn compensation(&self) -> Option<CompensationAction> {
        // Return undo compensation that restores original content
        // This demonstrates the Saga compensation pattern
        Some(CompensationAction::undo(format!(
            "Restore original content of {}",
            self.file_path.display()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_function_task() {
        let task = FunctionTask::new(
            TaskId::new("test_task"),
            "Test Task".to_string(),
            |_ctx| async { Ok(TaskResult::Success) },
        );

        let context = TaskContext::new("workflow_1", TaskId::new("test_task"));
        let result = task.execute(&context).await.unwrap();

        assert_eq!(result, TaskResult::Success);
        assert_eq!(task.id(), TaskId::new("test_task"));
        assert_eq!(task.name(), "Test Task");
    }

    #[tokio::test]
    async fn test_agent_loop_task() {
        let task = AgentLoopTask::new(
            TaskId::new("agent_task"),
            "Agent Task".to_string(),
            "Find all functions",
        );

        assert_eq!(task.id(), TaskId::new("agent_task"));
        assert_eq!(task.name(), "Agent Task");
        assert_eq!(task.query(), "Find all functions");

        let context = TaskContext::new("workflow_1", TaskId::new("agent_task"));
        let result = task.execute(&context).await.unwrap();
        assert_eq!(result, TaskResult::Success);
    }

    #[tokio::test]
    async fn test_graph_query_task() {
        let task = GraphQueryTask::find_symbol("process_data");

        assert_eq!(task.query_type, GraphQueryType::FindSymbol);
        assert_eq!(task.target, "process_data");

        let context = TaskContext::new("workflow_1", task.id());
        let result = task.execute(&context).await.unwrap();
        assert_eq!(result, TaskResult::Success);
    }

    #[tokio::test]
    async fn test_graph_query_references() {
        let task = GraphQueryTask::references("my_function");

        assert_eq!(task.query_type, GraphQueryType::References);
        assert_eq!(task.target, "my_function");
    }

    #[tokio::test]
    async fn test_graph_query_impact() {
        let task = GraphQueryTask::impact_analysis("struct_name");

        assert_eq!(task.query_type, GraphQueryType::ImpactAnalysis);
        assert_eq!(task.target, "struct_name");
    }

    #[tokio::test]
    async fn test_graph_query_with_custom_id() {
        let task = GraphQueryTask::with_id(
            TaskId::new("custom_id"),
            GraphQueryType::FindSymbol,
            "my_symbol",
        );

        assert_eq!(task.id(), TaskId::new("custom_id"));
        assert_eq!(task.target, "my_symbol");
    }

    #[tokio::test]
    async fn test_shell_command_task_stub() {
        let task = ShellCommandTask::new(
            TaskId::new("shell_task"),
            "Shell Task".to_string(),
            "echo",
        ).with_args(vec!["hello".to_string(), "world".to_string()]);

        assert_eq!(task.id(), TaskId::new("shell_task"));
        assert_eq!(task.command(), "echo");
        assert_eq!(task.args(), &["hello", "world"]);

        let context = TaskContext::new("workflow_1", task.id());
        let result = task.execute(&context).await.unwrap();
        assert_eq!(result, TaskResult::Success);
    }

    #[tokio::test]
    async fn test_shell_task_args_default() {
        let task = ShellCommandTask::new(
            TaskId::new("shell_task"),
            "Shell Task".to_string(),
            "ls",
        );

        assert_eq!(task.args().len(), 0);
        assert!(task.args().is_empty());
    }

    #[tokio::test]
    async fn test_shell_command_with_working_dir() {
        // Create a temporary directory for testing
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_shell_command.txt");

        // Create a test file in the temp directory
        std::fs::write(&test_file, "test content").unwrap();

        // Create a task that lists files in the temp directory
        let config = ShellCommandConfig::new("ls")
            .args(vec![temp_dir.to_string_lossy().to_string()])
            .working_dir(&temp_dir);

        let task = ShellCommandTask::with_config(
            TaskId::new("shell_task"),
            "Shell Task".to_string(),
            config,
        );

        let context = TaskContext::new("workflow_1", task.id());
        let result = task.execute(&context).await.unwrap();

        // Command should succeed
        assert_eq!(result, TaskResult::Success);

        // Clean up
        std::fs::remove_file(&test_file).ok();
    }

    #[tokio::test]
    async fn test_shell_command_with_env() {
        // Create a task that reads an environment variable
        let config = ShellCommandConfig::new("sh")
            .args(vec!["-c".to_string(), "echo $TEST_VAR".to_string()])
            .env("TEST_VAR", "test_value");

        let task = ShellCommandTask::with_config(
            TaskId::new("shell_task"),
            "Shell Task".to_string(),
            config,
        );

        let context = TaskContext::new("workflow_1", task.id());
        let result = task.execute(&context).await.unwrap();

        // Command should succeed
        assert_eq!(result, TaskResult::Success);
    }

    #[tokio::test]
    async fn test_shell_command_compensation() {
        // Create a task that spawns a long-running process
        // For testing, we use echo which exits immediately
        let task = ShellCommandTask::new(
            TaskId::new("shell_task"),
            "Shell Task".to_string(),
            "echo",
        ).with_args(vec!["test".to_string()]);

        // Before execution, compensation should indicate no process spawned
        let compensation = task.compensation();
        assert!(compensation.is_some());
        assert_eq!(compensation.unwrap().action_type, crate::workflow::task::CompensationType::Skip);

        // Execute the task
        let context = TaskContext::new("workflow_1", task.id());
        let result = task.execute(&context).await.unwrap();
        assert_eq!(result, TaskResult::Success);

        // After execution, compensation should indicate process termination
        let compensation = task.compensation();
        assert!(compensation.is_some());
        assert_eq!(compensation.unwrap().action_type, crate::workflow::task::CompensationType::UndoFunction);
    }

    #[tokio::test]
    async fn test_graph_query_compensation_skip() {
        let task = GraphQueryTask::find_symbol("my_function");

        // Graph queries should have Skip compensation
        let compensation = task.compensation();
        assert!(compensation.is_some());
        assert_eq!(compensation.unwrap().action_type, crate::workflow::task::CompensationType::Skip);
    }

    #[tokio::test]
    async fn test_agent_loop_compensation_skip() {
        let task = AgentLoopTask::new(
            TaskId::new("agent_task"),
            "Agent Task".to_string(),
            "Find all functions",
        );

        // AgentLoop should have Skip compensation in v0.4
        let compensation = task.compensation();
        assert!(compensation.is_some());
        assert_eq!(compensation.unwrap().action_type, crate::workflow::task::CompensationType::Skip);
    }

    #[tokio::test]
    async fn test_file_edit_compensation_undo() {
        let task = FileEditTask::new(
            TaskId::new("file_edit"),
            "Edit File".to_string(),
            PathBuf::from("/tmp/test.txt"),
            "original".to_string(),
            "new".to_string(),
        );

        // FileEdit should have UndoFunction compensation
        let compensation = task.compensation();
        assert!(compensation.is_some());
        assert_eq!(compensation.unwrap().action_type, crate::workflow::task::CompensationType::UndoFunction);
    }
}

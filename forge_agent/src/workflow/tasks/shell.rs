use crate::workflow::task::{
    CompensationAction, TaskContext, TaskError, TaskId, TaskResult, WorkflowTask,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

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
    #[deprecated(
        since = "0.4.0",
        note = "Use with_config() instead for better configurability"
    )]
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

#[async_trait::async_trait]
impl WorkflowTask for ShellCommandTask {
    async fn execute(&self, _context: &TaskContext) -> Result<TaskResult, TaskError> {
        let mut cmd = tokio::process::Command::new(&self.config.command);

        cmd.args(&self.config.args);

        if let Some(ref working_dir) = self.config.working_dir {
            cmd.current_dir(working_dir);
        }

        for (key, value) in &self.config.env {
            cmd.env(key, value);
        }

        let child = cmd.spawn().map_err(TaskError::Io)?;

        if let Some(pid) = child.id() {
            let mut last_pid = self.last_pid.lock().unwrap();
            *last_pid = Some(pid);
        }

        let output = if let Some(timeout) = self.config.timeout {
            tokio::time::timeout(timeout, child.wait_with_output())
                .await
                .map_err(|_| TaskError::Timeout(format!("Command timed out after {:?}", timeout)))?
                .map_err(TaskError::Io)?
        } else {
            child.wait_with_output().await.map_err(TaskError::Io)?
        };

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
        let pid_guard = self.last_pid.lock().unwrap();
        if let Some(pid) = *pid_guard {
            Some(CompensationAction::undo(format!(
                "Terminate spawned process: {}",
                pid
            )))
        } else {
            Some(CompensationAction::skip("No process was spawned"))
        }
    }
}

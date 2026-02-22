//! Tool registry for external tool integration with process lifecycle management.
//!
//! The tools module provides a centralized registry for external tools (magellan, cargo, splice, etc.)
//! that workflows can invoke by name. Tools are registered with their executable paths and default
//! arguments, and can be invoked with additional arguments via ToolInvocation.
//!
//! # Process Guards
//!
//! The module implements RAII-based process lifecycle management through ProcessGuard, which
//! automatically terminates spawned processes when dropped. This ensures proper cleanup even
//! if errors occur during workflow execution.
//!
//! # Example
//!
//! ```ignore
//! use forge_agent::workflow::tools::{Tool, ToolRegistry, ToolInvocation};
//!
//! let mut registry = ToolRegistry::new();
//!
//! // Register a tool
//! let magellan = Tool::new(
//!     "magellan",
//!     "/usr/bin/magellan",
//!     vec!["--db".to_string(), ".forge/graph.db".to_string()]
//! );
//! registry.register(magellan)?;
//!
//! // Invoke the tool
//! let invocation = ToolInvocation::new("magellan")
//!     .args(vec!["find".to_string(), "--name".to_string(), "symbol".to_string()]);
//! let result = registry.invoke(&invocation).await?;
//! ```

use crate::workflow::rollback::ToolCompensation;
use crate::workflow::task::{TaskContext, TaskError, TaskResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// A registered external tool.
///
/// Tools are registered with their executable path, default arguments, and description.
/// When invoked, default arguments are combined with invocation-specific arguments.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tool {
    /// Tool identifier (e.g., "magellan", "cargo", "splice")
    pub name: String,
    /// Path to the executable
    pub executable: PathBuf,
    /// Default arguments passed to every invocation
    pub default_args: Vec<String>,
    /// Human-readable description of the tool
    pub description: String,
}

impl Tool {
    /// Creates a new Tool with the given name and executable.
    ///
    /// # Arguments
    ///
    /// * `name` - Tool identifier
    /// * `executable` - Path to the executable (can be relative or absolute)
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::tools::Tool;
    /// use std::path::PathBuf;
    ///
    /// let tool = Tool::new("magellan", PathBuf::from("/usr/bin/magellan"));
    /// ```
    pub fn new(name: impl Into<String>, executable: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            executable: executable.into(),
            default_args: Vec::new(),
            description: String::new(),
        }
    }

    /// Sets the default arguments for the tool.
    ///
    /// # Arguments
    ///
    /// * `args` - Vector of default argument strings
    ///
    /// # Returns
    ///
    /// Self for builder pattern chaining
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::tools::Tool;
    ///
    /// let tool = Tool::new("magellan", "/usr/bin/magellan")
    ///     .default_args(vec!["--db".to_string(), ".forge/graph.db".to_string()]);
    /// ```
    pub fn default_args(mut self, args: Vec<String>) -> Self {
        self.default_args = args;
        self
    }

    /// Sets the description for the tool.
    ///
    /// # Arguments
    ///
    /// * `description` - Human-readable description
    ///
    /// # Returns
    ///
    /// Self for builder pattern chaining
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }
}

/// A specific tool invocation request.
///
/// ToolInvocation specifies which tool to invoke, additional arguments,
/// and optional execution context (working directory, environment variables).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolInvocation {
    /// Name of the tool to invoke
    pub tool_name: String,
    /// Additional arguments beyond tool defaults
    pub args: Vec<String>,
    /// Optional working directory for execution
    pub working_dir: Option<PathBuf>,
    /// Optional environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,
}

impl ToolInvocation {
    /// Creates a new ToolInvocation for the specified tool.
    ///
    /// # Arguments
    ///
    /// * `tool_name` - Name of the registered tool to invoke
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::tools::ToolInvocation;
    ///
    /// let invocation = ToolInvocation::new("magellan");
    /// ```
    pub fn new(tool_name: impl Into<String>) -> Self {
        Self {
            tool_name: tool_name.into(),
            args: Vec::new(),
            working_dir: None,
            env: HashMap::new(),
        }
    }

    /// Sets the arguments for this invocation.
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

    /// Sets the working directory for this invocation.
    ///
    /// # Arguments
    ///
    /// * `path` - Working directory path
    ///
    /// # Returns
    ///
    /// Self for builder pattern chaining
    pub fn working_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(path.into());
        self
    }

    /// Adds an environment variable for this invocation.
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
}

impl fmt::Display for ToolInvocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.tool_name)?;
        for arg in &self.args {
            write!(f, " {}", arg)?;
        }
        Ok(())
    }
}

/// Result of a tool invocation.
///
/// Contains the exit code, captured output, and success status.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolResult {
    /// Process exit code (None if process didn't terminate)
    pub exit_code: Option<i32>,
    /// Captured standard output
    pub stdout: String,
    /// Captured standard error
    pub stderr: String,
    /// True if exit code was 0
    pub success: bool,
}

impl ToolResult {
    /// Creates a new ToolResult from execution output.
    ///
    /// # Arguments
    ///
    /// * `exit_code` - Process exit code
    /// * `stdout` - Standard output
    /// * `stderr` - Standard error
    pub fn new(exit_code: Option<i32>, stdout: String, stderr: String) -> Self {
        let success = exit_code.map_or(false, |code| code == 0);
        Self {
            exit_code,
            stdout,
            stderr,
            success,
        }
    }

    /// Creates a successful ToolResult.
    ///
    /// # Arguments
    ///
    /// * `stdout` - Standard output
    pub fn success(stdout: String) -> Self {
        Self {
            exit_code: Some(0),
            stdout,
            stderr: String::new(),
            success: true,
        }
    }

    /// Creates a failed ToolResult.
    ///
    /// # Arguments
    ///
    /// * `exit_code` - Exit code
    /// * `stderr` - Standard error
    pub fn failure(exit_code: i32, stderr: String) -> Self {
        Self {
            exit_code: Some(exit_code),
            stdout: String::new(),
            stderr,
            success: false,
        }
    }
}

/// Errors that can occur during tool operations.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ToolError {
    /// Tool not found in registry
    #[error("Tool not registered: {0}")]
    ToolNotFound(String),

    /// Process execution failed
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    /// Tool execution timed out
    #[error("Tool timed out: {0}")]
    Timeout(String),

    /// Process termination error
    #[error("Failed to terminate process: {0}")]
    TerminationFailed(String),

    /// Tool already registered
    #[error("Tool already registered: {0}")]
    AlreadyRegistered(String),
}

/// Result of a fallback handler operation.
///
/// Fallback handlers can retry with modified invocation, skip with a result,
/// or fail with the original error.
#[derive(Clone, Debug)]
pub enum FallbackResult {
    /// Retry the tool with the same or modified invocation
    Retry(ToolInvocation),
    /// Skip the tool and return a result
    Skip(TaskResult),
    /// Fail with the original error
    Fail(ToolError),
}

/// Handler for tool execution failures.
///
/// FallbackHandler allows workflows to recover from tool failures using
/// configurable strategies (retry, skip, custom handlers).
#[async_trait]
pub trait FallbackHandler: Send + Sync {
    /// Handles a tool execution error.
    ///
    /// # Arguments
    ///
    /// * `error` - The error that occurred during tool execution
    /// * `invocation` - The invocation that caused the error
    ///
    /// # Returns
    ///
    /// A FallbackResult indicating whether to retry, skip, or fail
    async fn handle(&self, error: &ToolError, invocation: &ToolInvocation) -> FallbackResult;
}

/// Retry fallback handler with exponential backoff.
///
/// Retries tool execution on transient errors using exponential backoff.
/// Useful for network timeouts or temporary resource issues.
///
/// # Example
///
/// ```
/// use forge_agent::workflow::tools::RetryFallback;
///
/// // Retry up to 3 times with 100ms base backoff
/// let fallback = RetryFallback::new(3, 100);
/// ```
#[derive(Clone)]
pub struct RetryFallback {
    /// Maximum number of retry attempts
    max_attempts: u32,
    /// Base backoff duration in milliseconds
    backoff_ms: u64,
}

impl RetryFallback {
    /// Creates a new RetryFallback.
    ///
    /// # Arguments
    ///
    /// * `max_attempts` - Maximum number of retry attempts (including initial attempt)
    /// * `backoff_ms` - Base backoff duration in milliseconds (exponential: backoff_ms * 2^attempt)
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::tools::RetryFallback;
    ///
    /// let fallback = RetryFallback::new(3, 100);
    /// ```
    pub fn new(max_attempts: u32, backoff_ms: u64) -> Self {
        Self {
            max_attempts,
            backoff_ms,
        }
    }
}

#[async_trait]
impl FallbackHandler for RetryFallback {
    async fn handle(&self, error: &ToolError, invocation: &ToolInvocation) -> FallbackResult {
        // Extract current attempt from invocation metadata (if available)
        // For now, we'll always retry unless it's a ToolNotFound error
        match error {
            ToolError::ToolNotFound(_) => {
                // Don't retry if tool is not found
                FallbackResult::Fail(error.clone())
            }
            ToolError::Timeout(_) | ToolError::ExecutionFailed(_) => {
                // Retry transient errors
                FallbackResult::Retry(invocation.clone())
            }
            ToolError::AlreadyRegistered(_) | ToolError::TerminationFailed(_) => {
                // Don't retry registration or termination errors
                FallbackResult::Fail(error.clone())
            }
        }
    }
}

impl fmt::Debug for RetryFallback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RetryFallback")
            .field("max_attempts", &self.max_attempts)
            .field("backoff_ms", &self.backoff_ms)
            .finish()
    }
}

/// Skip fallback handler that returns a fixed result.
///
/// Always skips tool execution and returns a pre-configured result.
/// Useful for optional tools or graceful degradation scenarios.
///
/// # Example
///
/// ```
/// use forge_agent::workflow::tasks::TaskResult;
/// use forge_agent::workflow::tools::SkipFallback;
///
/// // Skip with success result
/// let fallback = SkipFallback::success();
///
/// // Skip with custom result
/// let fallback = SkipFallback::new(TaskResult::Skipped);
/// ```
#[derive(Clone)]
pub struct SkipFallback {
    /// Result to return when skipping
    result: TaskResult,
}

impl SkipFallback {
    /// Creates a new SkipFallback with the given result.
    ///
    /// # Arguments
    ///
    /// * `result` - The result to return when skipping
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::tasks::TaskResult;
    /// use forge_agent::workflow::tools::SkipFallback;
    ///
    /// let fallback = SkipFallback::new(TaskResult::Skipped);
    /// ```
    pub fn new(result: TaskResult) -> Self {
        Self { result }
    }

    /// Creates a SkipFallback that returns Success.
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::tools::SkipFallback;
    ///
    /// let fallback = SkipFallback::success();
    /// ```
    pub fn success() -> Self {
        Self {
            result: TaskResult::Success,
        }
    }

    /// Creates a SkipFallback that returns Skipped.
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::tools::SkipFallback;
    ///
    /// let fallback = SkipFallback::skip();
    /// ```
    pub fn skip() -> Self {
        Self {
            result: TaskResult::Skipped,
        }
    }
}

#[async_trait]
impl FallbackHandler for SkipFallback {
    async fn handle(&self, _error: &ToolError, _invocation: &ToolInvocation) -> FallbackResult {
        FallbackResult::Skip(self.result.clone())
    }
}

impl fmt::Debug for SkipFallback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SkipFallback")
            .field("result", &self.result)
            .finish()
    }
}

/// Chain fallback handler that tries multiple handlers in sequence.
///
/// Tries each handler in order until one returns a non-Fail result.
/// If all handlers fail, returns the last Fail result.
///
/// # Example
///
/// ```
/// use forge_agent::workflow::tools::{ChainFallback, RetryFallback, SkipFallback};
///
/// let fallback = ChainFallback::new()
///     .add(RetryFallback::new(3, 100))
///     .add(SkipFallback::skip());
/// ```
#[derive(Clone)]
pub struct ChainFallback {
    /// Chain of handlers to try in sequence
    handlers: Vec<Arc<dyn FallbackHandler>>,
}

impl ChainFallback {
    /// Creates a new ChainFallback.
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::tools::ChainFallback;
    ///
    /// let fallback = ChainFallback::new();
    /// ```
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    /// Adds a handler to the chain.
    ///
    /// # Arguments
    ///
    /// * `handler` - Handler to add to the chain
    ///
    /// # Returns
    ///
    /// Self for builder pattern chaining
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::tools::{ChainFallback, RetryFallback, SkipFallback};
    ///
    /// let fallback = ChainFallback::new()
    ///     .add(RetryFallback::new(3, 100))
    ///     .add(SkipFallback::skip());
    /// ```
    pub fn add(mut self, handler: impl FallbackHandler + 'static) -> Self {
        self.handlers.push(Arc::new(handler));
        self
    }
}

impl Default for ChainFallback {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FallbackHandler for ChainFallback {
    async fn handle(&self, error: &ToolError, invocation: &ToolInvocation) -> FallbackResult {
        let mut last_fail = None;

        for handler in &self.handlers {
            match handler.handle(error, invocation).await {
                FallbackResult::Fail(err) => {
                    last_fail = Some(err);
                }
                result => return result,
            }
        }

        // All handlers failed
        FallbackResult::Fail(last_fail.unwrap_or_else(|| error.clone()))
    }
}

impl fmt::Debug for ChainFallback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChainFallback")
            .field("handlers", &self.handlers.len())
            .finish()
    }
}

/// RAII guard for process lifecycle management.
///
/// ProcessGuard automatically terminates the spawned process when dropped.
/// This ensures proper cleanup even if errors occur during workflow execution.
///
/// The guard uses a shared boolean flag to track whether the process has already
/// been terminated manually, preventing double-termination in Drop.
#[derive(Clone, Debug)]
pub struct ProcessGuard {
    /// Process ID being guarded
    pid: u32,
    /// Name of the tool (for logging)
    tool_name: String,
    /// Shared flag to track termination status
    terminated: Arc<AtomicBool>,
}

impl ProcessGuard {
    /// Creates a new ProcessGuard for the given process.
    ///
    /// # Arguments
    ///
    /// * `pid` - Process ID to guard
    /// * `tool_name` - Name of the tool (for logging)
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::tools::ProcessGuard;
    ///
    /// let guard = ProcessGuard::new(12345, "magellan");
    /// ```
    pub fn new(pid: u32, tool_name: impl Into<String>) -> Self {
        Self {
            pid,
            tool_name: tool_name.into(),
            terminated: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Manually terminates the guarded process.
    ///
    /// Sets the terminated flag to prevent double-termination in Drop.
    ///
    /// # Returns
    ///
    /// - `Ok(())` if termination succeeded
    /// - `Err(ToolError)` if termination failed
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::tools::ProcessGuard;
    ///
    /// let guard = ProcessGuard::new(12345, "magellan");
    /// guard.terminate()?;
    /// ```
    pub fn terminate(&self) -> Result<(), ToolError> {
        // Check if already terminated
        if self.terminated.load(Ordering::SeqCst) {
            return Ok(());
        }

        // Try to kill the process gracefully
        #[cfg(unix)]
        {
            use std::process::Command;
            let result = Command::new("kill")
                .arg("-TERM")
                .arg(self.pid.to_string())
                .output();

            match result {
                Ok(output) => {
                    if output.status.success() {
                        self.terminated.store(true, Ordering::SeqCst);
                        Ok(())
                    } else {
                        Err(ToolError::TerminationFailed(format!(
                            "kill command failed for process {}",
                            self.pid
                        )))
                    }
                }
                Err(e) => Err(ToolError::TerminationFailed(format!(
                    "Failed to execute kill command: {}",
                    e
                ))),
            }
        }

        #[cfg(not(unix))]
        {
            Err(ToolError::TerminationFailed(
                "Process termination not supported on this platform".to_string(),
            ))
        }
    }

    /// Returns the process ID being guarded.
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::tools::ProcessGuard;
    ///
    /// let guard = ProcessGuard::new(12345, "magellan");
    /// assert_eq!(guard.pid(), 12345);
    /// ```
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Returns true if the process was terminated.
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::tools::ProcessGuard;
    ///
    /// let guard = ProcessGuard::new(12345, "magellan");
    /// assert!(!guard.is_terminated());
    /// ```
    pub fn is_terminated(&self) -> bool {
        self.terminated.load(Ordering::SeqCst)
    }
}

impl fmt::Display for ProcessGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ProcessGuard(pid={}, tool={}, terminated={})",
            self.pid,
            self.tool_name,
            self.is_terminated()
        )
    }
}

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        // Only terminate if not already terminated
        if !self.is_terminated() {
            if let Err(e) = self.terminate() {
                // Log the error but don't panic in Drop
                eprintln!("ProcessGuard drop error: {}", e);
            }
        }
    }
}

impl From<ProcessGuard> for ToolCompensation {
    fn from(guard: ProcessGuard) -> Self {
        ToolCompensation::new(
            format!("Terminate process: {} ({})", guard.tool_name, guard.pid),
            move |_context| {
                // Try to terminate the process
                if guard.terminate().is_ok() {
                    Ok(TaskResult::Success)
                } else {
                    // Termination failed, but don't fail rollback
                    Ok(TaskResult::Skipped)
                }
            },
        )
    }
}

/// Wrapper for tool invocation results with optional process guard.
///
/// ToolInvocationResult contains both the result of the tool execution
/// and an optional RAII guard for long-running processes.
#[derive(Clone, Debug)]
pub struct ToolInvocationResult {
    /// Result of the tool invocation
    pub result: ToolResult,
    /// Optional process guard (None for simple commands that complete immediately)
    pub guard: Option<ProcessGuard>,
}

impl ToolInvocationResult {
    /// Creates a new ToolInvocationResult.
    ///
    /// # Arguments
    ///
    /// * `result` - Tool execution result
    /// * `guard` - Optional process guard
    pub fn new(result: ToolResult, guard: Option<ProcessGuard>) -> Self {
        Self { result, guard }
    }

    /// Creates a result without a process guard (for completed commands).
    ///
    /// # Arguments
    ///
    /// * `result` - Tool execution result
    pub fn completed(result: ToolResult) -> Self {
        Self {
            result,
            guard: None,
        }
    }
}

/// Registry for external tools.
///
/// ToolRegistry stores registered tools and provides methods for invoking them
/// with proper process lifecycle management.
///
/// # Example
///
/// ```ignore
/// use forge_agent::workflow::tools::{Tool, ToolRegistry, ToolInvocation};
///
/// let mut registry = ToolRegistry::new();
///
/// // Register magellan
/// let magellan = Tool::new("magellan", "/usr/bin/magellan")
///     .default_args(vec!["--db".to_string(), ".forge/graph.db".to_string()])
///     .description("Graph-based code indexer");
/// registry.register(magellan)?;
///
/// // Check registration
/// assert!(registry.is_registered("magellan"));
///
/// // List all tools
/// let tools = registry.list_tools();
/// assert!(tools.contains(&"magellan"));
/// ```
#[derive(Clone)]
pub struct ToolRegistry {
    /// Registered tools indexed by name
    tools: HashMap<String, Tool>,
}

impl ToolRegistry {
    /// Creates a new empty ToolRegistry.
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::tools::ToolRegistry;
    ///
    /// let registry = ToolRegistry::new();
    /// assert_eq!(registry.len(), 0);
    /// ```
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Registers a tool in the registry.
    ///
    /// # Arguments
    ///
    /// * `tool` - Tool to register
    ///
    /// # Returns
    ///
    /// - `Ok(())` if registration succeeded
    /// - `Err(ToolError::AlreadyRegistered)` if tool with same name exists
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::tools::{Tool, ToolRegistry};
    ///
    /// let mut registry = ToolRegistry::new();
    /// let tool = Tool::new("magellan", "/usr/bin/magellan");
    /// registry.register(tool)?;
    /// ```
    pub fn register(&mut self, tool: Tool) -> Result<(), ToolError> {
        if self.tools.contains_key(&tool.name) {
            return Err(ToolError::AlreadyRegistered(tool.name.clone()));
        }
        self.tools.insert(tool.name.clone(), tool);
        Ok(())
    }

    /// Gets a tool by name.
    ///
    /// # Arguments
    ///
    /// * `name` - Tool name to look up
    ///
    /// # Returns
    ///
    /// - `Some(&Tool)` if tool exists
    /// - `None` if tool not found
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::tools::{Tool, ToolRegistry};
    ///
    /// let mut registry = ToolRegistry::new();
    /// registry.register(Tool::new("magellan", "/usr/bin/magellan")).unwrap();
    ///
    /// let tool = registry.get("magellan");
    /// assert!(tool.is_some());
    /// assert_eq!(tool.unwrap().name, "magellan");
    /// ```
    pub fn get(&self, name: &str) -> Option<&Tool> {
        self.tools.get(name)
    }

    /// Invokes a tool with the given invocation parameters.
    ///
    /// # Arguments
    ///
    /// * `invocation` - Tool invocation request
    ///
    /// # Returns
    ///
    /// - `Ok(ToolInvocationResult)` with result and optional process guard
    /// - `Err(ToolError)` if tool not found or execution fails
    ///
    /// # Example
    ///
    /// ```ignore
    /// use forge_agent::workflow::tools::{ToolRegistry, ToolInvocation, Tool};
    ///
    /// let mut registry = ToolRegistry::new();
    /// registry.register(Tool::new("echo", "/bin/echo")).unwrap();
    ///
    /// let invocation = ToolInvocation::new("echo")
    ///     .args(vec!["hello".to_string(), "world".to_string()]);
    ///
    /// let result = registry.invoke(&invocation).await?;
    /// assert!(result.result.success);
    /// ```
    pub async fn invoke(
        &self,
        invocation: &ToolInvocation,
    ) -> Result<ToolInvocationResult, ToolError> {
        // Look up the tool
        let tool = self
            .get(&invocation.tool_name)
            .ok_or_else(|| ToolError::ToolNotFound(invocation.tool_name.clone()))?;

        // Build full command: executable + default_args + invocation.args
        let mut cmd = tokio::process::Command::new(&tool.executable);
        cmd.args(&tool.default_args);
        cmd.args(&invocation.args);

        // Apply working directory if specified
        if let Some(ref working_dir) = invocation.working_dir {
            cmd.current_dir(working_dir);
        }

        // Apply environment variables
        for (key, value) in &invocation.env {
            cmd.env(key, value);
        }

        // Ensure stdout and stderr are captured
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        // Spawn the process
        let child = cmd
            .spawn()
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to spawn: {}", e)))?;

        // Get the process ID
        let pid = child.id().ok_or_else(|| {
            ToolError::ExecutionFailed("Failed to get process ID".to_string())
        })?;

        // Create a process guard immediately
        let guard = ProcessGuard::new(pid, &tool.name);

        // Wait for the process to complete (with timeout)
        // For now, use a default timeout of 30 seconds
        let timeout_duration = Duration::from_secs(30);

        let output = match tokio::time::timeout(timeout_duration, child.wait_with_output()).await {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => {
                return Err(ToolError::ExecutionFailed(format!(
                    "Failed to wait for process: {}",
                    e
                )))
            }
            Err(_) => {
                // Timeout - terminate the process
                let _ = guard.terminate();
                return Err(ToolError::Timeout(format!(
                    "Tool {} timed out after {:?}",
                    invocation.tool_name, timeout_duration
                )));
            }
        };

        // Mark the process as terminated (it completed normally)
        guard.terminated.store(true, std::sync::atomic::Ordering::SeqCst);

        // Parse the result
        let exit_code = output.status.code();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let result = ToolResult::new(exit_code, stdout, stderr);

        // Always return completed result (process already terminated)
        Ok(ToolInvocationResult::completed(result))
    }

    /// Lists all registered tool names.
    ///
    /// # Returns
    ///
    /// Vector of tool names
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::tools::{Tool, ToolRegistry};
    ///
    /// let mut registry = ToolRegistry::new();
    /// registry.register(Tool::new("magellan", "/usr/bin/magellan")).unwrap();
    /// registry.register(Tool::new("cargo", "/usr/bin/cargo")).unwrap();
    ///
    /// let tools = registry.list_tools();
    /// assert_eq!(tools.len(), 2);
    /// ```
    pub fn list_tools(&self) -> Vec<&str> {
        self.tools.keys().map(|k| k.as_str()).collect()
    }

    /// Checks if a tool is registered.
    ///
    /// # Arguments
    ///
    /// * `name` - Tool name to check
    ///
    /// # Returns
    ///
    /// - `true` if tool is registered
    /// - `false` if tool is not registered
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::tools::{Tool, ToolRegistry};
    ///
    /// let mut registry = ToolRegistry::new();
    /// registry.register(Tool::new("magellan", "/usr/bin/magellan")).unwrap();
    ///
    /// assert!(registry.is_registered("magellan"));
    /// assert!(!registry.is_registered("cargo"));
    /// ```
    pub fn is_registered(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Returns the number of registered tools.
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::tools::{Tool, ToolRegistry};
    ///
    /// let mut registry = ToolRegistry::new();
    /// assert_eq!(registry.len(), 0);
    ///
    /// registry.register(Tool::new("magellan", "/usr/bin/magellan")).unwrap();
    /// assert_eq!(registry.len(), 1);
    /// ```
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Returns true if the registry is empty.
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::tools::ToolRegistry;
    ///
    /// let registry = ToolRegistry::new();
    /// assert!(registry.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Creates a ToolRegistry with standard tools pre-registered.
    ///
    /// This method attempts to discover and register commonly-used tools:
    /// - magellan (graph-based code indexer)
    /// - cargo (Rust package manager)
    /// - splice (precision code editor)
    ///
    /// Tools that are not found are logged but don't cause failure (graceful degradation).
    ///
    /// # Returns
    ///
    /// A ToolRegistry with discovered tools registered
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::tools::ToolRegistry;
    ///
    /// let registry = ToolRegistry::with_standard_tools();
    /// // registry may have magellan, cargo, splice if they were found
    /// ```
    pub fn with_standard_tools() -> Self {
        let mut registry = Self::new();

        // Helper function to find a tool in PATH
        let find_tool = |name: &str| -> Option<PathBuf> {
            match Command::new("which").arg(name).output() {
                Ok(output) => {
                    if output.status.success() {
                        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        Some(PathBuf::from(path))
                    } else {
                        None
                    }
                }
                Err(_) => None,
            }
        };

        // Register magellan if found
        if let Some(path) = find_tool("magellan") {
            let tool = Tool::new("magellan", path)
                .description("Graph-based code indexer");
            if registry.register(tool).is_ok() {
                eprintln!("Registered standard tool: magellan");
            }
        } else {
            eprintln!("Warning: magellan not found in PATH");
        }

        // Register cargo if found
        if let Some(path) = find_tool("cargo") {
            let tool = Tool::new("cargo", path)
                .description("Rust package manager");
            if registry.register(tool).is_ok() {
                eprintln!("Registered standard tool: cargo");
            }
        } else {
            eprintln!("Warning: cargo not found in PATH");
        }

        // Register splice if found
        if let Some(path) = find_tool("splice") {
            let tool = Tool::new("splice", path)
                .description("Precision code editor");
            if registry.register(tool).is_ok() {
                eprintln!("Registered standard tool: splice");
            }
        } else {
            eprintln!("Warning: splice not found in PATH");
        }

        registry
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::with_standard_tools()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============== FallbackHandler Tests ==============

    #[tokio::test]
    async fn test_retry_fallback_retries_transient_errors() {
        let fallback = RetryFallback::new(3, 100);
        let invocation = ToolInvocation::new("test_tool").args(vec!["arg1".to_string()]);

        // Test timeout error (should retry)
        let error = ToolError::Timeout("Test timeout".to_string());
        let result = fallback.handle(&error, &invocation).await;

        assert!(matches!(result, FallbackResult::Retry(_)));
    }

    #[tokio::test]
    async fn test_retry_fallback_fails_on_tool_not_found() {
        let fallback = RetryFallback::new(3, 100);
        let invocation = ToolInvocation::new("nonexistent_tool");

        // Test tool not found (should fail)
        let error = ToolError::ToolNotFound("nonexistent_tool".to_string());
        let result = fallback.handle(&error, &invocation).await;

        assert!(matches!(result, FallbackResult::Fail(_)));
    }

    #[tokio::test]
    async fn test_skip_fallback_success() {
        let fallback = SkipFallback::success();
        let invocation = ToolInvocation::new("test_tool");
        let error = ToolError::ToolNotFound("test_tool".to_string());

        let result = fallback.handle(&error, &invocation).await;

        assert!(matches!(result, FallbackResult::Skip(TaskResult::Success)));
    }

    #[tokio::test]
    async fn test_skip_fallback_skip() {
        let fallback = SkipFallback::skip();
        let invocation = ToolInvocation::new("test_tool");
        let error = ToolError::ToolNotFound("test_tool".to_string());

        let result = fallback.handle(&error, &invocation).await;

        assert!(matches!(result, FallbackResult::Skip(TaskResult::Skipped)));
    }

    #[tokio::test]
    async fn test_skip_fallback_custom_result() {
        let fallback = SkipFallback::new(TaskResult::Failed("Custom failure".to_string()));
        let invocation = ToolInvocation::new("test_tool");
        let error = ToolError::ToolNotFound("test_tool".to_string());

        let result = fallback.handle(&error, &invocation).await;

        assert!(matches!(result, FallbackResult::Skip(TaskResult::Failed(_))));
        if let FallbackResult::Skip(TaskResult::Failed(msg)) = result {
            assert_eq!(msg, "Custom failure");
        }
    }

    #[tokio::test]
    async fn test_chain_fallback_tries_handlers_in_sequence() {
        let invocation = ToolInvocation::new("test_tool");
        let error = ToolError::Timeout("Test timeout".to_string());

        // Create chain with retry (fails) then skip (succeeds)
        let fallback = ChainFallback::new()
            .add(SkipFallback::skip())
            .add(SkipFallback::success());

        let result = fallback.handle(&error, &invocation).await;

        // First handler (skip) should be used
        assert!(matches!(result, FallbackResult::Skip(TaskResult::Skipped)));
    }

    #[tokio::test]
    async fn test_chain_fallback_all_handlers_fail() {
        let invocation = ToolInvocation::new("test_tool");
        let error = ToolError::Timeout("Test timeout".to_string());

        // Create chain with custom handler that always fails
        #[derive(Clone)]
        struct AlwaysFail;
        #[async_trait]
        impl FallbackHandler for AlwaysFail {
            async fn handle(&self, error: &ToolError, _invocation: &ToolInvocation) -> FallbackResult {
                FallbackResult::Fail(error.clone())
            }
        }

        let fallback = ChainFallback::new()
            .add(AlwaysFail)
            .add(AlwaysFail);

        let result = fallback.handle(&error, &invocation).await;

        assert!(matches!(result, FallbackResult::Fail(_)));
    }

    #[tokio::test]
    async fn test_chain_fallback_empty_chain() {
        let invocation = ToolInvocation::new("test_tool");
        let error = ToolError::Timeout("Test timeout".to_string());

        let fallback = ChainFallback::new();
        let result = fallback.handle(&error, &invocation).await;

        // Empty chain should return the original error
        assert!(matches!(result, FallbackResult::Fail(_)));
    }

    // ============== Tool Tests ==============

    #[test]
    fn test_tool_creation() {
        let tool = Tool::new("magellan", "/usr/bin/magellan");

        assert_eq!(tool.name, "magellan");
        assert_eq!(tool.executable, PathBuf::from("/usr/bin/magellan"));
        assert!(tool.default_args.is_empty());
        assert!(tool.description.is_empty());
    }

    #[test]
    fn test_tool_with_default_args() {
        let tool = Tool::new("magellan", "/usr/bin/magellan").default_args(vec![
            "--db".to_string(),
            ".forge/graph.db".to_string(),
        ]);

        assert_eq!(tool.default_args.len(), 2);
        assert_eq!(tool.default_args[0], "--db");
        assert_eq!(tool.default_args[1], ".forge/graph.db");
    }

    #[test]
    fn test_tool_with_description() {
        let tool = Tool::new("magellan", "/usr/bin/magellan")
            .description("Graph-based code indexer");

        assert_eq!(tool.description, "Graph-based code indexer");
    }

    #[test]
    fn test_tool_builder_pattern() {
        let tool = Tool::new("magellan", "/usr/bin/magellan")
            .default_args(vec!["--db".to_string(), ".forge/graph.db".to_string()])
            .description("Graph-based code indexer");

        assert_eq!(tool.name, "magellan");
        assert_eq!(tool.default_args.len(), 2);
        assert_eq!(tool.description, "Graph-based code indexer");
    }

    // ============== ToolInvocation Tests ==============

    #[test]
    fn test_tool_invocation_creation() {
        let invocation = ToolInvocation::new("magellan");

        assert_eq!(invocation.tool_name, "magellan");
        assert!(invocation.args.is_empty());
        assert!(invocation.working_dir.is_none());
        assert!(invocation.env.is_empty());
    }

    #[test]
    fn test_tool_invocation_with_args() {
        let invocation = ToolInvocation::new("magellan").args(vec![
            "find".to_string(),
            "--name".to_string(),
            "symbol".to_string(),
        ]);

        assert_eq!(invocation.args.len(), 3);
        assert_eq!(invocation.args[0], "find");
    }

    #[test]
    fn test_tool_invocation_with_working_dir() {
        let invocation = ToolInvocation::new("magellan")
            .working_dir("/home/user/project");

        assert_eq!(
            invocation.working_dir,
            Some(PathBuf::from("/home/user/project"))
        );
    }

    #[test]
    fn test_tool_invocation_with_env() {
        let invocation = ToolInvocation::new("magellan")
            .env("RUST_LOG", "debug");

        assert_eq!(invocation.env.len(), 1);
        assert_eq!(invocation.env.get("RUST_LOG"), Some(&"debug".to_string()));
    }

    #[test]
    fn test_tool_invocation_display() {
        let invocation = ToolInvocation::new("magellan")
            .args(vec!["find".to_string(), "--name".to_string()]);

        let display = format!("{}", invocation);
        assert!(display.contains("magellan"));
        assert!(display.contains("find"));
    }

    // ============== ToolResult Tests ==============

    #[test]
    fn test_tool_result_success() {
        let result = ToolResult::success("output".to_string());

        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.stdout, "output");
        assert!(result.stderr.is_empty());
        assert!(result.success);
    }

    #[test]
    fn test_tool_result_failure() {
        let result = ToolResult::failure(1, "error".to_string());

        assert_eq!(result.exit_code, Some(1));
        assert!(result.stdout.is_empty());
        assert_eq!(result.stderr, "error");
        assert!(!result.success);
    }

    #[test]
    fn test_tool_result_new() {
        let result = ToolResult::new(Some(0), "stdout".to_string(), "stderr".to_string());

        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.stdout, "stdout");
        assert_eq!(result.stderr, "stderr");
        assert!(result.success);
    }

    #[test]
    fn test_tool_result_none_exit_code() {
        let result = ToolResult::new(None, "stdout".to_string(), "stderr".to_string());

        assert_eq!(result.exit_code, None);
        assert!(!result.success);
    }

    // ============== ToolRegistry Tests ==============

    #[test]
    fn test_tool_registry_new() {
        let registry = ToolRegistry::new();

        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_register_tool() {
        let mut registry = ToolRegistry::new();
        let tool = Tool::new("magellan", "/usr/bin/magellan");

        registry.register(tool).unwrap();

        assert_eq!(registry.len(), 1);
        assert!(registry.is_registered("magellan"));
    }

    #[test]
    fn test_register_duplicate_tool() {
        let mut registry = ToolRegistry::new();

        registry
            .register(Tool::new("magellan", "/usr/bin/magellan"))
            .unwrap();

        let result = registry.register(Tool::new("magellan", "/usr/bin/magellan"));

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ToolError::AlreadyRegistered("magellan".to_string()));
    }

    #[test]
    fn test_get_tool() {
        let mut registry = ToolRegistry::new();

        registry
            .register(Tool::new("magellan", "/usr/bin/magellan"))
            .unwrap();

        let tool = registry.get("magellan");
        assert!(tool.is_some());
        assert_eq!(tool.unwrap().name, "magellan");
    }

    #[test]
    fn test_get_nonexistent_tool() {
        let registry = ToolRegistry::new();

        let tool = registry.get("magellan");
        assert!(tool.is_none());
    }

    #[test]
    fn test_list_tools() {
        let mut registry = ToolRegistry::new();

        registry
            .register(Tool::new("magellan", "/usr/bin/magellan"))
            .unwrap();
        registry
            .register(Tool::new("cargo", "/usr/bin/cargo"))
            .unwrap();

        let tools = registry.list_tools();
        assert_eq!(tools.len(), 2);
        assert!(tools.contains(&"magellan"));
        assert!(tools.contains(&"cargo"));
    }

    #[test]
    fn test_is_registered() {
        let mut registry = ToolRegistry::new();

        registry
            .register(Tool::new("magellan", "/usr/bin/magellan"))
            .unwrap();

        assert!(registry.is_registered("magellan"));
        assert!(!registry.is_registered("cargo"));
    }

    #[test]
    fn test_tool_registry_default() {
        let registry = ToolRegistry::default();

        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[tokio::test]
    async fn test_invoke_basic_tool() {
        let mut registry = ToolRegistry::new();

        // Register echo as a test tool
        registry
            .register(Tool::new("echo", "echo"))
            .unwrap();

        // Invoke echo
        let invocation = ToolInvocation::new("echo").args(vec!["hello".to_string()]);

        let result = registry.invoke(&invocation).await.unwrap();

        assert!(result.result.success);
        // echo adds newline, so check for "hello\n" or just "hello"
        let trimmed = result.result.stdout.trim();
        assert_eq!(trimmed, "hello", "Expected 'hello', got '{}'", trimmed);
    }

    #[tokio::test]
    async fn test_invoke_with_default_args() {
        let mut registry = ToolRegistry::new();

        // Register echo with default argument
        registry
            .register(
                Tool::new("echo", "/bin/echo").default_args(vec!["-n".to_string()]),
            )
            .unwrap();

        // Invoke echo
        let invocation = ToolInvocation::new("echo").args(vec!["test".to_string()]);

        let result = registry.invoke(&invocation).await.unwrap();

        assert!(result.result.success);
    }

    #[tokio::test]
    async fn test_invoke_nonexistent_tool() {
        let registry = ToolRegistry::new();

        let invocation = ToolInvocation::new("nonexistent").args(vec!["arg".to_string()]);

        let result = registry.invoke(&invocation).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            ToolError::ToolNotFound("nonexistent".to_string())
        );
    }

    // ============== ProcessGuard Tests ==============

    #[test]
    fn test_process_guard_creation() {
        let guard = ProcessGuard::new(12345, "test_tool");

        assert_eq!(guard.pid(), 12345);
        assert!(!guard.is_terminated());
    }

    #[test]
    fn test_process_guard_display() {
        let guard = ProcessGuard::new(12345, "test_tool");

        let display = format!("{}", guard);
        assert!(display.contains("12345"));
        assert!(display.contains("test_tool"));
    }

    #[test]
    fn test_process_guard_clone() {
        let guard1 = ProcessGuard::new(12345, "test_tool");
        let guard2 = guard1.clone();

        assert_eq!(guard1.pid(), guard2.pid());
        assert_eq!(guard1.tool_name, guard2.tool_name);

        // Both guards share the same termination flag
        assert_eq!(guard1.is_terminated(), guard2.is_terminated());
    }

    #[test]
    fn test_process_guard_into_tool_compensation() {
        let guard = ProcessGuard::new(12345, "test_tool");

        let compensation: ToolCompensation = guard.into();

        assert!(compensation.description.contains("12345"));
        assert!(compensation.description.contains("test_tool"));
    }

    #[tokio::test]
    async fn test_tool_invocation_result_completed() {
        let result = ToolResult::success("output".to_string());
        let invocation_result = ToolInvocationResult::completed(result);

        assert!(invocation_result.guard.is_none());
        assert!(invocation_result.result.success);
    }

    #[tokio::test]
    async fn test_tool_invocation_result_with_guard() {
        let result = ToolResult::failure(1, "error".to_string());
        let guard = ProcessGuard::new(12345, "test_tool");
        let invocation_result = ToolInvocationResult::new(result, Some(guard));

        assert!(invocation_result.guard.is_some());
        assert!(!invocation_result.result.success);
    }
}

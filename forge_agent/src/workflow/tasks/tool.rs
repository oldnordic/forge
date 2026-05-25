use crate::workflow::task::{
    CompensationAction, TaskContext, TaskError, TaskId, TaskResult, WorkflowTask,
};
use crate::workflow::tools::{FallbackHandler, FallbackResult, ToolError, ToolInvocation};
use std::path::PathBuf;
use std::sync::Arc;

/// Task that invokes a registered tool from the ToolRegistry.
///
/// ToolTask executes external tools (magellan, cargo, splice, etc.) with
/// configurable fallback handlers for error recovery.
///
/// # Example
///
/// ```ignore
/// use forge_agent::workflow::tasks::ToolTask;
/// use forge_agent::workflow::tools::ToolInvocation;
/// use forge_agent::workflow::TaskId;
///
/// let task = ToolTask::new(
///     TaskId::new("tool_task"),
///     "Magellan Query".to_string(),
///     "magellan"
/// )
/// .args(vec!["find".to_string(), "--name".to_string(), "symbol".to_string()]);
/// ```
pub struct ToolTask {
    /// Task identifier
    id: TaskId,
    /// Human-readable task name
    name: String,
    /// Tool invocation specification
    invocation: ToolInvocation,
    /// Optional fallback handler for error recovery
    pub(super) fallback: Option<Arc<dyn FallbackHandler>>,
}

impl ToolTask {
    /// Creates a new ToolTask.
    ///
    /// # Arguments
    ///
    /// * `id` - Task identifier
    /// * `name` - Human-readable task name
    /// * `tool_name` - Name of the registered tool to invoke
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::tasks::ToolTask;
    /// use forge_agent::workflow::TaskId;
    ///
    /// let task = ToolTask::new(
    ///     TaskId::new("tool_task"),
    ///     "Query Magellan".to_string(),
    ///     "magellan"
    /// );
    /// ```
    pub fn new(id: TaskId, name: String, tool_name: impl Into<String>) -> Self {
        Self {
            id,
            name,
            invocation: ToolInvocation::new(tool_name),
            fallback: None,
        }
    }

    /// Sets the arguments for the tool invocation.
    ///
    /// # Arguments
    ///
    /// * `args` - Vector of argument strings
    ///
    /// # Returns
    ///
    /// Self for builder pattern chaining
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::tasks::ToolTask;
    /// use forge_agent::workflow::TaskId;
    ///
    /// let task = ToolTask::new(
    ///     TaskId::new("tool_task"),
    ///     "Query Magellan".to_string(),
    ///     "magellan"
    /// )
    /// .args(vec!["find".to_string(), "--name".to_string(), "symbol".to_string()]);
    /// ```
    pub fn args(mut self, args: Vec<String>) -> Self {
        self.invocation = self.invocation.args(args);
        self
    }

    /// Sets the working directory for the tool invocation.
    ///
    /// # Arguments
    ///
    /// * `dir` - Working directory path
    ///
    /// # Returns
    ///
    /// Self for builder pattern chaining
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::tasks::ToolTask;
    /// use forge_agent::workflow::TaskId;
    ///
    /// let task = ToolTask::new(
    ///     TaskId::new("tool_task"),
    ///     "Run cargo".to_string(),
    ///     "cargo"
    /// )
    /// .working_dir("/home/user/project");
    /// ```
    pub fn working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.invocation = self.invocation.working_dir(dir);
        self
    }

    /// Adds an environment variable to the tool invocation.
    ///
    /// # Arguments
    ///
    /// * `key` - Environment variable name
    /// * `value` - Environment variable value
    ///
    /// # Returns
    ///
    /// Self for builder pattern chaining
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::tasks::ToolTask;
    /// use forge_agent::workflow::TaskId;
    ///
    /// let task = ToolTask::new(
    ///     TaskId::new("tool_task"),
    ///     "Run cargo".to_string(),
    ///     "cargo"
    /// )
    /// .env("RUST_LOG", "debug");
    /// ```
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.invocation = self.invocation.env(key, value);
        self
    }

    /// Sets the fallback handler for error recovery.
    ///
    /// # Arguments
    ///
    /// * `handler` - Fallback handler to use on tool failure
    ///
    /// # Returns
    ///
    /// Self for builder pattern chaining
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::tasks::ToolTask;
    /// use forge_agent::workflow::tools::RetryFallback;
    /// use forge_agent::workflow::TaskId;
    ///
    /// let task = ToolTask::new(
    ///     TaskId::new("tool_task"),
    ///     "Query Magellan".to_string(),
    ///     "magellan"
    /// )
    /// .with_fallback(Box::new(RetryFallback::new(3, 100)));
    /// ```
    pub fn with_fallback(mut self, handler: Box<dyn FallbackHandler>) -> Self {
        self.fallback = Some(Arc::from(handler));
        self
    }

    /// Gets the tool name for this task.
    pub fn tool_name(&self) -> &str {
        &self.invocation.tool_name
    }

    /// Gets the invocation for this task.
    pub fn invocation(&self) -> &ToolInvocation {
        &self.invocation
    }

    /// Records a fallback activation event to the audit log.
    async fn record_fallback_event(
        context: &TaskContext,
        tool_name: &str,
        error: &ToolError,
        fallback_action: &str,
    ) {
        use crate::audit::AuditEvent;
        use chrono::Utc;

        let _event = AuditEvent::WorkflowToolFallback {
            timestamp: Utc::now(),
            workflow_id: context.workflow_id.clone(),
            task_id: context.task_id.as_str().to_string(),
            tool_name: tool_name.to_string(),
            error: error.to_string(),
            fallback_action: fallback_action.to_string(),
        };

        eprintln!("Fallback event: {} -> {}", tool_name, fallback_action);
    }
}

#[async_trait::async_trait]
impl WorkflowTask for ToolTask {
    async fn execute(&self, context: &TaskContext) -> Result<TaskResult, TaskError> {
        let registry = context.tool_registry.as_ref().ok_or_else(|| {
            TaskError::ExecutionFailed("ToolRegistry not available in TaskContext".to_string())
        })?;

        let invocation_result = registry.invoke(&self.invocation).await;

        match invocation_result {
            Ok(result) => {
                if result.result.success {
                    Ok(TaskResult::Success)
                } else {
                    Ok(TaskResult::Failed(result.result.stderr))
                }
            }
            Err(error) => {
                if let Some(ref fallback) = self.fallback {
                    match fallback.handle(&error, &self.invocation).await {
                        FallbackResult::Retry(retry_invocation) => {
                            Self::record_fallback_event(
                                context,
                                &self.invocation.tool_name,
                                &error,
                                "Retry",
                            )
                            .await;

                            match registry.invoke(&retry_invocation).await {
                                Ok(retry_result) => {
                                    if retry_result.result.success {
                                        Ok(TaskResult::Success)
                                    } else {
                                        Ok(TaskResult::Failed(retry_result.result.stderr))
                                    }
                                }
                                Err(retry_error) => Ok(TaskResult::Failed(format!(
                                    "Tool {} failed after retry: {}",
                                    self.invocation.tool_name, retry_error
                                ))),
                            }
                        }
                        FallbackResult::Skip(result) => {
                            Self::record_fallback_event(
                                context,
                                &self.invocation.tool_name,
                                &error,
                                "Skip",
                            )
                            .await;

                            Ok(result)
                        }
                        FallbackResult::Fail(fail_error) => {
                            Self::record_fallback_event(
                                context,
                                &self.invocation.tool_name,
                                &error,
                                &format!("Fail: {}", fail_error),
                            )
                            .await;

                            Ok(TaskResult::Failed(format!(
                                "Tool {} failed: {}",
                                self.invocation.tool_name, fail_error
                            )))
                        }
                    }
                } else {
                    Ok(TaskResult::Failed(format!(
                        "Tool {} failed: {}",
                        self.invocation.tool_name, error
                    )))
                }
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
        Some(CompensationAction::skip(
            "Tool side effects handled by ProcessGuard",
        ))
    }
}

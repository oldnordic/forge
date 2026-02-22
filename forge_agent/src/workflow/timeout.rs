//! Timeout configuration for tasks and workflows.
//!
//! Provides configurable timeout limits for individual tasks and entire workflows
//! using tokio::time primitives. Timeouts prevent indefinite execution and enable
//! proper error handling with audit logging.

use std::fmt;
use std::time::Duration;

/// Error types for timeout operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TimeoutError {
    /// Task exceeded its time limit
    TaskTimeout { task_id: String, timeout: Duration },
    /// Workflow exceeded its time limit
    WorkflowTimeout { timeout: Duration },
}

impl fmt::Display for TimeoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TimeoutError::TaskTimeout { task_id, timeout } => {
                write!(
                    f,
                    "Task '{}' exceeded timeout limit of {:?}",
                    task_id, timeout
                )
            }
            TimeoutError::WorkflowTimeout { timeout } => {
                write!(f, "Workflow exceeded timeout limit of {:?}", timeout)
            }
        }
    }
}

impl std::error::Error for TimeoutError {}

/// Timeout configuration for individual tasks.
///
/// Wraps a Duration to provide task-level timeout limits with
/// convenience constructors for common values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TaskTimeout(Duration);

impl TaskTimeout {
    /// Creates a new TaskTimeout with the specified duration.
    ///
    /// # Arguments
    ///
    /// * `duration` - The timeout duration
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::timeout::TaskTimeout;
    /// use std::time::Duration;
    ///
    /// let timeout = TaskTimeout::new(Duration::from_secs(30));
    /// ```
    pub fn new(duration: Duration) -> Self {
        Self(duration)
    }

    /// Creates a TaskTimeout from seconds.
    ///
    /// # Arguments
    ///
    /// * `secs` - Timeout in seconds
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::timeout::TaskTimeout;
    ///
    /// let timeout = TaskTimeout::from_secs(30);
    /// ```
    pub fn from_secs(secs: u64) -> Self {
        Self(Duration::from_secs(secs))
    }

    /// Creates a TaskTimeout from milliseconds.
    ///
    /// # Arguments
    ///
    /// * `millis` - Timeout in milliseconds
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::timeout::TaskTimeout;
    ///
    /// let timeout = TaskTimeout::from_millis(5000);
    /// ```
    pub fn from_millis(millis: u64) -> Self {
        Self(Duration::from_millis(millis))
    }

    /// Returns the timeout duration.
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::timeout::TaskTimeout;
    /// use std::time::Duration;
    ///
    /// let timeout = TaskTimeout::from_secs(30);
    /// assert_eq!(timeout.duration(), Duration::from_secs(30));
    /// ```
    pub fn duration(&self) -> Duration {
        self.0
    }
}

impl Default for TaskTimeout {
    /// Default timeout is 30 seconds.
    fn default() -> Self {
        Self(Duration::from_secs(30))
    }
}

/// Timeout configuration for entire workflows.
///
/// Wraps a Duration to provide workflow-level timeout limits with
/// convenience constructors for common values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkflowTimeout(Duration);

impl WorkflowTimeout {
    /// Creates a new WorkflowTimeout with the specified duration.
    ///
    /// # Arguments
    ///
    /// * `duration` - The timeout duration
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::timeout::WorkflowTimeout;
    /// use std::time::Duration;
    ///
    /// let timeout = WorkflowTimeout::new(Duration::from_secs(300));
    /// ```
    pub fn new(duration: Duration) -> Self {
        Self(duration)
    }

    /// Creates a WorkflowTimeout from seconds.
    ///
    /// # Arguments
    ///
    /// * `secs` - Timeout in seconds
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::timeout::WorkflowTimeout;
    ///
    /// let timeout = WorkflowTimeout::from_secs(300);
    /// ```
    pub fn from_secs(secs: u64) -> Self {
        Self(Duration::from_secs(secs))
    }

    /// Creates a WorkflowTimeout from milliseconds.
    ///
    /// # Arguments
    ///
    /// * `millis` - Timeout in milliseconds
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::timeout::WorkflowTimeout;
    ///
    /// let timeout = WorkflowTimeout::from_millis(5000);
    /// ```
    pub fn from_millis(millis: u64) -> Self {
        Self(Duration::from_millis(millis))
    }

    /// Returns the timeout duration.
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::timeout::WorkflowTimeout;
    /// use std::time::Duration;
    ///
    /// let timeout = WorkflowTimeout::from_secs(300);
    /// assert_eq!(timeout.duration(), Duration::from_secs(300));
    /// ```
    pub fn duration(&self) -> Duration {
        self.0
    }
}

impl Default for WorkflowTimeout {
    /// Default timeout is 5 minutes.
    fn default() -> Self {
        Self(Duration::from_secs(300))
    }
}

/// Combined timeout configuration for tasks and workflows.
///
/// Provides configurable timeout limits for both individual tasks
/// and entire workflows. Timeouts are optional - None means no timeout.
///
/// # Example
///
/// ```
/// use forge_agent::workflow::timeout::TimeoutConfig;
///
/// // Use default timeouts (30s task, 5m workflow)
/// let config = TimeoutConfig::new();
///
/// // Disable task timeout
/// let config = TimeoutConfig::no_task_timeout();
///
/// // Disable both timeouts
/// let config = TimeoutConfig::no_timeouts();
/// ```
#[derive(Clone, Debug)]
pub struct TimeoutConfig {
    /// Optional task-level timeout (None means no task timeout)
    pub task_timeout: Option<TaskTimeout>,
    /// Optional workflow-level timeout (None means no workflow timeout)
    pub workflow_timeout: Option<WorkflowTimeout>,
}

impl TimeoutConfig {
    /// Creates a new TimeoutConfig with default timeouts.
    ///
    /// Defaults are 30 seconds for tasks and 5 minutes for workflows.
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::timeout::TimeoutConfig;
    /// use std::time::Duration;
    ///
    /// let config = TimeoutConfig::new();
    /// assert_eq!(config.task_timeout.unwrap().duration(), Duration::from_secs(30));
    /// assert_eq!(config.workflow_timeout.unwrap().duration(), Duration::from_secs(300));
    /// ```
    pub fn new() -> Self {
        Self {
            task_timeout: Some(TaskTimeout::default()),
            workflow_timeout: Some(WorkflowTimeout::default()),
        }
    }

    /// Creates a TimeoutConfig with task timeout disabled.
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::timeout::TimeoutConfig;
    ///
    /// let config = TimeoutConfig::no_task_timeout();
    /// assert!(config.task_timeout.is_none());
    /// assert!(config.workflow_timeout.is_some());
    /// ```
    pub fn no_task_timeout() -> Self {
        Self {
            task_timeout: None,
            workflow_timeout: Some(WorkflowTimeout::default()),
        }
    }

    /// Creates a TimeoutConfig with workflow timeout disabled.
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::timeout::TimeoutConfig;
    ///
    /// let config = TimeoutConfig::no_workflow_timeout();
    /// assert!(config.task_timeout.is_some());
    /// assert!(config.workflow_timeout.is_none());
    /// ```
    pub fn no_workflow_timeout() -> Self {
        Self {
            task_timeout: Some(TaskTimeout::default()),
            workflow_timeout: None,
        }
    }

    /// Creates a TimeoutConfig with both timeouts disabled.
    ///
    /// # Example
    ///
    /// ```
    /// use forge_agent::workflow::timeout::TimeoutConfig;
    ///
    /// let config = TimeoutConfig::no_timeouts();
    /// assert!(config.task_timeout.is_none());
    /// assert!(config.workflow_timeout.is_none());
    /// ```
    pub fn no_timeouts() -> Self {
        Self {
            task_timeout: None,
            workflow_timeout: None,
        }
    }
}

impl Default for TimeoutConfig {
    /// Default configuration has both timeouts enabled.
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeout_error_display() {
        let task_err = TimeoutError::TaskTimeout {
            task_id: "task-1".to_string(),
            timeout: Duration::from_secs(30),
        };
        assert!(task_err.to_string().contains("task-1"));
        assert!(task_err.to_string().contains("30s"));

        let workflow_err = TimeoutError::WorkflowTimeout {
            timeout: Duration::from_secs(300),
        };
        assert!(workflow_err.to_string().contains("Workflow"));
        assert!(workflow_err.to_string().contains("300s"));
    }

    #[test]
    fn test_task_timeout_creation() {
        let timeout = TaskTimeout::new(Duration::from_secs(45));
        assert_eq!(timeout.duration(), Duration::from_secs(45));
    }

    #[test]
    fn test_task_timeout_convenience_constructors() {
        let secs = TaskTimeout::from_secs(60);
        assert_eq!(secs.duration(), Duration::from_secs(60));

        let millis = TaskTimeout::from_millis(5000);
        assert_eq!(millis.duration(), Duration::from_millis(5000));
    }

    #[test]
    fn test_task_timeout_default() {
        let timeout = TaskTimeout::default();
        assert_eq!(timeout.duration(), Duration::from_secs(30));
    }

    #[test]
    fn test_workflow_timeout_creation() {
        let timeout = WorkflowTimeout::new(Duration::from_secs(600));
        assert_eq!(timeout.duration(), Duration::from_secs(600));
    }

    #[test]
    fn test_workflow_timeout_convenience_constructors() {
        let secs = WorkflowTimeout::from_secs(300);
        assert_eq!(secs.duration(), Duration::from_secs(300));

        let millis = WorkflowTimeout::from_millis(10000);
        assert_eq!(millis.duration(), Duration::from_millis(10000));
    }

    #[test]
    fn test_workflow_timeout_default() {
        let timeout = WorkflowTimeout::default();
        assert_eq!(timeout.duration(), Duration::from_secs(300));
    }

    #[test]
    fn test_timeout_config_defaults() {
        let config = TimeoutConfig::new();
        assert!(config.task_timeout.is_some());
        assert!(config.workflow_timeout.is_some());
        assert_eq!(
            config.task_timeout.unwrap().duration(),
            Duration::from_secs(30)
        );
        assert_eq!(
            config.workflow_timeout.unwrap().duration(),
            Duration::from_secs(300)
        );
    }

    #[test]
    fn test_timeout_config_disable_task_timeout() {
        let config = TimeoutConfig::no_task_timeout();
        assert!(config.task_timeout.is_none());
        assert!(config.workflow_timeout.is_some());
    }

    #[test]
    fn test_timeout_config_disable_workflow_timeout() {
        let config = TimeoutConfig::no_workflow_timeout();
        assert!(config.task_timeout.is_some());
        assert!(config.workflow_timeout.is_none());
    }

    #[test]
    fn test_timeout_config_no_timeouts() {
        let config = TimeoutConfig::no_timeouts();
        assert!(config.task_timeout.is_none());
        assert!(config.workflow_timeout.is_none());
    }

    #[test]
    fn test_timeout_config_default_impl() {
        let config = TimeoutConfig::default();
        assert!(config.task_timeout.is_some());
        assert!(config.workflow_timeout.is_some());
    }

    #[test]
    fn test_task_timeout_copy() {
        let timeout1 = TaskTimeout::from_secs(30);
        let timeout2 = timeout1;
        assert_eq!(timeout1, timeout2);
    }

    #[test]
    fn test_workflow_timeout_copy() {
        let timeout1 = WorkflowTimeout::from_secs(300);
        let timeout2 = timeout1;
        assert_eq!(timeout1, timeout2);
    }

    #[test]
    fn test_timeout_error_equality() {
        let err1 = TimeoutError::TaskTimeout {
            task_id: "task-1".to_string(),
            timeout: Duration::from_secs(30),
        };
        let err2 = TimeoutError::TaskTimeout {
            task_id: "task-1".to_string(),
            timeout: Duration::from_secs(30),
        };
        let err3 = TimeoutError::TaskTimeout {
            task_id: "task-2".to_string(),
            timeout: Duration::from_secs(30),
        };

        assert_eq!(err1, err2);
        assert_ne!(err1, err3);
    }
}

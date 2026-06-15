mod fallback;
mod process;
mod registry;

pub use fallback::{ChainFallback, RetryFallback, SkipFallback};
pub use process::{ProcessGuard, ToolInvocationResult};
pub use registry::ToolRegistry;

use crate::workflow::rollback::ToolCompensation;
use crate::workflow::task::TaskResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub executable: PathBuf,
    pub default_args: Vec<String>,
    pub description: String,
}

impl Tool {
    pub fn new(name: impl Into<String>, executable: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            executable: executable.into(),
            default_args: Vec::new(),
            description: String::new(),
        }
    }

    pub fn default_args(mut self, args: Vec<String>) -> Self {
        self.default_args = args;
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolInvocation {
    pub tool_name: String,
    pub args: Vec<String>,
    pub working_dir: Option<PathBuf>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

impl ToolInvocation {
    pub fn new(tool_name: impl Into<String>) -> Self {
        Self {
            tool_name: tool_name.into(),
            args: Vec::new(),
            working_dir: None,
            env: HashMap::new(),
        }
    }

    pub fn args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    pub fn working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }
}

impl fmt::Display for ToolInvocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.tool_name, self.args.join(" "))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolResult {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

impl ToolResult {
    pub fn new(exit_code: Option<i32>, stdout: String, stderr: String) -> Self {
        let success = exit_code == Some(0);
        Self {
            exit_code,
            stdout,
            stderr,
            success,
        }
    }

    pub fn success(stdout: String) -> Self {
        Self {
            exit_code: Some(0),
            stdout,
            stderr: String::new(),
            success: true,
        }
    }

    pub fn failure(exit_code: i32, stderr: String) -> Self {
        Self {
            exit_code: Some(exit_code),
            stdout: String::new(),
            stderr,
            success: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ToolError {
    #[error("Tool not registered: {0}")]
    ToolNotFound(String),

    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Tool timed out: {0}")]
    Timeout(String),

    #[error("Failed to terminate process: {0}")]
    TerminationFailed(String),

    #[error("Tool already registered: {0}")]
    AlreadyRegistered(String),
}

#[derive(Clone, Debug)]
pub enum FallbackResult {
    Retry(ToolInvocation),
    Skip(TaskResult),
    Fail(ToolError),
}

#[async_trait]
pub trait FallbackHandler: Send + Sync {
    async fn handle(&self, error: &ToolError, invocation: &ToolInvocation) -> FallbackResult;
}

#[cfg(test)]
mod tests;

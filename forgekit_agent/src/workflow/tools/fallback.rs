use crate::workflow::task::TaskResult;
use async_trait::async_trait;
use std::fmt;
use std::sync::Arc;

use super::{FallbackHandler, FallbackResult, ToolError, ToolInvocation};

#[derive(Clone)]
pub struct RetryFallback {
    max_attempts: u32,
    backoff_ms: u64,
}

impl RetryFallback {
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
        match error {
            ToolError::ToolNotFound(_) => FallbackResult::Fail(error.clone()),
            ToolError::Timeout(_) | ToolError::ExecutionFailed(_) => {
                FallbackResult::Retry(invocation.clone())
            }
            ToolError::AlreadyRegistered(_) | ToolError::TerminationFailed(_) => {
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

#[derive(Clone)]
pub struct SkipFallback {
    result: TaskResult,
}

impl SkipFallback {
    pub fn new(result: TaskResult) -> Self {
        Self { result }
    }

    pub fn success() -> Self {
        Self {
            result: TaskResult::Success,
        }
    }

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

#[derive(Clone)]
pub struct ChainFallback {
    handlers: Vec<Arc<dyn FallbackHandler>>,
}

impl ChainFallback {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    pub fn with_handler(mut self, handler: impl FallbackHandler + 'static) -> Self {
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

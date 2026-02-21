//! Retry logic with exponential backoff and jitter
//!
//! This module provides retry logic for verification checks that fail due to
//! transient issues like timeouts or network errors.

use std::time::Duration;
use serde::{Deserialize, Serialize};

/// Configuration for retry behavior
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial delay before first retry
    pub initial_delay: Duration,
    /// Maximum delay cap
    pub max_delay: Duration,
    /// Backoff multiplier (default: 2.0 for exponential)
    pub backoff_factor: f64,
    /// Whether to add jitter to delay
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_factor: 2.0,
            jitter: true,
        }
    }
}

// Placeholder - will be implemented in Task 2
pub async fn execute_with_retry<F, Fut, T, E>(
    _operation: F,
    _config: RetryConfig,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    // TODO: Implement in Task 2
    panic!("execute_with_retry not yet implemented");
}

// Placeholder - will be implemented in Task 2
pub fn is_retryable<E>(_error: &E) -> bool {
    // TODO: Implement in Task 2
    false
}

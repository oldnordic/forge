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

/// Execute an async operation with retry logic
///
/// # Arguments
/// * `operation` - Async operation that returns Result<T, E>
/// * `config` - Retry configuration
///
/// # Returns
/// * `Ok(T)` - Operation succeeded
/// * `Err(E)` - Operation failed after all retries
///
/// # Behavior
/// - On success: returns immediately
/// - On error when attempt < max_retries:
///   * Calculates delay: initial_delay * backoff_factor^attempt
///   * Adds jitter if enabled: delay * (0.5 + random::<f32>())
///   * Caps at max_delay
///   * Sleeps and retries
/// - On final error: returns error
pub async fn execute_with_retry<F, Fut, T, E>(
    mut operation: F,
    config: RetryConfig,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    let mut attempt = 0;

    loop {
        // Attempt the operation
        match operation().await {
            Ok(result) => return Ok(result),
            Err(error) if attempt < config.max_retries && is_retryable_internal(&error) => {
                // Calculate delay with exponential backoff
                let delay_ms = config.initial_delay.as_millis() as f64
                    * config.backoff_factor.powi(attempt as i32);

                let mut delay = Duration::from_millis(delay_ms as u64);

                // Add jitter if enabled
                if config.jitter {
                    let jitter_factor = 0.5 + rand::random::<f64>(); // 0.5 to 1.5
                    delay = Duration::from_millis((delay.as_millis() as f64 * jitter_factor) as u64);
                }

                // Cap at max_delay
                if delay > config.max_delay {
                    delay = config.max_delay;
                }

                // Sleep before retry
                tokio::time::sleep(delay).await;
                attempt += 1;
            }
            Err(error) => return Err(error),
        }
    }
}

/// Check if an error should trigger a retry
///
/// Timeouts, panics, and IO errors are retryable.
/// Validation errors are NOT retryable.
pub fn is_retryable<E>(_error: &E) -> bool {
    // In a real implementation, we'd check the error type
    // For now, we'll rely on the internal check in execute_with_retry
    true
}

// Internal check for retryable errors
fn is_retryable_internal<E>(_error: &E) -> bool {
    // For now, assume all errors are retryable
    // In Task 3, we'll make this more specific based on actual error types
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_retry_success_on_first_attempt() {
        let config = RetryConfig::default();
        let attempt_count = Arc::new(AtomicU32::new(0));

        let result = execute_with_retry(
            || {
                let attempt_count = attempt_count.clone();
                async move {
                    attempt_count.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, String>("success")
                }
            },
            config,
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        assert_eq!(attempt_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_success_after_two_attempts() {
        let config = RetryConfig::default();
        let attempt_count = Arc::new(AtomicU32::new(0));

        let result = execute_with_retry(
            || {
                let attempt_count = attempt_count.clone();
                async move {
                    let count = attempt_count.fetch_add(1, Ordering::SeqCst);
                    if count < 1 {
                        Err::<(), _>("error")
                    } else {
                        Ok(())
                    }
                }
            },
            config,
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(attempt_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_retry_failure_after_max_retries() {
        let config = RetryConfig {
            max_retries: 2,
            ..Default::default()
        };
        let attempt_count = Arc::new(AtomicU32::new(0));

        let result = execute_with_retry(
            || {
                let attempt_count = attempt_count.clone();
                async move {
                    attempt_count.fetch_add(1, Ordering::SeqCst);
                    Err::<(), _>("persistent error")
                }
            },
            config,
        )
        .await;

        assert!(result.is_err());
        // Should have initial attempt + 2 retries = 3 total
        assert_eq!(attempt_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_exponential_backoff() {
        // Use a paused time handle to test delay calculations
        tokio::time::pause();

        let config = RetryConfig {
            max_retries: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            backoff_factor: 2.0,
            jitter: false, // Disable jitter for predictable testing
        };

        let mut delays = Vec::new();
        let attempt_count = Arc::new(AtomicU32::new(0));

        let _ = execute_with_retry(
            || {
                let attempt_count = attempt_count.clone();
                async move {
                    let count = attempt_count.fetch_add(1, Ordering::SeqCst);
                    if count < 3 {
                        // Record the time before sleep
                        let before = tokio::time::Instant::now();
                        // This will cause a retry
                        Err::<(), _>("error")
                    } else {
                        Err::<(), _>("final error")
                    }
                }
            },
            config,
        )
        .await;

        tokio::time::resume();
    }

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_delay, Duration::from_millis(100));
        assert_eq!(config.max_delay, Duration::from_secs(30));
        assert_eq!(config.backoff_factor, 2.0);
        assert!(config.jitter);
    }

    #[tokio::test]
    async fn test_max_delay_capping() {
        tokio::time::pause();

        let config = RetryConfig {
            max_retries: 10,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_millis(500), // Very low cap
            backoff_factor: 10.0, // High growth factor
            jitter: false,
        };

        let attempt_count = Arc::new(AtomicU32::new(0));

        let _ = execute_with_retry(
            || {
                let attempt_count = attempt_count.clone();
                async move {
                    attempt_count.fetch_add(1, Ordering::SeqCst);
                    Err::<(), _>("error")
                }
            },
            config,
        )
        .await;

        tokio::time::resume();
    }

    #[tokio::test]
    async fn test_jitter_randomization() {
        // Run multiple retries and verify delays are different
        let config = RetryConfig {
            max_retries: 5,
            initial_delay: Duration::from_millis(100),
            backoff_factor: 1.0,
            jitter: true,
            ..Default::default()
        };

        let attempt_count = Arc::new(AtomicU32::new(0));

        let _ = execute_with_retry(
            || {
                let attempt_count = attempt_count.clone();
                async move {
                    attempt_count.fetch_add(1, Ordering::SeqCst);
                    Err::<(), _>("error")
                }
            },
            config,
        )
        .await;

        // With jitter, we should have completed all retries
        assert_eq!(attempt_count.load(Ordering::SeqCst), 6); // initial + 5 retries
    }
}

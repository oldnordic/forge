//! Async cancellation token system with parent-child hierarchy.
//!
//! Provides a cooperative cancellation model for workflows and tasks,
//! allowing users to cancel running operations and propagate cancellation
//! to all active tasks through a parent-child token hierarchy.
//!
//! # Architecture
//!
//! The cancellation system consists of three core types:
//! - [`CancellationToken`]: Thread-safe token representing cancellation state
//! - [`CancellationTokenSource`]: Owner of the parent token with cancel() method
//! - [`ChildToken`]: Derived child token for task-level cancellation
//!
//! # Cooperative Cancellation Patterns
//!
//! This module supports two main patterns for cooperative cancellation:
//!
//! ## 1. Polling Pattern
//!
//! Poll the cancellation token in long-running loops:
//!
//! ```ignore
//! while !token.poll_cancelled() {
//!     // Do work
//!     tokio::time::sleep(Duration::from_millis(100)).await;
//! }
//! ```
//!
//! ## 2. Async Wait Pattern
//!
//! Wait for cancellation signal asynchronously:
//!
//! ```ignore
//! tokio::select! {
//!     _ = token.wait_cancelled() => {
//!         // Handle cancellation
//!     }
//!     result = do_work() => {
//!         // Handle completion
//!     }
//! }
//! ```
//!
//! # Example
//!
//! ```ignore
//! use forge_agent::workflow::{CancellationTokenSource, CancellationToken};
//!
//! // Create cancellation source for workflow
//! let source = CancellationTokenSource::new();
//! let token = source.token();
//!
//! // Pass token to tasks
//! tokio::spawn(async move {
//!     while !token.is_cancelled() {
//!         // Do work
//!     }
//! });
//!
//! // Cancel from anywhere
//! source.cancel();
//! ```

use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;

/// Thread-safe cancellation token.
///
/// Wraps an Arc<AtomicBool> for thread-safe cancellation state.
/// Tokens can be cheaply cloned and shared across tasks.
///
/// # Cooperative Cancellation
///
/// Tasks can cooperatively respond to cancellation by:
/// - Polling with [`poll_cancelled()`](Self::poll_cancelled) in loops
/// - Awaiting with [`wait_cancelled()`](Self::wait_cancelled) in async contexts
///
/// # Cloning
///
/// Cloning a token creates a new reference to the same cancellation state.
/// When any token is cancelled (via CancellationTokenSource), all clones
/// will report as cancelled.
///
/// # Example
///
/// ```ignore
/// let source = CancellationTokenSource::new();
/// let token1 = source.token();
/// let token2 = token1.clone(); // Same state
///
/// assert!(!token1.is_cancelled());
/// assert!(!token2.is_cancelled());
///
/// source.cancel();
///
/// assert!(token1.is_cancelled());
/// assert!(token2.is_cancelled());
/// ```
#[derive(Clone, Debug)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

impl CancellationToken {
    /// Creates a new non-cancelled token.
    pub(crate) fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            notify: Arc::new(Notify::new()),
        }
    }

    /// Returns true if the token has been cancelled.
    ///
    /// Uses Ordering::SeqCst for strongest memory guarantees to ensure
    /// cancellation is visible across all threads.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let source = CancellationTokenSource::new();
    /// let token = source.token();
    ///
    /// assert!(!token.is_cancelled());
    /// source.cancel();
    /// assert!(token.is_cancelled());
    /// ```
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Polls the cancellation state - semantic alias for [`is_cancelled()`].
    ///
    /// This method is intended for use in long-running loops where tasks
    /// cooperatively check for cancellation. The naming makes the intent
    /// clearer than [`is_cancelled()`] in polling contexts.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Polling pattern in a loop
    /// while !token.poll_cancelled() {
    ///     // Do work
    ///     tokio::time::sleep(Duration::from_millis(100)).await;
    /// }
    /// ```
    pub fn poll_cancelled(&self) -> bool {
        self.is_cancelled()
    }

    /// Async method that waits until the token is cancelled.
    ///
    /// This uses polling with tokio::time::sleep to avoid busy-waiting.
    /// Multiple tasks can wait simultaneously.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Wait for cancellation
    /// token.wait_until_cancelled().await;
    /// println!("Token was cancelled!");
    /// ```
    ///
    /// # Use with tokio::select!
    ///
    /// ```ignore
    /// tokio::select! {
    ///     _ = token.wait_until_cancelled() => {
    ///         println!("Cancelled!");
    ///     }
    ///     result = do_work() => {
    ///         println!("Work completed: {:?}", result);
    ///     }
    /// }
    /// ```
    pub async fn wait_until_cancelled(&self) {
        while !self.is_cancelled() {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    /// Returns a Future that completes when this token is cancelled.
    ///
    /// This is equivalent to [`wait_until_cancelled()`] but returns a named future type
    /// that can be stored and passed around.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let future = token.wait_cancelled();
    /// // ... later
    /// future.await;
    /// ```
    pub fn wait_cancelled(&self) -> impl Future<Output = ()> + Send + Sync + 'static {
        let cancelled = self.cancelled.clone();
        async move {
            while !cancelled.load(Ordering::SeqCst) {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        }
    }
}

/// Source that owns a cancellation token and can trigger cancellation.
///
/// The CancellationTokenSource is the owner of the parent token and provides
/// the cancel() method to set the cancellation state. When cancelled, all
/// child tokens and clones will report as cancelled.
///
/// # Parent-Child Hierarchy
///
/// The source can create child tokens via child_token(), which allows for
/// hierarchical cancellation. Children inherit the parent's cancellation state.
///
/// # Cloning
///
/// Cloning a source creates a new handle to the same underlying token.
/// This allows multiple parts of the code to share cancellation control.
///
/// # Example
///
/// ```ignore
/// let source = CancellationTokenSource::new();
/// let token = source.token();
///
/// // Pass token to workflow
/// tokio::spawn(async move {
///     while !token.is_cancelled() {
///         // Do work
///     }
/// });
///
/// // Cancel workflow from main thread
/// source.cancel();
/// ```
#[derive(Clone)]
pub struct CancellationTokenSource {
    token: CancellationToken,
}

impl CancellationTokenSource {
    /// Creates a new cancellation source with a fresh token.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let source = CancellationTokenSource::new();
    /// let token = source.token();
    /// assert!(!token.is_cancelled());
    /// ```
    pub fn new() -> Self {
        Self {
            token: CancellationToken::new(),
        }
    }

    /// Returns a reference to the parent token.
    ///
    /// The token can be cloned and passed to tasks for cancellation checking.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let source = CancellationTokenSource::new();
    /// let token = source.token();
    /// let token2 = token.clone(); // Both reference same state
    /// ```
    pub fn token(&self) -> CancellationToken {
        self.token.clone()
    }

    /// Cancels the token, propagating to all child tokens and clones.
    ///
    /// This method is idempotent - calling it multiple times has no additional effect.
    /// All tasks waiting via [`wait_cancelled()`](CancellationToken::wait_cancelled) will be woken.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let source = CancellationTokenSource::new();
    /// let token = source.token();
    ///
    /// source.cancel();
    /// assert!(token.is_cancelled());
    ///
    /// source.cancel(); // Idempotent - no additional effect
    /// assert!(token.is_cancelled());
    /// ```
    pub fn cancel(&self) {
        self.token.cancelled.store(true, Ordering::SeqCst);
        self.token.notify.notify_waiters();
    }

    /// Creates a child token that inherits parent cancellation.
    ///
    /// Child tokens check both their local state and the parent's state.
    /// This allows for task-level cancellation independent of workflow cancellation.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let source = CancellationTokenSource::new();
    /// let child = source.child_token();
    ///
    /// // Child inherits parent cancellation
    /// source.cancel();
    /// assert!(child.is_cancelled());
    /// ```
    pub fn child_token(&self) -> ChildToken {
        ChildToken {
            parent: self.token.clone(),
            local_cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Default for CancellationTokenSource {
    fn default() -> Self {
        Self::new()
    }
}

/// Child cancellation token that inherits from a parent.
///
/// Child tokens check both their local cancellation state and their parent's
/// state. This allows for hierarchical cancellation where a task can be
/// cancelled independently or inherit cancellation from its parent workflow.
///
/// # Cancellation Logic
///
/// A child token is cancelled if:
/// - The parent token is cancelled, OR
/// - The child's local cancel() method was called
///
/// # Example
///
/// ```ignore
/// let source = CancellationTokenSource::new();
/// let child = source.child_token();
///
/// // Child inherits parent cancellation
/// source.cancel();
/// assert!(child.is_cancelled());
///
/// // Or child can be cancelled independently
/// let source2 = CancellationTokenSource::new();
/// let child2 = source2.child_token();
/// child2.cancel();
/// assert!(child2.is_cancelled());
/// assert!(!source2.token().is_cancelled());
/// ```
#[derive(Clone)]
pub struct ChildToken {
    parent: CancellationToken,
    local_cancelled: Arc<AtomicBool>,
}

impl ChildToken {
    /// Returns true if either parent or local token is cancelled.
    ///
    /// Checks both the parent token and local cancellation state.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let source = CancellationTokenSource::new();
    /// let child = source.child_token();
    ///
    /// assert!(!child.is_cancelled());
    ///
    /// // Cancel parent
    /// source.cancel();
    /// assert!(child.is_cancelled());
    /// ```
    pub fn is_cancelled(&self) -> bool {
        self.parent.is_cancelled() || self.local_cancelled.load(Ordering::SeqCst)
    }

    /// Cancels this child token locally.
    ///
    /// Local cancellation does not affect the parent token or other children.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let source = CancellationTokenSource::new();
    /// let child1 = source.child_token();
    /// let child2 = source.child_token();
    ///
    /// child1.cancel();
    /// assert!(child1.is_cancelled());
    /// assert!(!child2.is_cancelled()); // Other children unaffected
    /// assert!(!source.token().is_cancelled()); // Parent unaffected
    /// ```
    pub fn cancel(&self) {
        self.local_cancelled.store(true, Ordering::SeqCst);
    }
}

impl std::fmt::Debug for ChildToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChildToken")
            .field("parent_cancelled", &self.parent.is_cancelled())
            .field("local_cancelled", &self.local_cancelled.load(Ordering::SeqCst))
            .field("is_cancelled", &self.is_cancelled())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_initially_not_cancelled() {
        let source = CancellationTokenSource::new();
        let token = source.token();

        assert!(!token.is_cancelled());
    }

    #[test]
    fn test_source_cancel_sets_token() {
        let source = CancellationTokenSource::new();
        let token = source.token();

        source.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_token_clone_shares_state() {
        let source = CancellationTokenSource::new();
        let token1 = source.token();
        let token2 = token1.clone();

        source.cancel();

        // Both clones should see cancellation
        assert!(token1.is_cancelled());
        assert!(token2.is_cancelled());
    }

    #[test]
    fn test_child_token_inherits_parent_cancellation() {
        let source = CancellationTokenSource::new();
        let child = source.child_token();

        assert!(!child.is_cancelled());

        // Cancel parent
        source.cancel();
        assert!(child.is_cancelled());
    }

    #[test]
    fn test_child_token_independent_cancel() {
        let source = CancellationTokenSource::new();
        let child = source.child_token();

        // Cancel child locally
        child.cancel();
        assert!(child.is_cancelled());

        // Parent should not be cancelled
        assert!(!source.token().is_cancelled());
    }

    #[test]
    fn test_multiple_children_all_cancelled() {
        let source = CancellationTokenSource::new();
        let child1 = source.child_token();
        let child2 = source.child_token();
        let child3 = source.child_token();

        // Cancel parent
        source.cancel();

        // All children should be cancelled
        assert!(child1.is_cancelled());
        assert!(child2.is_cancelled());
        assert!(child3.is_cancelled());
    }

    #[test]
    fn test_cancellation_thread_safe() {
        use std::thread;
        use std::time::Duration;

        let source = CancellationTokenSource::new();
        let token = source.token();
        let token_clone = token.clone();

        // Spawn thread to check cancellation
        let handle = thread::spawn(move || {
            while !token_clone.is_cancelled() {
                // Busy wait (for testing only)
            }
            // Thread should exit when cancelled
        });

        // Give thread time to start
    thread::sleep(Duration::from_millis(10));

    // Cancel from main thread
    source.cancel();

    // Thread should exit
    handle.join().unwrap();
}

    #[test]
    fn test_token_debug_display() {
        let source = CancellationTokenSource::new();
        let token = source.token();

        // Debug should work
        let debug_str = format!("{:?}", token);
        assert!(debug_str.contains("CancellationToken"));

        // Cancel and debug again
        source.cancel();
        let debug_str_cancelled = format!("{:?}", token);
        assert!(debug_str_cancelled.contains("CancellationToken"));
    }

    #[test]
    fn test_child_token_debug_display() {
        let source = CancellationTokenSource::new();
        let child = source.child_token();

        // Debug should show state
        let debug_str = format!("{:?}", child);
        assert!(debug_str.contains("ChildToken"));
        assert!(debug_str.contains("parent_cancelled: false"));
        assert!(debug_str.contains("local_cancelled: false"));

        // Cancel parent
        source.cancel();
        let debug_str_cancelled = format!("{:?}", child);
        assert!(debug_str_cancelled.contains("parent_cancelled: true"));
    }

    #[test]
    fn test_source_cancel_idempotent() {
        let source = CancellationTokenSource::new();
        let token = source.token();

        // Cancel multiple times
        source.cancel();
        source.cancel();
        source.cancel();

        // Should still be cancelled
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_child_token_parent_and_local_both_cancelled() {
        let source = CancellationTokenSource::new();
        let child = source.child_token();

        // Cancel both parent and child
        source.cancel();
        child.cancel();

        // Should still be cancelled
        assert!(child.is_cancelled());
    }

    #[test]
    fn test_default_source() {
        let source = CancellationTokenSource::default();
        let token = source.token();

        assert!(!token.is_cancelled());
    }

    // Tests for cooperative cancellation

    #[test]
    fn test_poll_cancelled_returns_false_initially() {
        let source = CancellationTokenSource::new();
        let token = source.token();

        assert!(!token.poll_cancelled());
    }

    #[test]
    fn test_poll_cancelled_returns_true_after_cancel() {
        let source = CancellationTokenSource::new();
        let token = source.token();

        source.cancel();
        assert!(token.poll_cancelled());
    }

    #[tokio::test]
    async fn test_wait_cancelled_completes_on_cancel() {
        let source = CancellationTokenSource::new();
        let token = source.token();

        // Spawn a task to cancel after a delay
        let source_clone = source.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            source_clone.cancel();
        });

        // Wait for cancellation - should complete within 200ms
        let start = std::time::Instant::now();
        token.wait_cancelled().await;
        let elapsed = start.elapsed();

        assert!(elapsed < tokio::time::Duration::from_millis(200));
        assert!(token.is_cancelled());
    }

    #[tokio::test]
    async fn test_wait_cancelled_multiple_waiters() {
        let source = CancellationTokenSource::new();
        let token1 = source.token();
        let token2 = source.token();
        let token3 = source.token();

        // Spawn multiple waiters
        let handle1 = tokio::spawn(async move {
            token1.wait_cancelled().await;
        });

        let handle2 = tokio::spawn(async move {
            token2.wait_cancelled().await;
        });

        let handle3 = tokio::spawn(async move {
            token3.wait_cancelled().await;
        });

        // Cancel after a delay
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        source.cancel();

        // All waiters should complete
        let start = std::time::Instant::now();
        let (r1, r2, r3) = tokio::join!(handle1, handle2, handle3);
        let elapsed = start.elapsed();

        assert!(r1.is_ok());
        assert!(r2.is_ok());
        assert!(r3.is_ok());
        assert!(elapsed < tokio::time::Duration::from_millis(200));
    }

    #[tokio::test]
    async fn test_wait_cancelled_idempotent() {
        let source = CancellationTokenSource::new();
        let token = source.token();

        // Cancel immediately
        source.cancel();

        // Multiple waits should all complete immediately
        let start = std::time::Instant::now();
        token.wait_cancelled().await;
        token.wait_cancelled().await;
        token.wait_cancelled().await;
        let elapsed = start.elapsed();

        assert!(elapsed < tokio::time::Duration::from_millis(10));
    }

    #[tokio::test]
    async fn test_cooperative_cancellation_pattern() {
        let source = CancellationTokenSource::new();
        let token = source.token();

        // Simulate a task that cooperatively polls for cancellation
        let token_clone = token.clone();
        let handle = tokio::spawn(async move {
            let mut iterations = 0;
            while !token_clone.poll_cancelled() {
                iterations += 1;
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

                // Safety limit to avoid infinite loop in test
                if iterations >= 100 {
                    break;
                }
            }
            iterations
        });

        // Cancel after 50ms
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        source.cancel();

        // Task should exit early due to cancellation
        let iterations = handle.await.unwrap();
        assert!(iterations < 100); // Should not complete all 100 iterations
        assert!(iterations > 2); // Should have done some work
    }

    // Integration test with WorkflowExecutor

    #[tokio::test]
    async fn test_workflow_cancellation_with_executor() {
        use crate::workflow::dag::Workflow;
        use crate::workflow::executor::WorkflowExecutor;
        use crate::workflow::task::{TaskContext, TaskId, TaskResult, WorkflowTask};
        use async_trait::async_trait;

        // Create a simple task for testing
        struct SimpleTask {
            id: TaskId,
            name: String,
        }

        #[async_trait]
        impl WorkflowTask for SimpleTask {
            async fn execute(&self, _context: &TaskContext) -> Result<TaskResult, crate::workflow::TaskError> {
                // Simulate some work
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                Ok(TaskResult::Success)
            }

            fn id(&self) -> TaskId {
                self.id.clone()
            }

            fn name(&self) -> &str {
                &self.name
            }
        }

        // Create workflow with 5 sequential tasks
        let mut workflow = Workflow::new();
        for i in 1..=5 {
            workflow.add_task(Box::new(SimpleTask {
                id: TaskId::new(format!("task-{}", i)),
                name: format!("Task {}", i),
            }));
        }

        // Create cancellation source and cancel before execution
        let source = CancellationTokenSource::new();
        let mut executor = WorkflowExecutor::new(workflow)
            .with_cancellation_source(source);

        // Cancel immediately
        executor.cancel();

        // Execute workflow (should be cancelled immediately)
        let result = executor.execute().await.unwrap();

        // Workflow should be cancelled with no tasks completed
        assert!(!result.success);
        assert_eq!(result.completed_tasks.len(), 0);

        // Verify cancellation was recorded in audit log
        let events = executor.audit_log().replay();
        assert!(events.iter().any(|e| matches!(e, crate::audit::AuditEvent::WorkflowCancelled { .. })));
    }
}

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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Thread-safe cancellation token.
///
/// Wraps an Arc<AtomicBool> for thread-safe cancellation state.
/// Tokens can be cheaply cloned and shared across tasks.
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
}

impl CancellationToken {
    /// Creates a new non-cancelled token.
    pub(crate) fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
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
}

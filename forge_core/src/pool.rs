//! Connection pool for concurrent database access.
//!
//! This module provides a semaphore-based connection pool
//! for limiting concurrent database connections.

use std::sync::Arc;
use tokio::sync::Semaphore;

/// Connection pool for database connections.
///
/// The pool limits the number of concurrent connections
/// using a semaphore permit system.
///
/// # Examples
///
/// ```no_run
/// use forge_core::pool::ConnectionPool;
///
/// # #[tokio::main]
/// # async fn main() -> anyhow::Result<()> {
/// let pool = ConnectionPool::new("/path/to/db.sqlite", 10);
///
/// // Acquire a connection
/// let _permit = pool.acquire().await?;
/// // Use connection here
/// #     Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct ConnectionPool {
    /// Path to the database file.
    pub db_path: std::path::PathBuf,
    /// Semaphore for limiting connections.
    semaphore: Arc<Semaphore>,
    /// Maximum number of connections.
    pub max_connections: usize,
}

impl ConnectionPool {
    /// Creates a new connection pool.
    ///
    /// # Arguments
    ///
    /// * `db_path` - Path to the SQLite database file
    /// * `max_connections` - Maximum number of concurrent connections
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use forge_core::pool::ConnectionPool;
    ///
    /// let pool = ConnectionPool::new("./db.sqlite", 10);
    /// ```
    pub fn new(db_path: impl AsRef<std::path::Path>, max_connections: usize) -> Self {
        Self {
            db_path: db_path.as_ref().to_path_buf(),
            semaphore: Arc::new(Semaphore::new(max_connections)),
            max_connections,
        }
    }

    /// Acquires a permit from the pool.
    ///
    /// This will wait until a connection is available.
    /// The permit is released when dropped.
    ///
    /// # Returns
    ///
    /// A `ConnectionPermit` that represents the acquired connection.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use forge_core::pool::ConnectionPool;
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// # let pool = ConnectionPool::new("./db.sqlite", 10);
    /// let permit = pool.acquire().await?;
    /// // Use connection
    /// drop(permit); // Release back to pool
    /// #     Ok(())
    /// # }
    /// ```
    pub async fn acquire(&self) -> anyhow::Result<ConnectionPermit> {
        let permit = self.semaphore.clone().acquire_owned().await?;
        Ok(ConnectionPermit {
            _permit: permit,
            db_path: self.db_path.clone(),
        })
    }

    /// Returns the current number of available connections.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use forge_core::pool::ConnectionPool;
    /// # let pool = ConnectionPool::new("./db.sqlite", 10);
    /// let available = pool.available_connections();
    /// println!("Available connections: {}", available);
    /// ```
    pub fn available_connections(&self) -> usize {
        self.semaphore.available_permits()
    }

    /// Tries to acquire a permit without waiting.
    ///
    /// # Returns
    ///
    /// - `Some(permit)` if a connection is immediately available
    /// - `None` if all connections are in use
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use forge_core::pool::ConnectionPool;
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// # let pool = ConnectionPool::new("./db.sqlite", 10);
    /// if let Some(permit) = pool.try_acquire().await {
    ///     // Use connection
    /// } else {
    ///     // No connection available
    /// }
    /// #     Ok(())
    /// # }
    /// ```
    pub async fn try_acquire(&self) -> Option<ConnectionPermit> {
        self.semaphore.clone().try_acquire_owned().ok().map(|permit| ConnectionPermit {
            _permit: permit,
            db_path: self.db_path.clone(),
        })
    }
}

/// A permit representing an acquired connection.
///
/// When dropped, the connection is returned to the pool.
pub struct ConnectionPermit {
    _permit: tokio::sync::OwnedSemaphorePermit,
    db_path: std::path::PathBuf,
}

impl std::fmt::Debug for ConnectionPermit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionPermit")
            .field("db_path", &self.db_path)
            .finish()
    }
}

impl ConnectionPermit {
    /// Returns the path to the database file.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use forge_core::pool::ConnectionPool;
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// # let pool = ConnectionPool::new("./db.sqlite", 10);
    /// # let permit = pool.acquire().await?;
    /// let db_path = permit.db_path();
    /// println!("Connected to: {:?}", db_path);
    /// #     Ok(())
    /// # }
    /// ```
    pub fn db_path(&self) -> &std::path::Path {
        &self.db_path
    }
}

impl std::fmt::Display for ConnectionPermit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ConnectionPermit({})", self.db_path.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pool_creation() {
        let pool = ConnectionPool::new("/tmp/test.db", 5);
        assert_eq!(pool.max_connections, 5);
        assert_eq!(pool.available_connections(), 5);
    }

    #[tokio::test]
    async fn test_pool_acquire() {
        let pool = ConnectionPool::new("/tmp/test.db", 2);

        let permit1 = pool.acquire().await.unwrap();
        assert_eq!(pool.available_connections(), 1);

        let permit2 = pool.acquire().await.unwrap();
        assert_eq!(pool.available_connections(), 0);

        // Dropping permit returns it to pool
        drop(permit1);
        assert_eq!(pool.available_connections(), 1);

        drop(permit2);
        assert_eq!(pool.available_connections(), 2);
    }

    #[tokio::test]
    async fn test_pool_try_acquire() {
        let pool = ConnectionPool::new("/tmp/test.db", 1);

        let permit1 = pool.try_acquire().await;
        assert!(permit1.is_some());
        assert_eq!(pool.available_connections(), 0);

        // Second acquire fails
        let permit2 = pool.try_acquire().await;
        assert!(permit2.is_none());

        drop(permit1);
        assert_eq!(pool.available_connections(), 1);
    }

    #[tokio::test]
    async fn test_pool_db_path() {
        let pool = ConnectionPool::new("/tmp/test.db", 5);
        assert_eq!(pool.db_path, std::path::PathBuf::from("/tmp/test.db"));

        let permit = pool.acquire().await.unwrap();
        assert_eq!(permit.db_path(), std::path::Path::new("/tmp/test.db"));
    }

    #[tokio::test]
    async fn test_pool_concurrent_acquires() {
        use tokio::sync::Barrier;

        let pool = Arc::new(ConnectionPool::new("/tmp/test.db", 5));
        let barrier = Arc::new(Barrier::new(10));
        let mut handles = vec![];

        // Spawn 10 tasks trying to acquire
        for _i in 0..10 {
            let pool_clone = pool.clone();
            let barrier_clone = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier_clone.wait().await; // Coordinate start
                let _permit = pool_clone.acquire().await.unwrap();
                // Hold permit briefly
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                // Permit releases here when dropped
            }));
        }

        // Wait for all to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // All 10 should have completed eventually
        assert_eq!(pool.available_connections(), 5);
    }

    #[tokio::test]
    async fn test_pool_timeout_behavior() {
        use tokio::time::{timeout, Duration};

        let pool = ConnectionPool::new("/tmp/test.db", 1);

        // Acquire 1 permit
        let _permit1 = pool.acquire().await.unwrap();
        assert_eq!(pool.available_connections(), 0);

        // Try to acquire another - should timeout
        let start = std::time::Instant::now();
        let result = timeout(Duration::from_millis(100), pool.acquire()).await;

        let elapsed = start.elapsed();

        // Should have timed out
        assert!(result.is_err());
        // Should have taken approximately the timeout duration
        assert!(elapsed >= Duration::from_millis(90));
        assert!(elapsed < Duration::from_millis(200));
    }

    #[tokio::test]
    async fn test_pool_permit_drop_returns() {
        let pool = ConnectionPool::new("/tmp/test.db", 3);
        assert_eq!(pool.available_connections(), 3);

        // Acquire permit
        let permit = pool.acquire().await.unwrap();
        assert_eq!(pool.available_connections(), 2);

        // Drop permit
        drop(permit);
        assert_eq!(pool.available_connections(), 3);
    }

    #[tokio::test]
    async fn test_pool_stress() {
        let pool = ConnectionPool::new("/tmp/test.db", 10);

        // Run 100 acquire/release cycles
        for _ in 0..100 {
            let permit = pool.acquire().await.unwrap();
            // Verify no deadlocks
            assert_eq!(permit.db_path(), std::path::Path::new("/tmp/test.db"));
            drop(permit);
        }

        // Verify final available equals max
        assert_eq!(pool.available_connections(), 10);
    }

    #[tokio::test]
    async fn test_pool_all_permits_acquired() {
        let pool = ConnectionPool::new("/tmp/test.db", 3);

        // Acquire all permits up to max
        let permit1 = pool.acquire().await.unwrap();
        let permit2 = pool.acquire().await.unwrap();
        let permit3 = pool.acquire().await.unwrap();

        // Verify available is 0
        assert_eq!(pool.available_connections(), 0);

        // Verify try_acquire returns None
        let permit4 = pool.try_acquire().await;
        assert!(permit4.is_none());

        // Release one permit
        drop(permit1);

        // Verify try_acquire now works
        let permit5 = pool.try_acquire().await;
        assert!(permit5.is_some());

        // Clean up
        drop(permit2);
        drop(permit3);
        drop(permit5);
    }

    #[tokio::test]
    async fn test_pool_available_count() {
        let pool = ConnectionPool::new("/tmp/test.db", 5);

        // Initial available should be max
        assert_eq!(pool.available_connections(), 5);

        // Acquire varying number of permits
        let permit1 = pool.acquire().await.unwrap();
        assert_eq!(pool.available_connections(), 4);

        let permit2 = pool.acquire().await.unwrap();
        assert_eq!(pool.available_connections(), 3);

        let permit3 = pool.acquire().await.unwrap();
        assert_eq!(pool.available_connections(), 2);

        // Drop permits and verify available increases
        drop(permit1);
        assert_eq!(pool.available_connections(), 3);

        drop(permit2);
        assert_eq!(pool.available_connections(), 4);

        drop(permit3);
        assert_eq!(pool.available_connections(), 5);
    }
}

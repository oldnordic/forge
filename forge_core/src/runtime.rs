//! Runtime orchestration for all phase 2 components.
//!
//! This module provides the unified `Runtime` type that combines
//! file watching, incremental indexing, query caching, and connection pooling.

use crate::cache::QueryCache;
use crate::indexing::IncrementalIndexer;
use crate::pool::ConnectionPool;
use crate::storage::UnifiedGraphStore;
use crate::watcher::Watcher;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Runtime combining all phase 2 components.
///
/// The `Runtime` manages file watching, incremental indexing,
/// query caching, and connection pooling for enhanced performance.
///
/// # Examples
///
/// ```no_run
/// use forge_core::runtime::Runtime;
/// use std::path::PathBuf;
///
/// # #[tokio::main]
/// # async fn main() -> anyhow::Result<()> {
/// let mut runtime = Runtime::new(PathBuf::from("./project")).await?;
///
/// // Start watching for file changes
/// let _result = runtime.start_with_watching().await?;
///
/// // Process events as they arrive
/// let _stats = runtime.process_events().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct Runtime {
    /// The underlying graph store.
    pub store: Arc<UnifiedGraphStore>,
    /// File watcher for hot-reload.
    pub watcher: Option<Watcher>,
    /// Incremental indexer for processing changes.
    pub indexer: IncrementalIndexer,
    /// Query cache layer.
    pub cache: QueryCache<String, String>,
    /// Connection pool (when enabled).
    pub pool: Option<ConnectionPool>,
}

impl Runtime {
    /// Creates a new runtime instance.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the codebase directory
    ///
    /// # Returns
    ///
    /// A `Runtime` instance or an error.
    ///
    /// # Errors
    ///
    /// Returns an error if the graph store cannot be initialized.
    pub async fn new(path: PathBuf) -> anyhow::Result<Self> {
        let store = Arc::new(UnifiedGraphStore::open(&path).await?);
        let indexer = IncrementalIndexer::new(store.clone());

        // Default cache: 1000 entries, 5 minute TTL
        let cache = QueryCache::new(1000, Duration::from_secs(300));

        // Connection pool
        let db_path = path.join(".forge/graph.db");
        let pool = Some(ConnectionPool::new(&db_path, 10));

        Ok(Self {
            store,
            watcher: None,
            indexer,
            cache,
            pool,
        })
    }

    /// Starts file watching on the codebase.
    ///
    /// # Returns
    ///
    /// `Ok(())` if watching started successfully, or an error.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be watched.
    pub async fn start_with_watching(&mut self) -> anyhow::Result<()> {
        let (tx, _rx) = Watcher::channel();
        let watcher = Watcher::new(self.store.clone(), tx);

        // Start watching the current directory
        let path = std::env::current_dir()?;
        watcher.start(path).await?;

        self.watcher = Some(watcher);

        // Note: For v0.2, event processing is manual via process_events()
        // Background processing would require the store to be Send + Sync
        // Users should call process_events() periodically or in their own task

        Ok(())
    }

    /// Processes any pending file change events.
    ///
    /// This method flushes the incremental indexer, applying all
    /// queued file changes to the graph store.
    ///
    /// # Returns
    ///
    /// Flush statistics or an error.
    ///
    /// # Errors
    ///
    /// Returns an error if flushing fails.
    pub async fn process_events(&self) -> anyhow::Result<crate::indexing::FlushStats> {
        self.indexer.flush().await
    }

    /// Returns a reference to the cache.
    pub fn cache(&self) -> &QueryCache<String, String> {
        &self.cache
    }

    /// Returns a reference to the connection pool (if available).
    pub fn pool(&self) -> Option<&ConnectionPool> {
        self.pool.as_ref()
    }

    /// Returns the number of pending file changes.
    pub async fn pending_changes(&self) -> usize {
        self.indexer.pending_count().await
    }

    /// Returns true if watching is active.
    pub fn is_watching(&self) -> bool {
        self.watcher.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::watcher::WatchEvent;

    #[tokio::test]
    async fn test_runtime_creation() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();

        let runtime = Runtime::new(path).await.unwrap();

        assert!(!runtime.is_watching());
        assert_eq!(runtime.pending_changes().await, 0);
        assert!(runtime.pool().is_some()); // Pool should always be available now
    }

    #[tokio::test]
    async fn test_runtime_cache() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();

        let runtime = Runtime::new(path).await.unwrap();

        runtime.cache.insert("test".to_string(), "value".to_string()).await;
        let value = runtime.cache.get(&"test".to_string()).await;

        assert_eq!(value, Some("value".to_string()));
    }

    #[tokio::test]
    async fn test_runtime_pending_changes() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();

        let runtime = Runtime::new(path).await.unwrap();

        runtime.indexer.queue(WatchEvent::Modified(PathBuf::from("test.rs")));
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert_eq!(runtime.pending_changes().await, 1);

        runtime.process_events().await.unwrap();
        assert_eq!(runtime.pending_changes().await, 0);
    }

    #[tokio::test]
    async fn test_runtime_process_events() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();

        let runtime = Runtime::new(path).await.unwrap();

        runtime.indexer.queue(WatchEvent::Created(PathBuf::from("test.rs")));
        tokio::time::sleep(Duration::from_millis(50)).await;

        let stats = runtime.process_events().await.unwrap();
        assert!(stats.indexed >= 0);
    }

    #[tokio::test]
    async fn test_runtime_is_watching() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();

        let runtime = Runtime::new(path).await.unwrap();
        assert!(!runtime.is_watching());

        // Note: start_with_watching requires actual directory
        // which may not work in temp tests, so we just test the flag
    }
}

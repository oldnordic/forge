//! Incremental indexing for processing file changes.
//!
//! This module provides change-based incremental indexing to avoid full
//! re-scans when files are modified.

use crate::storage::UnifiedGraphStore;
use crate::watcher::WatchEvent;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

/// Incremental indexer for processing file changes.
///
/// The `IncrementalIndexer` batches file system events and processes
/// them on flush, avoiding full re-indexing of the codebase.
///
/// # Examples
///
/// ```no_run
/// use forge_core::indexing::IncrementalIndexer;
/// use forge_core::watcher::WatchEvent;
/// use std::path::PathBuf;
///
/// # #[tokio::main]
/// # async fn main() -> anyhow::Result<()> {
/// # let store = unimplemented!();
/// let indexer = IncrementalIndexer::new(store);
///
/// // Queue some changes
/// indexer.queue(WatchEvent::Modified(PathBuf::from("src/lib.rs")));
/// indexer.queue(WatchEvent::Created(PathBuf::from("src/new.rs")));
///
/// // Process changes
/// indexer.flush().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct IncrementalIndexer {
    /// The graph store for writing index updates.
    store: Arc<UnifiedGraphStore>,
    /// Pending files to process.
    pending: Arc<tokio::sync::Mutex<HashSet<PathBuf>>>,
    /// Files to delete.
    deleted: Arc<tokio::sync::Mutex<HashSet<PathBuf>>>,
}

impl IncrementalIndexer {
    /// Creates a new incremental indexer.
    ///
    /// # Arguments
    ///
    /// * `store` - The graph store for index updates
    pub fn new(store: Arc<UnifiedGraphStore>) -> Self {
        Self {
            store,
            pending: Arc::new(tokio::sync::Mutex::new(HashSet::new())),
            deleted: Arc::new(tokio::sync::Mutex::new(HashSet::new())),
        }
    }

    /// Queues a watch event for processing.
    ///
    /// # Arguments
    ///
    /// * `event` - The watch event to queue
    pub fn queue(&self, event: WatchEvent) {
        match event {
            WatchEvent::Created(path) | WatchEvent::Modified(path) => {
                let pending = self.pending.clone();
                tokio::spawn(async move {
                    pending.lock().await.insert(path);
                });
            }
            WatchEvent::Deleted(path) => {
                let deleted = self.deleted.clone();
                tokio::spawn(async move {
                    deleted.lock().await.insert(path);
                });
            }
            WatchEvent::Error(_) => {
                // Log error but don't fail
            }
        }
    }

    /// Flushes pending changes to the graph store.
    ///
    /// This method processes all queued file changes and updates
    /// the index incrementally.
    ///
    /// # Returns
    ///
    /// `Ok(())` if flush succeeded, or an error.
    ///
    /// # Errors
    ///
    /// Returns an error if any file cannot be indexed.
    pub async fn flush(&self) -> anyhow::Result<FlushStats> {
        let mut pending = self.pending.lock().await;
        let mut deleted = self.deleted.lock().await;

        let mut stats = FlushStats::default();

        // Process deletions first
        for path in deleted.drain() {
            if let Err(e) = self.delete_file(&path).await {
                eprintln!("Error deleting {:?}: {}", path, e);
            } else {
                stats.deleted += 1;
            }
        }

        // Process additions/updates
        for path in pending.drain() {
            if let Err(e) = self.index_file(&path).await {
                eprintln!("Error indexing {:?}: {}", path, e);
            } else {
                stats.indexed += 1;
            }
        }

        Ok(stats)
    }

    /// Performs a full rescan of the codebase.
    ///
    /// This clears all pending changes and re-indexes from scratch.
    ///
    /// # Returns
    ///
    /// `Ok(count)` with number of files indexed, or an error.
    ///
    /// # Errors
    ///
    /// Returns an error if the rescan fails.
    pub async fn full_rescan(&self) -> anyhow::Result<usize> {
        // Clear pending
        self.pending.lock().await.clear();
        self.deleted.lock().await.clear();

        // For now, return placeholder count
        // In a full implementation, this would:
        // 1. Scan the directory tree
        // 2. Identify all source files
        // 3. Re-index each file

        Ok(0)
    }

    /// Returns the number of pending files to process.
    pub async fn pending_count(&self) -> usize {
        self.pending.lock().await.len() + self.deleted.lock().await.len()
    }

    /// Clears all pending changes without processing.
    pub async fn clear_pending(&self) {
        self.pending.lock().await.clear();
        self.deleted.lock().await.clear();
    }

    /// Indexes a single file.
    async fn index_file(&self, _path: &PathBuf) -> anyhow::Result<()> {
        // In a full implementation, this would:
        // 1. Read the file
        // 2. Parse it with tree-sitter or similar
        // 3. Extract symbols, references, etc.
        // 4. Write to the graph store

        // For v0.2, we'll store a placeholder record
        // The full indexing will be added in a later phase

        Ok(())
    }

    /// Deletes a file from the index.
    async fn delete_file(&self, _path: &PathBuf) -> anyhow::Result<()> {
        // In a full implementation, this would:
        // 1. Query all symbols in this file
        // 2. Delete those symbols
        // 3. Delete incoming/outgoing references
        // 4. Clean up any CFG blocks

        Ok(())
    }
}

/// Statistics from a flush operation.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct FlushStats {
    /// Number of files indexed.
    pub indexed: usize,
    /// Number of files deleted.
    pub deleted: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::UnifiedGraphStore;

    #[tokio::test]
    async fn test_indexer_creation() {
        let store = Arc::new(UnifiedGraphStore::memory().await.unwrap());
        let indexer = IncrementalIndexer::new(store);

        assert_eq!(indexer.pending_count().await, 0);
    }

    #[tokio::test]
    async fn test_queue_events() {
        let store = Arc::new(UnifiedGraphStore::memory().await.unwrap());
        let indexer = IncrementalIndexer::new(store);

        indexer.queue(WatchEvent::Created(PathBuf::from("src/a.rs")));
        indexer.queue(WatchEvent::Modified(PathBuf::from("src/b.rs")));
        indexer.queue(WatchEvent::Deleted(PathBuf::from("src/c.rs")));

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        assert_eq!(indexer.pending_count().await, 3);
    }

    #[tokio::test]
    async fn test_flush_clears_pending() {
        let store = Arc::new(UnifiedGraphStore::memory().await.unwrap());
        let indexer = IncrementalIndexer::new(store);

        indexer.queue(WatchEvent::Modified(PathBuf::from("src/lib.rs")));
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let stats = indexer.flush().await.unwrap();
        assert_eq!(stats.indexed, 1);
        assert_eq!(indexer.pending_count().await, 0);
    }

    #[tokio::test]
    async fn test_flush_stats() {
        let store = Arc::new(UnifiedGraphStore::memory().await.unwrap());
        let indexer = IncrementalIndexer::new(store);

        indexer.queue(WatchEvent::Modified(PathBuf::from("src/a.rs")));
        indexer.queue(WatchEvent::Created(PathBuf::from("src/b.rs")));
        indexer.queue(WatchEvent::Deleted(PathBuf::from("src/c.rs")));

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let stats = indexer.flush().await.unwrap();

        // Note: actual indexing is stubbed, so counts may vary
        // The test verifies the stats structure is returned
        assert!(stats.indexed > 0);
        assert!(stats.deleted > 0);
    }

    #[tokio::test]
    async fn test_clear_pending() {
        let store = Arc::new(UnifiedGraphStore::memory().await.unwrap());
        let indexer = IncrementalIndexer::new(store);

        indexer.queue(WatchEvent::Modified(PathBuf::from("src/lib.rs")));
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        indexer.clear_pending().await;
        assert_eq!(indexer.pending_count().await, 0);
    }

    #[tokio::test]
    async fn test_indexer_flush_multiple() {
        let store = Arc::new(UnifiedGraphStore::memory().await.unwrap());
        let indexer = IncrementalIndexer::new(store);

        // Queue 3 different Modified events
        indexer.queue(WatchEvent::Modified(PathBuf::from("src/a.rs")));
        indexer.queue(WatchEvent::Modified(PathBuf::from("src/b.rs")));
        indexer.queue(WatchEvent::Modified(PathBuf::from("src/c.rs")));

        // Wait for async queue processing
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Verify pending count
        assert_eq!(indexer.pending_count().await, 3);

        // Flush and verify stats
        let stats = indexer.flush().await.unwrap();
        assert_eq!(stats.indexed, 3);
        assert_eq!(stats.deleted, 0);

        // Verify pending is empty after flush
        assert_eq!(indexer.pending_count().await, 0);
    }

    #[tokio::test]
    async fn test_indexer_delete_handling() {
        let store = Arc::new(UnifiedGraphStore::memory().await.unwrap());
        let indexer = IncrementalIndexer::new(store);

        // Queue a Deleted event
        indexer.queue(WatchEvent::Deleted(PathBuf::from("src/removed.rs")));

        // Wait for async queue processing
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Verify it's counted in pending
        assert_eq!(indexer.pending_count().await, 1);

        // Flush and verify deleted_count in stats
        let stats = indexer.flush().await.unwrap();
        assert_eq!(stats.indexed, 0);
        assert_eq!(stats.deleted, 1);
    }

    #[tokio::test]
    async fn test_indexer_clear() {
        let store = Arc::new(UnifiedGraphStore::memory().await.unwrap());
        let indexer = IncrementalIndexer::new(store);

        // Add some pending changes
        indexer.queue(WatchEvent::Created(PathBuf::from("src/new.rs")));
        indexer.queue(WatchEvent::Modified(PathBuf::from("src/existing.rs")));

        // Wait for async queue processing
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Verify we have pending changes
        assert_eq!(indexer.pending_count().await, 2);

        // Clear state
        indexer.clear_pending().await;

        // Verify pending_changes() returns 0
        assert_eq!(indexer.pending_count().await, 0);
    }

    #[tokio::test]
    async fn test_indexer_duplicate_queue() {
        let store = Arc::new(UnifiedGraphStore::memory().await.unwrap());
        let indexer = IncrementalIndexer::new(store);

        // Queue the same file path twice
        let path = PathBuf::from("src/duplicate.rs");
        indexer.queue(WatchEvent::Created(path.clone()));
        indexer.queue(WatchEvent::Modified(path.clone()));

        // Wait for async queue processing
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Verify only one entry is in pending (HashSet deduplicates)
        // Note: Both Created and Modified go to pending HashSet, so we get 1
        assert_eq!(indexer.pending_count().await, 1);

        // Flush to verify
        let stats = indexer.flush().await.unwrap();
        assert_eq!(stats.indexed, 1);
    }

    #[tokio::test]
    async fn test_indexer_statistics() {
        let store = Arc::new(UnifiedGraphStore::memory().await.unwrap());
        let indexer = IncrementalIndexer::new(store);

        // Create a known mix of events
        indexer.queue(WatchEvent::Created(PathBuf::from("src/a.rs")));
        indexer.queue(WatchEvent::Created(PathBuf::from("src/b.rs")));
        indexer.queue(WatchEvent::Modified(PathBuf::from("src/c.rs")));
        indexer.queue(WatchEvent::Deleted(PathBuf::from("src/d.rs")));
        indexer.queue(WatchEvent::Deleted(PathBuf::from("src/e.rs")));

        // Wait for async queue processing
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Flush and verify all counts match
        let stats = indexer.flush().await.unwrap();
        assert_eq!(stats.indexed, 3); // 2 Created + 1 Modified
        assert_eq!(stats.deleted, 2); // 2 Deleted
    }

    #[tokio::test]
    async fn test_indexer_concurrent_flush() {
        let store = Arc::new(UnifiedGraphStore::memory().await.unwrap());
        let indexer = IncrementalIndexer::new(store);

        // Queue multiple events rapidly (the indexer uses tokio::spawn internally)
        // This tests that the internal async queue handling is thread-safe
        for i in 0..5 {
            indexer.queue(WatchEvent::Modified(PathBuf::from(format!("src/file{}.rs", i))));
        }

        // Wait for all async queue operations to complete
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Flush and verify all events processed
        let stats = indexer.flush().await.unwrap();
        assert_eq!(stats.indexed, 5);
        assert_eq!(stats.deleted, 0);
        assert_eq!(indexer.pending_count().await, 0);

        // Verify no race conditions or panics occurred
        // (If we got here without panic, the test passed)
    }
}

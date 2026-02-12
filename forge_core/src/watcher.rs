//! File watching for hot-reload and incremental updates.
//!
//! This module provides file system monitoring using the `notify` crate,
//! enabling automatic detection of code changes for incremental indexing.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::storage::UnifiedGraphStore;

/// File system event emitted by the watcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchEvent {
    /// A new file was created.
    Created(PathBuf),
    /// An existing file was modified.
    Modified(PathBuf),
    /// A file was deleted.
    Deleted(PathBuf),
    /// An error occurred while watching.
    Error(String),
}

/// File watcher for monitoring codebase changes.
///
/// The `Watcher` spawns a background task that monitors the specified
/// directory and emits events via a channel. Events are consumed by
/// the incremental indexer for hot-reload capability.
#[derive(Clone, Debug)]
pub struct Watcher {
    /// The graph store (for future use in event correlation).
    store: Arc<UnifiedGraphStore>,
    /// Channel sender for watch events.
    sender: mpsc::UnboundedSender<WatchEvent>,
    /// The underlying notify watcher (kept alive to continue watching).
    #[allow(clippy::type_complexity)]
    inner: Arc<std::sync::Mutex<Option<notify::RecommendedWatcher>>>,
}

impl Watcher {
    /// Creates a new watcher instance.
    ///
    /// # Arguments
    ///
    /// * `store` - The graph store for event correlation
    /// * `sender` - Channel to send watch events
    pub(crate) fn new(
        store: Arc<UnifiedGraphStore>,
        sender: mpsc::UnboundedSender<WatchEvent>,
    ) -> Self {
        Self {
            store,
            sender,
            inner: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Starts watching the specified directory.
    ///
    /// Spawns a background task that recursively watches the directory
    /// and emits events for file system changes.
    ///
    /// # Arguments
    ///
    /// * `path` - Directory path to watch
    ///
    /// # Returns
    ///
    /// `Ok(())` if watching started successfully, or an error.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be watched.
    pub async fn start(&self, path: PathBuf) -> notify::Result<()> {
        use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};

        let sender = self.sender.clone();

        // Create event handler function
        let mut last_event = std::time::Instant::now();
        let mut last_path: Option<PathBuf> = None;

        let event_handler = move |res: notify::Result<notify::Event>| {
            // Debounce: ignore events within 100ms of same path
            let now = std::time::Instant::now();

            match res {
                Ok(event) => {
                    for path in event.paths {
                        // Check debounce
                        if let Some(last) = &last_path {
                            if last == &path && now.duration_since(last_event).as_millis() < 100 {
                                continue;
                            }
                        }

                        let watch_event = match event.kind {
                            notify::EventKind::Create(_) => WatchEvent::Created(path.clone()),
                            notify::EventKind::Modify(_) => WatchEvent::Modified(path.clone()),
                            notify::EventKind::Remove(_) => WatchEvent::Deleted(path.clone()),
                            _ => continue,
                        };

                        last_path = Some(path);
                        last_event = now;

                        let _ = sender.send(watch_event);
                    }
                }
                Err(e) => {
                    let _ = sender.send(WatchEvent::Error(e.to_string()));
                }
            }
        };

        // Create watcher and store it to keep alive
        let mut watcher = RecommendedWatcher::new(
            event_handler,
            notify::Config::default(),
        )?;
        watcher.watch(&path, RecursiveMode::Recursive)?;

        // Store the watcher to keep it alive
        *self.inner.lock().unwrap() = Some(watcher);

        Ok(())
    }

    /// Creates a new channel pair for watch events.
    ///
    /// # Returns
    ///
    /// A tuple of (sender, receiver) for watch events.
    pub fn channel() -> (mpsc::UnboundedSender<WatchEvent>, mpsc::UnboundedReceiver<WatchEvent>) {
        mpsc::unbounded_channel()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::UnifiedGraphStore;
    use tempfile::TempDir;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn test_watcher_creation() {
        let (tx, _rx) = Watcher::channel();
        let store = Arc::new(UnifiedGraphStore::memory().await.unwrap());
        let watcher = Watcher::new(store, tx);

        // Watcher should be clonable
        let _ = watcher.clone();
    }

    #[tokio::test]
    async fn test_watcher_channel() {
        let (tx, mut rx) = Watcher::channel();

        // Send an event
        let path = PathBuf::from("/test/file.rs");
        tx.send(WatchEvent::Created(path.clone())).unwrap();

        // Receive it
        let received = rx.recv().await.unwrap();
        assert_eq!(received, WatchEvent::Created(path));
    }

    #[tokio::test]
    async fn test_watch_event_equality() {
        let path = PathBuf::from("/test/file.rs");

        assert_eq!(WatchEvent::Created(path.clone()), WatchEvent::Created(path.clone()));
        assert_eq!(WatchEvent::Modified(path.clone()), WatchEvent::Modified(path.clone()));
        assert_eq!(WatchEvent::Deleted(path), WatchEvent::Deleted(PathBuf::from("/test/file.rs")));
        assert_ne!(WatchEvent::Created(PathBuf::from("/a.rs")), WatchEvent::Created(PathBuf::from("/b.rs")));
    }

    #[tokio::test]
    async fn test_watcher_create_event() {
        let temp_dir = TempDir::new().unwrap();
        let (tx, mut rx) = Watcher::channel();
        let store = Arc::new(UnifiedGraphStore::memory().await.unwrap());
        let watcher = Watcher::new(store, tx);

        // Start watching the temp directory
        watcher.start(temp_dir.path().to_path_buf()).await.unwrap();

        // Give the watcher a moment to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Create a test file
        let test_file = temp_dir.path().join("test_create.rs");
        tokio::fs::write(&test_file, "fn test() {}").await.unwrap();

        // Wait for the create event
        let event = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("Timeout waiting for create event")
            .expect("No event received");

        assert!(matches!(event, WatchEvent::Created(path) if path == test_file));
    }

    #[tokio::test]
    async fn test_watcher_modify_event() {
        let temp_dir = TempDir::new().unwrap();
        let (tx, mut rx) = Watcher::channel();
        let store = Arc::new(UnifiedGraphStore::memory().await.unwrap());
        let watcher = Watcher::new(store, tx);

        // Start watching the temp directory
        watcher.start(temp_dir.path().to_path_buf()).await.unwrap();

        // Give the watcher a moment to start
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Create a test file first
        let test_file = temp_dir.path().join("test_modify.rs");
        tokio::fs::write(&test_file, "fn test() {}").await.unwrap();

        // Wait for the create event and discard it
        let _ = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("Timeout waiting for create event");

        // Give the file system time to settle
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Modify the file
        tokio::fs::write(&test_file, "fn test() { println!(\"modified\"); }").await.unwrap();

        // Wait for the modify event (with longer timeout for file system)
        let event = timeout(Duration::from_secs(3), rx.recv())
            .await
            .expect("Timeout waiting for modify event")
            .expect("No event received");

        assert!(matches!(event, WatchEvent::Modified(path) if path == test_file));
    }

    #[tokio::test]
    async fn test_watcher_delete_event() {
        let temp_dir = TempDir::new().unwrap();
        let (tx, mut rx) = Watcher::channel();
        let store = Arc::new(UnifiedGraphStore::memory().await.unwrap());
        let watcher = Watcher::new(store, tx);

        // Start watching the temp directory
        watcher.start(temp_dir.path().to_path_buf()).await.unwrap();

        // Give the watcher a moment to start
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Create a test file first
        let test_file = temp_dir.path().join("test_delete.rs");
        tokio::fs::write(&test_file, "fn test() {}").await.unwrap();

        // Wait for the create event and discard it
        let _ = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("Timeout waiting for create event");

        // Give the file system time to settle
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Delete the file
        tokio::fs::remove_file(&test_file).await.unwrap();

        // Wait for the delete event (with longer timeout for file system)
        let event = timeout(Duration::from_secs(3), rx.recv())
            .await
            .expect("Timeout waiting for delete event")
            .expect("No event received");

        assert!(matches!(event, WatchEvent::Deleted(path) if path == test_file));
    }

    #[tokio::test]
    async fn test_watcher_recursive_watching() {
        let temp_dir = TempDir::new().unwrap();
        let (tx, mut rx) = Watcher::channel();
        let store = Arc::new(UnifiedGraphStore::memory().await.unwrap());
        let watcher = Watcher::new(store, tx);

        // Start watching the temp directory (should be recursive)
        watcher.start(temp_dir.path().to_path_buf()).await.unwrap();

        // Give the watcher a moment to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Create a subdirectory
        let subdir = temp_dir.path().join("subdir");
        tokio::fs::create_dir(&subdir).await.unwrap();

        // Wait for the directory create event and discard it (if any)
        let _ = timeout(Duration::from_secs(1), rx.recv()).await;

        // Give a brief moment for the file system to settle
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Create a file in the subdirectory
        let test_file = subdir.join("nested.rs");
        tokio::fs::write(&test_file, "fn nested() {}").await.unwrap();

        // Wait for the create event from the subdirectory
        let event = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("Timeout waiting for nested create event")
            .expect("No event received");

        assert!(matches!(event, WatchEvent::Created(path) if path == test_file));
    }

    #[tokio::test]
    async fn test_watcher_multiple_events() {
        let temp_dir = TempDir::new().unwrap();
        let (tx, mut rx) = Watcher::channel();
        let store = Arc::new(UnifiedGraphStore::memory().await.unwrap());
        let watcher = Watcher::new(store, tx);

        // Start watching the temp directory
        watcher.start(temp_dir.path().to_path_buf()).await.unwrap();

        // Give the watcher a moment to start
        tokio::time::sleep(Duration::from_millis(200)).await;

        let test_file = temp_dir.path().join("test_multiple.rs");

        // Create
        tokio::fs::write(&test_file, "fn test() {}").await.unwrap();

        // Wait for create event
        let event1 = timeout(Duration::from_secs(3), rx.recv())
            .await
            .expect("Timeout waiting for create event")
            .expect("No event received");
        assert!(matches!(event1, WatchEvent::Created(_)));

        // Give the file system time to settle
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Modify
        tokio::fs::write(&test_file, "fn test() { println!(\"modified\"); }").await.unwrap();

        // Wait for modify event (with longer timeout)
        let event2 = timeout(Duration::from_secs(3), rx.recv())
            .await
            .expect("Timeout waiting for modify event")
            .expect("No event received");
        assert!(matches!(event2, WatchEvent::Modified(_)));

        // Give the file system time to settle
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Delete
        tokio::fs::remove_file(&test_file).await.unwrap();

        // Wait for delete event (with longer timeout)
        let event3 = timeout(Duration::from_secs(3), rx.recv())
            .await
            .expect("Timeout waiting for delete event")
            .expect("No event received");
        assert!(matches!(event3, WatchEvent::Deleted(_)));

        // Verify all events were for the same file
        if let WatchEvent::Created(p1) = event1 {
            if let WatchEvent::Modified(p2) = event2 {
                if let WatchEvent::Deleted(p3) = event3 {
                    assert_eq!(p1, test_file);
                    assert_eq!(p2, test_file);
                    assert_eq!(p3, test_file);
                    return;
                }
            }
        }
        panic!("Events did not match expected sequence");
    }

    #[tokio::test]
    async fn test_watcher_debounce() {
        let temp_dir = TempDir::new().unwrap();
        let (tx, mut rx) = Watcher::channel();
        let store = Arc::new(UnifiedGraphStore::memory().await.unwrap());
        let watcher = Watcher::new(store, tx);

        // Start watching the temp directory
        watcher.start(temp_dir.path().to_path_buf()).await.unwrap();

        // Give the watcher a moment to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Create a test file first
        let test_file = temp_dir.path().join("test_debounce.rs");
        tokio::fs::write(&test_file, "fn test() {}").await.unwrap();

        // Wait for the create event and discard it
        let _ = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("Timeout waiting for create event");

        // Rapidly modify the same file 3 times
        for i in 0..3 {
            tokio::fs::write(&test_file, format!("fn test() {{ println!(\"{}\"); }}", i)).await.unwrap();
            // Small delay between writes (less than debounce threshold of 100ms)
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        // Collect events for a short period
        let mut events = Vec::new();
        let start = std::time::Instant::now();

        while start.elapsed() < Duration::from_secs(1) {
            match timeout(Duration::from_millis(100), rx.recv()).await {
                Ok(Some(event)) => {
                    if matches!(event, WatchEvent::Modified(_)) {
                        events.push(event);
                    }
                }
                _ => break,
            }
        }

        // Due to debouncing (100ms threshold), we should receive fewer than 3 events
        // The exact number depends on timing, but it should be less than 3
        assert!(events.len() < 3, "Expected fewer than 3 events due to debouncing, got {}", events.len());

        // Verify the last event represents the final state
        if let Some(last_event) = events.last() {
            if let WatchEvent::Modified(path) = last_event {
                assert_eq!(path, &test_file);
            }
        }
    }
}

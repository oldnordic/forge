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
///
/// # Examples
///
/// ```no_run
/// use forge_core::watcher::Watcher;
/// use std::path::PathBuf;
/// use tokio::sync::mpsc;
///
/// # #[tokio::main]
/// # async fn main() -> anyhow::Result<()> {
/// let (tx, mut rx) = mpsc::unbounded_channel();
/// let path = PathBuf::from("./src");
///
/// let watcher = Watcher::new(store, tx);
/// watcher.start(path).await?;
///
/// // Process events
/// while let Some(event) = rx.recv().await {
///     println!("Event: {:?}", event);
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct Watcher {
    /// The graph store (for future use in event correlation).
    store: Arc<UnifiedGraphStore>,
    /// Channel sender for watch events.
    sender: mpsc::UnboundedSender<WatchEvent>,
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
        Self { store, sender }
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

        // Create watcher
        RecommendedWatcher::new(
            event_handler,
            notify::Config::default(),
        )?.watch(&path, RecursiveMode::Recursive)?;

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
}

//! ForgeKit runtime layer - File watching, caching, and metrics.
//!
//! This crate provides runtime services for the ForgeKit SDK:
//!
//! - File watching with `notify` crate
//! - Incremental indexing via magellan
//! - Query caching with LRU eviction
//! - Connection pooling and metrics
//!
//! # Examples
//!
//! ```rust,no_run
//! use forge_runtime::{ForgeRuntime, RuntimeConfig};
//!
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! let config = RuntimeConfig {
//!     watch_enabled: true,
//!     debounce_ms: 500,
//!     cache_size: 1000,
//!     cache_ttl_secs: 300,
//!     ..Default::default()
//! };
//!
//! let runtime = ForgeRuntime::new("./my-project").await?;
//!
//! // Start file watching with automatic re-indexing
//! runtime.watch().await?;
//!
//! // Runtime now manages caching and metrics
//! # Ok(())
//! # }
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context as _;
use tokio::sync::Mutex;

// Re-export forge_core types
pub use forge_core::{Watcher, WatchEvent, IncrementalIndexer, PathFilter, QueryCache, FlushStats};

pub mod metrics;
pub use metrics::{RuntimeMetrics, MetricKind, MetricsSummary};

/// Runtime configuration for indexing and caching.
#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    /// Enable file watching
    pub watch_enabled: bool,
    /// Debounce delay for file events (milliseconds)
    pub debounce_ms: u64,
    /// Maximum cache size
    pub cache_size: usize,
    /// Cache TTL (seconds)
    pub cache_ttl_secs: u64,
    /// Directory to watch (default: "src/")
    pub watch_dir: String,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            watch_enabled: false,
            debounce_ms: 500,
            cache_size: 10_000,
            cache_ttl_secs: 300,
            watch_dir: "src".to_string(),
        }
    }
}

/// Runtime statistics snapshot.
#[derive(Clone, Debug)]
pub struct RuntimeStats {
    /// Current number of cached entries
    pub cache_size: usize,
    /// Whether file watcher is active
    pub watch_active: bool,
    /// Number of reindex operations performed
    pub reindex_count: u64,
    /// Metrics summary
    pub metrics: MetricsSummary,
}

/// ForgeKit runtime for automatic reindexing and caching.
///
/// The `ForgeRuntime` integrates file watching, incremental indexing,
/// query caching, and metrics collection into a single API.
pub struct ForgeRuntime {
    /// Path to the codebase
    codebase_path: PathBuf,
    /// Runtime configuration
    config: RuntimeConfig,
    /// Graph store for indexing
    store: Option<Arc<forge_core::UnifiedGraphStore>>,
    /// File watcher
    watcher: Option<Watcher>,
    /// Incremental indexer
    indexer: Option<IncrementalIndexer>,
    /// Query cache
    cache: Option<QueryCache<String, String>>,
    /// Runtime metrics
    metrics: RuntimeMetrics,
    /// Watch task handle (for cleanup)
    watch_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// Whether watching is active
    watch_active: Arc<std::sync::atomic::AtomicBool>,
}

impl ForgeRuntime {
    /// Creates a new runtime with default configuration.
    ///
    /// # Arguments
    ///
    /// * `codebase_path` - Path to codebase
    pub async fn new(codebase_path: impl AsRef<Path>) -> anyhow::Result<Self> {
        Self::with_config(codebase_path, RuntimeConfig::default()).await
    }

    /// Creates a new runtime with custom configuration.
    ///
    /// # Arguments
    ///
    /// * `codebase_path` - Path to codebase
    /// * `config` - Runtime configuration
    pub async fn with_config(
        codebase_path: impl AsRef<Path>,
        config: RuntimeConfig,
    ) -> anyhow::Result<Self> {
        let codebase_path = codebase_path.as_ref().canonicalize()
            .context("Failed to canonicalize codebase path")?;

        // Initialize the graph store
        let store = Arc::new(
            forge_core::UnifiedGraphStore::open(&codebase_path, forge_core::BackendKind::default())
                .await
                .context("Failed to open graph store")?
        );

        // Create indexer with path filter
        let filter = PathFilter::include_dirs(&[&config.watch_dir]);
        let indexer = IncrementalIndexer::with_filter(store.clone(), filter);

        // Create query cache
        let cache = QueryCache::new(
            config.cache_size,
            Duration::from_secs(config.cache_ttl_secs),
        );

        Ok(Self {
            codebase_path,
            config,
            store: Some(store),
            watcher: None,
            indexer: Some(indexer),
            cache: Some(cache),
            metrics: RuntimeMetrics::new(),
            watch_handle: Arc::new(Mutex::new(None)),
            watch_active: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
    }

    /// Starts the file watcher for automatic reindexing.
    ///
    /// This will monitor the codebase for changes and trigger
    /// reindexing as needed. Events are debounced to avoid
    /// excessive reindexing.
    pub async fn watch(&mut self) -> anyhow::Result<()> {
        if !self.config.watch_enabled {
            return Err(anyhow::anyhow!("File watching is not enabled in config"));
        }

        if self.watch_active.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(anyhow::anyhow!("File watching is already active"));
        }

        let store = self.store.clone().ok_or_else(|| anyhow::anyhow!("Store not initialized"))?;
        let indexer = self.indexer.clone().ok_or_else(|| anyhow::anyhow!("Indexer not initialized"))?;

        // Create watcher channel
        let (tx, rx) = Watcher::channel();
        let watcher = Watcher::new(store, tx);
        self.watcher = Some(watcher);

        // Start watching the configured directory
        let watch_path = self.codebase_path.join(&self.config.watch_dir);

        // Verify directory exists before watching
        if !watch_path.exists() {
            return Err(anyhow::anyhow!("Watch directory does not exist: {}", watch_path.display()));
        }

        if let Some(watcher) = &self.watcher {
            watcher.start(watch_path.clone()).await
                .context("Failed to start file watcher")?;
        }

        // Spawn background task to handle events
        let metrics = self.metrics.clone();
        let indexer_clone = indexer.clone();
        let watch_active = self.watch_active.clone();
        let debounce = Duration::from_millis(self.config.debounce_ms);

        let handle = tokio::spawn(async move {
            watch_active.store(true, std::sync::atomic::Ordering::Relaxed);

            let mut rx = rx;
            let mut last_flush = std::time::Instant::now();

            loop {
                let is_running = watch_active.load(std::sync::atomic::Ordering::Relaxed);
                if !is_running {
                    break;
                }

                match tokio::time::timeout(Duration::from_secs(1), rx.recv()).await {
                    Ok(Some(event)) => {
                        // Queue the event for processing
                        indexer_clone.queue(event);
                    }
                    Ok(None) => {
                        // Channel closed
                        break;
                    }
                    Err(_) => {
                        // Timeout - continue loop to check watch_active
                    }
                }

                // Flush on debounce interval
                if last_flush.elapsed() >= debounce {
                    if let Ok(_stats) = indexer_clone.flush().await {
                        metrics.record(MetricKind::Reindex);
                    }
                    last_flush = std::time::Instant::now();
                }
            }

            // Final flush on shutdown
            let _ = indexer_clone.flush().await;
        });

        *self.watch_handle.lock().await = Some(handle);

        Ok(())
    }

    /// Stops the file watcher.
    pub async fn stop_watching(&mut self) -> anyhow::Result<()> {
        self.watch_active.store(false, std::sync::atomic::Ordering::Relaxed);

        if let Some(handle) = self.watch_handle.lock().await.take() {
            handle.await.ok();
        }

        Ok(())
    }

    /// Gets a reference to the query cache.
    pub fn cache(&self) -> Option<&QueryCache<String, String>> {
        self.cache.as_ref()
    }

    /// Gets a reference to the metrics collector.
    pub fn metrics(&self) -> &RuntimeMetrics {
        &self.metrics
    }

    /// Clears all caches.
    pub async fn clear_cache(&self) -> anyhow::Result<()> {
        if let Some(cache) = &self.cache {
            cache.clear().await;
        }
        Ok(())
    }

    /// Gets runtime statistics.
    pub fn stats(&self) -> RuntimeStats {
        RuntimeStats {
            cache_size: self.cache.as_ref().map(|c| futures::executor::block_on(c.len())).unwrap_or(0),
            watch_active: self.watch_active.load(std::sync::atomic::Ordering::Relaxed),
            reindex_count: self.metrics.count(MetricKind::Reindex),
            metrics: self.metrics.summary(),
        }
    }

    /// Gets the codebase path.
    pub fn codebase_path(&self) -> &Path {
        &self.codebase_path
    }

    /// Gets the configuration.
    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }
}

impl Drop for ForgeRuntime {
    fn drop(&mut self) {
        // Signal shutdown
        self.watch_active.store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_runtime_config_default() {
        let config = RuntimeConfig::default();
        assert_eq!(config.debounce_ms, 500);
        assert_eq!(config.cache_size, 10_000);
        assert_eq!(config.cache_ttl_secs, 300);
    }

    #[tokio::test]
    async fn test_runtime_creation() {
        let temp = tempfile::tempdir().unwrap();
        let config = RuntimeConfig::default();
        let runtime = ForgeRuntime::with_config(temp.path(), config).await.unwrap();

        let stats = runtime.stats();
        assert_eq!(stats.cache_size, 0);
        assert!(!stats.watch_active);
    }

    #[tokio::test]
    async fn test_runtime_with_custom_config() {
        let temp = tempfile::tempdir().unwrap();
        let config = RuntimeConfig {
            watch_enabled: false,
            debounce_ms: 1000,
            cache_size: 100,
            cache_ttl_secs: 600,
            watch_dir: "src".to_string(),
        };

        let runtime = ForgeRuntime::with_config(temp.path(), config).await.unwrap();

        assert_eq!(runtime.config().debounce_ms, 1000);
        assert_eq!(runtime.config().cache_size, 100);
    }

    #[tokio::test]
    async fn test_runtime_cache_operations() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = ForgeRuntime::new(temp.path()).await.unwrap();

        let cache = runtime.cache().expect("Cache should be initialized");

        cache.insert("key1".to_string(), "value1".to_string()).await;
        let value = cache.get(&"key1".to_string()).await;

        assert_eq!(value, Some("value1".to_string()));
    }

    #[tokio::test]
    async fn test_runtime_metrics() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = ForgeRuntime::new(temp.path()).await.unwrap();

        runtime.metrics().record(MetricKind::GraphQuery);
        runtime.metrics().record(MetricKind::GraphQuery);

        assert_eq!(runtime.metrics().count(MetricKind::GraphQuery), 2);
    }

    #[tokio::test]
    async fn test_runtime_clear_cache() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = ForgeRuntime::new(temp.path()).await.unwrap();

        let cache = runtime.cache().expect("Cache should be initialized");
        cache.insert("key1".to_string(), "value1".to_string()).await;

        runtime.clear_cache().await.unwrap();

        assert!(cache.is_empty().await);
    }

    #[tokio::test]
    async fn test_runtime_stats() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = ForgeRuntime::new(temp.path()).await.unwrap();

        runtime.metrics().record(MetricKind::GraphQuery);
        runtime.metrics().record_cache_access(true);

        let stats = runtime.stats();

        assert_eq!(stats.metrics.graph_queries, 1);
        assert_eq!(stats.metrics.cache_hit_rate, 1.0);
    }

    #[tokio::test]
    async fn test_runtime_watch_fails_when_disabled() {
        let temp = tempfile::tempdir().unwrap();
        let config = RuntimeConfig {
            watch_enabled: false,
            ..Default::default()
        };

        let mut runtime = ForgeRuntime::with_config(temp.path(), config).await.unwrap();

        assert!(runtime.watch().await.is_err());
    }

    #[tokio::test]
    async fn test_runtime_watch_fails_for_nonexistent_dir() {
        let temp = tempfile::tempdir().unwrap();
        let config = RuntimeConfig {
            watch_enabled: true,
            watch_dir: "nonexistent".to_string(),
            ..Default::default()
        };

        let mut runtime = ForgeRuntime::with_config(temp.path(), config).await.unwrap();

        assert!(runtime.watch().await.is_err());
    }
}

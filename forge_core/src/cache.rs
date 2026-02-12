//! Query caching layer with LRU eviction and TTL expiration.
//!
//! This module provides a thread-safe cache for query results to reduce
//! database load and improve response times.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Cache entry with expiration time.
#[derive(Debug, Clone)]
struct CacheEntry<V> {
    /// The cached value.
    value: V,
    /// When this entry expires.
    expires_at: Instant,
}

/// Thread-safe query cache with LRU eviction.
///
/// The `QueryCache` stores query results with a TTL (time-to-live)
/// and evicts oldest entries when the cache is full.
///
/// # Examples
///
/// ```no_run
/// use forge_core::cache::QueryCache;
/// use std::time::Duration;
///
/// # #[tokio::main]
/// # async fn main() -> anyhow::Result<()> {
/// let cache = QueryCache::<String, String>::new(100, Duration::from_secs(300));
///
/// // Insert a value
/// cache.insert("key".to_string(), "value".to_string()).await;
///
/// // Retrieve it
/// if let Some(value) = cache.get(&"key".to_string()).await {
///     println!("Cached: {}", value);
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct QueryCache<K, V>
where
    K: Clone + Eq + std::hash::Hash + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// Maximum number of entries.
    max_size: usize,
    /// Time-to-live for cache entries.
    ttl: Duration,
    /// The underlying cache store.
    inner: Arc<RwLock<CacheInner<K, V>>>,
}

/// Internal cache storage.
struct CacheInner<K, V>
where
    K: Clone + Eq + std::hash::Hash,
    V: Clone,
{
    /// Map from key to cached entry.
    entries: HashMap<K, CacheEntry<V>>,
    /// Keys in insertion order (for FIFO eviction).
    keys: Vec<K>,
}

impl<K, V> QueryCache<K, V>
where
    K: Clone + Eq + std::hash::Hash + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// Creates a new query cache.
    ///
    /// # Arguments
    ///
    /// * `max_size` - Maximum number of entries before eviction
    /// * `ttl` - Time-to-live for cache entries
    pub fn new(max_size: usize, ttl: Duration) -> Self {
        Self {
            max_size,
            ttl,
            inner: Arc::new(RwLock::new(CacheInner {
                entries: HashMap::new(),
                keys: Vec::new(),
            })),
        }
    }

    /// Gets a cached value if it exists and hasn't expired.
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key
    ///
    /// # Returns
    ///
    /// `Some(value)` if cached and valid, `None` otherwise.
    pub async fn get(&self, key: &K) -> Option<V> {
        let mut inner = self.inner.write().await;
        let now = Instant::now();

        // Clone key to avoid borrow issues
        let key_clone = key.clone();
        let value_opt = inner.entries.get(&key_clone).cloned();

        if let Some(entry) = value_opt {
            if now < entry.expires_at {
                // Touch key: move to end of list (LRU behavior)
                if let Some(pos) = inner.keys.iter().position(|k| k == &key_clone) {
                    inner.keys.remove(pos);
                    inner.keys.push(key_clone);
                }
                return Some(entry.value);
            } else {
                // Expired - remove it
                inner.entries.remove(&key_clone);
                if let Some(pos) = inner.keys.iter().position(|k| k == &key_clone) {
                    inner.keys.remove(pos);
                }
            }
        }
        None
    }

    /// Inserts a value into the cache.
    ///
    /// If the cache is full, the oldest entry is evicted (FIFO).
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key
    /// * `value` - The value to cache
    pub async fn insert(&self, key: K, value: V) {
        let mut inner = self.inner.write().await;

        // Check if we need to evict
        while inner.keys.len() >= self.max_size && !inner.keys.is_empty() {
            // Evict oldest (FIFO) - or first key
            if let Some(old_key) = inner.keys.first() {
                let old_key = old_key.clone();
                inner.keys.remove(0);
                inner.entries.remove(&old_key);
            }
        }

        let expires_at = Instant::now() + self.ttl;

        // Update or insert
        if !inner.keys.contains(&key) {
            inner.keys.push(key.clone());
        }
        inner.entries.insert(key, CacheEntry { value, expires_at });
    }

    /// Invalidates a specific cache entry.
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key to invalidate
    pub async fn invalidate(&self, key: &K) {
        let mut inner = self.inner.write().await;
        inner.entries.remove(key);
        if let Some(pos) = inner.keys.iter().position(|k| k == key) {
            inner.keys.remove(pos);
        }
    }

    /// Clears all cached entries.
    pub async fn clear(&self) {
        let mut inner = self.inner.write().await;
        inner.entries.clear();
        inner.keys.clear();
    }

    /// Returns the current number of cached entries.
    pub async fn len(&self) -> usize {
        let inner = self.inner.read().await;
        inner.entries.len()
    }

    /// Returns true if the cache is empty.
    pub async fn is_empty(&self) -> bool {
        let inner = self.inner.read().await;
        inner.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_cache_insert_get() {
        let cache = QueryCache::new(10, Duration::from_secs(60));

        cache.insert("key1".to_string(), "value1".to_string()).await;
        let value = cache.get(&"key1".to_string()).await;

        assert_eq!(value, Some("value1".to_string()));
    }

    #[tokio::test]
    async fn test_cache_miss() {
        let cache = QueryCache::new(10, Duration::from_secs(60));

        let value: Option<String> = cache.get(&"nonexistent".to_string()).await;
        assert!(value.is_none());
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        let cache = QueryCache::new(10, Duration::from_millis(50));

        cache.insert("key".to_string(), "value".to_string()).await;

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(100)).await;

        let value = cache.get(&"key".to_string()).await;
        assert!(value.is_none());
    }

    #[tokio::test]
    async fn test_cache_eviction() {
        let cache = QueryCache::new(2, Duration::from_secs(60));

        cache.insert("key1".to_string(), "value1".to_string()).await;
        cache.insert("key2".to_string(), "value2".to_string()).await;
        cache.insert("key3".to_string(), "value3".to_string()).await;

        // key1 should be evicted (FIFO)
        assert_eq!(cache.len().await, 2);
        assert!(cache.get(&"key1".to_string()).await.is_none());
        assert_eq!(cache.get(&"key2".to_string()).await, Some("value2".to_string()));
        assert_eq!(cache.get(&"key3".to_string()).await, Some("value3".to_string()));
    }

    #[tokio::test]
    async fn test_cache_invalidate() {
        let cache = QueryCache::new(10, Duration::from_secs(60));

        cache.insert("key".to_string(), "value".to_string()).await;
        cache.invalidate(&"key".to_string()).await;

        assert!(cache.get(&"key".to_string()).await.is_none());
        assert_eq!(cache.len().await, 0);
    }

    #[tokio::test]
    async fn test_cache_clear() {
        let cache = QueryCache::new(10, Duration::from_secs(60));

        cache.insert("key1".to_string(), "value1".to_string()).await;
        cache.insert("key2".to_string(), "value2".to_string()).await;
        cache.clear().await;

        assert!(cache.is_empty().await);
    }
}

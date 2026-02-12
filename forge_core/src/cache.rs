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
#[derive(Debug)]
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

        // If max_size is 0, don't insert anything
        if self.max_size == 0 {
            return;
        }

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

    #[tokio::test]
    async fn test_cache_lru_touch() {
        let cache = QueryCache::new(3, Duration::from_secs(60));

        // Insert items 1, 2, 3
        cache.insert("key1".to_string(), "value1".to_string()).await;
        cache.insert("key2".to_string(), "value2".to_string()).await;
        cache.insert("key3".to_string(), "value3".to_string()).await;

        // Access item 1 (moves to end)
        let _ = cache.get(&"key1".to_string()).await;

        // Insert item 4 (causes eviction of oldest, which should be key2)
        cache.insert("key4".to_string(), "value4".to_string()).await;

        // Verify key2 is evicted (not key1 which was touched)
        assert_eq!(cache.len().await, 3);
        assert!(cache.get(&"key2".to_string()).await.is_none());
        assert_eq!(cache.get(&"key1".to_string()).await, Some("value1".to_string()));
        assert_eq!(cache.get(&"key3".to_string()).await, Some("value3".to_string()));
        assert_eq!(cache.get(&"key4".to_string()).await, Some("value4".to_string()));
    }

    #[tokio::test]
    async fn test_cache_update_existing() {
        let cache = QueryCache::new(10, Duration::from_millis(100));

        // Insert key with value A
        cache.insert("key".to_string(), "valueA".to_string()).await;

        // Wait partial TTL
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Insert same key with value B (refreshes TTL)
        cache.insert("key".to_string(), "valueB".to_string()).await;

        // Immediately get key - should return B with fresh TTL
        assert_eq!(cache.get(&"key".to_string()).await, Some("valueB".to_string()));

        // Wait for original TTL to pass (100ms total)
        tokio::time::sleep(Duration::from_millis(60)).await;

        // Should still be valid because update refreshed TTL
        assert_eq!(cache.get(&"key".to_string()).await, Some("valueB".to_string()));
    }

    #[tokio::test]
    async fn test_cache_concurrent_access() {
        let cache = QueryCache::new(20, Duration::from_secs(60));
        let mut handles = vec![];

        // Spawn 10 tasks concurrently inserting different keys
        for i in 0..10 {
            let cache_clone = cache.clone();
            handles.push(tokio::spawn(async move {
                cache_clone.insert(format!("key{}", i), format!("value{}", i)).await;
            }));
        }

        // Wait for all to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Verify all 10 items are in cache
        assert_eq!(cache.len().await, 10);
        for i in 0..10 {
            assert_eq!(
                cache.get(&format!("key{}", i)).await,
                Some(format!("value{}", i))
            );
        }
    }

    #[tokio::test]
    async fn test_cache_stress_eviction() {
        let cache = QueryCache::new(5, Duration::from_secs(60));

        // Insert 100 items sequentially
        for i in 0..100 {
            cache.insert(format!("key{}", i), format!("value{}", i)).await;
        }

        // Verify only 5 items remain
        assert_eq!(cache.len().await, 5);

        // Verify remaining are the last 5 inserted
        assert!(cache.get(&"key0".to_string()).await.is_none());
        assert!(cache.get(&"key94".to_string()).await.is_none());
        assert_eq!(cache.get(&"key95".to_string()).await, Some("value95".to_string()));
        assert_eq!(cache.get(&"key96".to_string()).await, Some("value96".to_string()));
        assert_eq!(cache.get(&"key97".to_string()).await, Some("value97".to_string()));
        assert_eq!(cache.get(&"key98".to_string()).await, Some("value98".to_string()));
        assert_eq!(cache.get(&"key99".to_string()).await, Some("value99".to_string()));
    }

    #[tokio::test]
    async fn test_cache_zero_max_size() {
        let cache = QueryCache::new(0, Duration::from_secs(60));

        // Insert should not add entries
        cache.insert("key1".to_string(), "value1".to_string()).await;
        cache.insert("key2".to_string(), "value2".to_string()).await;

        // Verify len() always returns 0
        assert_eq!(cache.len().await, 0);
        assert!(cache.is_empty().await);
        assert!(cache.get(&"key1".to_string()).await.is_none());
        assert!(cache.get(&"key2".to_string()).await.is_none());
    }

    #[tokio::test]
    async fn test_cache_ttl_expiration() {
        let cache = QueryCache::new(10, Duration::from_millis(100));

        // Insert key
        cache.insert("key".to_string(), "value".to_string()).await;
        assert_eq!(cache.len().await, 1);

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Verify get returns None (expired)
        assert!(cache.get(&"key".to_string()).await.is_none());

        // Verify len decreased
        assert_eq!(cache.len().await, 0);
    }
}

impl<K, V> std::fmt::Debug for QueryCache<K, V>
where
    K: Clone + Eq + std::hash::Hash + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueryCache")
            .field("max_size", &self.max_size)
            .field("ttl", &self.ttl)
            .field("inner", &"<cache>")
            .finish()
    }
}

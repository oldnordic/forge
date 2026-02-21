//! Integration test for core runtime modules (Watcher, Indexing, Cache)

use forge_core::{Watcher, WatchEvent, PathFilter, QueryCache};
use std::time::Duration;
use std::path::PathBuf;

#[tokio::test]
async fn test_watcher_module_accessible() {
    // Verify Watcher types are accessible
    let (tx, _rx) = Watcher::channel();
    let event = WatchEvent::Created(PathBuf::from("test.rs"));

    // Verify we can send events
    tx.send(event).expect("Should be able to send event");
}

#[tokio::test]
async fn test_indexing_module_accessible() {
    // Verify Indexing types are accessible
    let filter = PathFilter::default();
    assert!(filter.should_index(std::path::Path::new("src/lib.rs")));
    assert!(!filter.should_index(std::path::Path::new("target/debug.rs")));
}

#[tokio::test]
async fn test_cache_module_accessible() {
    // Verify Cache types are accessible
    let cache = QueryCache::new(10, Duration::from_secs(60));

    cache.insert("key".to_string(), "value".to_string()).await;
    let result = cache.get(&"key".to_string()).await;

    assert_eq!(result, Some("value".to_string()));
}

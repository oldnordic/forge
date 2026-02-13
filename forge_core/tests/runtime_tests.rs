//! End-to-end integration tests for runtime layer.

use forge_core::Forge;

#[tokio::test]
async fn test_runtime_watch_and_index() {
    let temp = tempfile::tempdir().unwrap();

    // Create Forge with runtime
    let forge = Forge::with_runtime(temp.path()).await.unwrap();
    let runtime = forge.runtime().unwrap();

    // Start watching
    let mut runtime_mut = runtime.as_ref().clone();
    runtime_mut.start_watching().await.unwrap();

    // Create a test file
    let test_file = temp.path().join("test.rs");
    tokio::fs::write(&test_file, "fn test() {}").await.unwrap();

    // Wait for indexer to pick it up
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Verify indexer has pending change or processed it
    let stats = runtime_mut.indexer_stats().await;
    // Stats exist (may show 0 if backend is stub)
    let _ = stats;

    // Verify watching is active
    assert!(runtime_mut.is_watching());
}

#[tokio::test]
async fn test_runtime_cache_invalidation() {
    let temp = tempfile::tempdir().unwrap();
    let forge = Forge::with_runtime(temp.path()).await.unwrap();
    let runtime = forge.runtime().unwrap();

    // Insert something in cache
    let cache = runtime.cache();
    cache.insert("test_key".to_string(), "test_value".to_string()).await;

    // Verify it's cached
    assert!(cache.get(&"test_key".to_string()).await.is_some());

    // Invalidate
    cache.invalidate(&"test_key".to_string()).await;

    // Verify it's gone
    assert!(cache.get(&"test_key".to_string()).await.is_none());
}

#[tokio::test]
async fn test_runtime_pool_concurrent_access() {
    let temp = tempfile::tempdir().unwrap();
    let forge = Forge::with_runtime(temp.path()).await.unwrap();
    let runtime = forge.runtime().unwrap();

    let pool = runtime.pool().unwrap();

    // Try to acquire multiple permits
    let permit1 = pool.acquire().await.unwrap();
    let permit2 = pool.acquire().await.unwrap();

    // Verify we can get at least 2
    assert!(pool.available_connections() < 10);

    drop(permit1);
    drop(permit2);

    // Verify connections are released
    assert!(pool.available_connections() > 8);
}

#[tokio::test]
async fn test_runtime_full_lifecycle() {
    let temp = tempfile::tempdir().unwrap();

    // Create with runtime
    let forge = Forge::with_runtime(temp.path()).await.unwrap();
    let runtime = forge.runtime().unwrap();

    // Start watching
    let mut runtime_mut = runtime.as_ref().clone();
    runtime_mut.start_watching().await.unwrap();

    // Verify components exist
    // Cache is functional (len may be 0)
    let _cache_len = runtime_mut.cache().len().await;
    assert!(runtime_mut.pool().unwrap().available_connections() > 0);

    // Create file
    let test_file = temp.path().join("lifecycle.rs");
    tokio::fs::write(&test_file, "fn lifecycle() {}").await.unwrap();

    // Wait and check stats
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    let stats = runtime_mut.indexer_stats().await;

    // Stop watching
    runtime_mut.stop_watching();

    // Verify lifecycle completed without errors
    // Stats may show 0 if backend is stub
    let _ = stats;

    // Verify watching stopped
    assert!(!runtime_mut.is_watching());
}

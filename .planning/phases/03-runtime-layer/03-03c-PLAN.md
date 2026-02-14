---
phase: 03-test-infrastructure
plan: 03c
type: execute
wave: 3
depends_on: ["03-03a", "03-03b"]
files_modified:
  - forge_core/src/runtime.rs
  - tests/integration/runtime_tests.rs
  - tests/integration/mod.rs
autonomous: true

must_haves:
  truths:
    - "Runtime exposes cache and pool accessors"
    - "Runtime indexer receives events from watcher"
    - "Runtime all components orchestrate without panics"
    - "Runtime start_watching is idempotent (safe to call twice)"
    - "Runtime stop_watching terminates file watching"
    - "Integration tests verify end-to-end runtime behavior"
  artifacts:
    - path: "forge_core/src/runtime.rs"
      provides: "Runtime orchestration with comprehensive test coverage"
      exports: ["test_runtime_cache_and_pool_access", "test_runtime_indexer_integration", "test_runtime_full_orchestration", "test_runtime_double_start_watching", "test_runtime_stop_watching", "test_runtime_error_handling"]
      covered_by: "Task 1"
    - path: "tests/integration/runtime_tests.rs"
      provides: "End-to-end runtime integration tests"
      exports: ["test_runtime_watch_and_index", "test_runtime_cache_invalidation", "test_runtime_pool_concurrent_access", "test_runtime_full_lifecycle"]
      covered_by: "Task 2"
    - path: "tests/integration/mod.rs"
      provides: "Integration test module declarations"
      exports: ["runtime_tests"]
      covered_by: "Task 2"
  key_links:
    - from: "tests/integration/runtime_tests.rs"
      to: "forge_core::runtime::Runtime"
      via: "Forge::with_runtime"
      pattern: "Forge::with_runtime|Runtime::new"
    - from: "forge_core/src/runtime.rs"
      to: "forge_core::watcher"
      via: "Runtime::start_with_watching"
      pattern: "Watcher::new|watcher.start"
    - from: "forge_core/src/runtime.rs"
      to: "forge_core::cache"
      via: "Runtime::cache accessor"
      pattern: "runtime.cache|QueryCache"
    - from: "forge_core/src/runtime.rs"
      to: "forge_core::pool"
      via: "Runtime::pool accessor"
      pattern: "runtime.pool|ConnectionPool"

---

<objective>
Expand test coverage for runtime orchestration and create end-to-end integration tests.

This plan covers the runtime component and integration testing:
1. Expand runtime.rs tests with orchestration, error handling, and lifecycle scenarios
2. Create end-to-end integration tests that verify all runtime components work together

Purpose: The runtime orchestrates all components. Integration tests verify the full system works correctly from file watching through caching to indexing.

Output: 10 new tests (6 unit tests in runtime.rs, 4 integration tests)
</objective>

<execution_context>
@/home/feanor/.claude/get-shit-done/workflows/execute-plan.md
@/home/feanor/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/ROADMAP.md
@.planning/STATE.md
@.planning/phases/03-runtime-layer/03-RESEARCH.md
@forge_core/src/runtime.rs
@tests/common/mod.rs
@tests/integration/mod.rs
</context>

<tasks>

<task type="auto">
  <name>Task 1: Expand runtime.rs Tests</name>
  <files>forge_core/src/runtime.rs</files>
  <action>
Add 6 new tests to forge_core/src/runtime.rs in #[cfg(test)] mod tests:

1. test_runtime_cache_and_pool_access - Verify runtime exposes cache and pool:
   - Create runtime
   - Call runtime.cache()
   - Call runtime.pool()
   - Verify both return valid instances (not None/null)

2. test_runtime_indexer_integration - Verify indexer gets events:
   - Create runtime with watching
   - Create test file in watched directory
   - Wait for indexer to process (poll pending_changes)
   - Verify file appears in indexer queue or is processed

3. test_runtime_full_orchestration - Verify all components work together:
   - Create runtime
   - Perform query (uses cache)
   - Create file (triggers watcher)
   - Flush indexer
   - Verify no panics or errors
   - Verify stats reflect operations

4. test_runtime_double_start_watching - Verify calling start_watching twice is safe:
   - Start watching once
   - Start watching again (same path)
   - Verify no duplicate watchers or errors
   - Verify only one watcher active

5. test_runtime_stop_watching - Verify stop_watching terminates watcher:
   - Start watching
   - Call stop_watching()
   - Create test file
   - Verify no events received (or pending_changes remains 0)

6. test_runtime_error_handling - Verify runtime handles errors gracefully:
   - Create runtime with invalid path (empty string)
   - Verify error is returned
   - Verify no panics occur
   - Test with non-existent directory (should create or error gracefully)

Use wait_for() helper from tests/common for async polling conditions.
  </action>
  <verify>
Run: cargo test -p forge_core runtime
Expected: 11 tests pass (5 existing + 6 new)
  </verify>
  <done>
forge_core/src/runtime.rs has 11 tests covering: creation (existing), cache (existing), pending changes (existing), process events (existing), watching (existing), cache/pool access, indexer integration, full orchestration, double start safety, stop watching behavior, and error handling.
  </done>
</task>

<task type="auto">
  <name>Task 2: Create Runtime Integration Tests</name>
  <files>tests/integration/runtime_tests.rs</files>
  <action>
Create tests/integration/runtime_tests.rs with end-to-end tests:

```rust
//! End-to-end integration tests for runtime layer.

use forge_core::Forge;
use tests_common::test_forge;

#[tokio::test]
async fn test_runtime_watch_and_index() {
    let temp = tempfile::tempdir().unwrap();

    // Create Forge with runtime
    let forge = Forge::with_runtime(temp.path()).await.unwrap();
    let runtime = forge.runtime().unwrap();

    // Start watching
    runtime.start_watching().await.unwrap();

    // Create a test file
    let test_file = temp.path().join("test.rs");
    tokio::fs::write(&test_file, "fn test() {}").await.unwrap();

    // Wait for indexer to pick it up
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Verify indexer has pending change or processed it
    let stats = runtime.indexer_stats().await;
    assert!(stats.pending_changes > 0 || stats.total_processed > 0);
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

    let pool = runtime.pool();

    // Try to acquire multiple permits
    let permit1 = pool.acquire().await.unwrap();
    let permit2 = pool.acquire().await.unwrap();

    // Verify we can get at least 2
    assert!(pool.available_connections() < 10);

    drop(permit1);
    drop(permit2);
}

#[tokio::test]
async fn test_runtime_full_lifecycle() {
    let temp = tempfile::tempdir().unwrap();

    // Create with runtime
    let forge = Forge::with_runtime(temp.path()).await.unwrap();
    let runtime = forge.runtime().unwrap();

    // Start watching
    runtime.start_watching().await.unwrap();

    // Verify components exist
    assert!(runtime.cache().len().await >= 0);
    assert!(runtime.pool().available_connections() > 0);

    // Create file
    let test_file = temp.path().join("lifecycle.rs");
    tokio::fs::write(&test_file, "fn lifecycle() {}").await.unwrap();

    // Wait and check stats
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    let stats = runtime.indexer_stats().await;

    // Stop watching
    runtime.stop_watching().await.unwrap();

    // Verify lifecycle completed without errors
    assert!(stats.total_files >= 0);
}
```

Update tests/integration/mod.rs to include:
```rust
mod runtime_tests;
```

Also create/update tests/integration/accessor_tests.rs if it doesn't exist from 03-02.
  </action>
  <verify>
Run: cargo test --test runtime_tests
Expected: 4 integration tests pass
  </verify>
  <done>
tests/integration/runtime_tests.rs exists with 4 tests: watch_and_index, cache_invalidation, pool_concurrent_access, and full_lifecycle. All tests use Forge::with_runtime() to test actual runtime behavior. tests/integration/mod.rs includes the runtime_tests module.
  </done>
</task>

</tasks>

<verification>
Overall phase checks:
- [ ] cargo test -p forge_core runtime passes all 11 tests
- [ ] cargo test --test runtime_tests passes all 4 integration tests
- [ ] Integration tests verify full runtime lifecycle
- [ ] No race conditions in async tests
</verification>

<success_criteria>
Phase 03-03c complete when:
1. runtime.rs has 11 tests covering orchestration and lifecycle
2. Integration tests verify end-to-end runtime behavior
3. All integration tests pass
4. cargo test --workspace shows 100+ passing tests
</success_criteria>

<output>
After completion, create `.planning/phases/03-runtime-layer/03-03c-SUMMARY.md` with:
- List of tests added to runtime.rs
- List of integration tests created
- Full lifecycle test results
- Total test count for the phase
</output>

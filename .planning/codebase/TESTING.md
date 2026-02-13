# Testing Patterns

**Analysis Date:** 2025-02-13
**Version:** 0.1.0

This document defines testing standards and conventions for ForgeKit. Testing is mandatory for all code changes.

---

## Test Framework

### Primary Testing Tools

| Tool | Purpose | Version | Usage |
|------|---------|---------|--------|
| **tokio::test** | Async test runtime | 1.x, full features | `#[tokio::test]` for async functions |
| **Standard assert macros** | Assertions | std | `assert!`, `assert_eq!`, `assert_matches!` |
| **tempfile** | Temporary directories | 3.x | `tempfile::tempdir()` for test isolation |
| **anyhow** | Test error handling | 1.x | `anyhow::Result` for test helpers |

### Test Dependencies

```toml
[dev-dependencies]
tokio = { version = "1", features = ["test-util", "macros"] }
tempfile = "3"
anyhow = "1"
```

### Test Configuration

Profile-level optimization for tests:
```toml
[profile.test]
opt-level = 2
```

---

## Test Organization

### Directory Structure

```
forge/
├── tests/                    # Workspace-level integration tests
│   ├── common/
│   │   └── mod.rs        # Shared test utilities
│   ├── integration/
│   │   └── mod.rs        # Integration test module
│   ├── integration/
│   │   ├── builder_tests.rs
│   │   ├── accessor_tests.rs
│   │   └── runtime_tests.rs
│   └── common/
│       └── mod.rs        # Test fixtures and helpers

forge_core/
├── src/
│   ├── lib.rs           # Contains #[cfg(test)] mod tests
│   ├── types.rs         # Contains #[cfg(test)] mod tests
│   ├── error.rs         # Contains #[cfg(test)] mod tests
│   └── ...              # Each module file has inline tests
└── tests/                # Crate-level integration tests
    ├── builder_tests.rs
    ├── accessor_tests.rs
    └── runtime_tests.rs
```

### Unit Test Placement

Unit tests live in same file as code, in a `#[cfg(test)]` module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_example() {
        // Test implementation
    }
}
```

**Example from:** `forge_core/src/types.rs` (lines 264-693)
- 32 unit tests for types
- Tests for `SymbolId`, `BlockId`, `PathId`, `Location`, `Span`, `SymbolKind`, etc.

### Integration Test Placement

Integration tests go in `tests/` directory:

**Example from:** `tests/integration/builder_tests.rs`
```rust
//! Integration tests for ForgeBuilder pattern.

use forge_core::Forge;

#[tokio::test]
async fn test_builder_default_config() {
    let temp = tempfile::tempdir().unwrap();
    let forge = Forge::open(temp.path()).await.unwrap();

    // Verify default configuration works
    assert!(forge.runtime().is_none());
}
```

**Example from:** `forge_core/tests/accessor_tests.rs`
```rust
//! Integration tests for module accessors.

use forge_core::Forge;

#[tokio::test]
async fn test_all_accessors_work() {
    let temp = tempfile::tempdir().unwrap();
    let forge = Forge::open(temp.path()).await.unwrap();

    // All accessors should return valid instances
    let _graph = forge.graph();
    let _search = forge.search();
    let _cfg = forge.cfg();
    let _edit = forge.edit();
    let _analysis = forge.analysis();
}
```

### Common Test Utilities

**Location:** `tests/common/mod.rs`

Provides helper functions for tests:

```rust
/// Creates a test Forge instance with temporary storage.
pub async fn test_forge() -> anyhow::Result<(TempDir, forge_core::Forge)> {
    let temp = TempDir::new()?;
    let forge = forge_core::Forge::open(temp.path()).await?;
    Ok((temp, forge))
}

/// Creates a test file with given content in specified directory.
pub async fn create_test_file(dir: &Path, name: &str, content: &str) -> anyhow::Result<PathBuf>

/// Creates a test directory structure for a Rust project.
pub async fn create_test_rust_project(dir: &Path, name: &str) -> anyhow::Result<()>

/// Creates a test Symbol with standard test values.
pub fn test_symbol() -> forge_core::Symbol

/// Creates a test Location with standard test values.
pub fn test_location() -> Location

/// Creates a test Span with standard test values.
pub fn test_span() -> Span

/// Assert helper that verifies a Result is Err and contains expected substring.
pub fn assert_error_variant<T>(result: anyhow::Result<T>, expected: &str)

/// Async helper for polling conditions with timeout.
pub async fn wait_for<F>(mut condition: F, timeout_ms: u64) -> anyhow::Result<()>
    where F: FnMut() -> bool
```

**Self-testing:** `tests/common/mod.rs` contains 11 tests for its own utilities

---

## TDD Workflow (Mandatory)

ForgeKit enforces strict Test-Driven Development. See `docs/DEVELOPMENT_WORKFLOW.md` for complete details.

### The 5-Step Workflow

```
1. UNDERSTAND - Read source, check schema
2. PLAN       - Document decision
3. PROVE      - Write failing test, show it fails
4. IMPLEMENT  - Write code to pass test
5. VERIFY     - Show test passes, run cargo check
```

### Step 1: UNDERSTAND

Before writing any code:

```bash
# Check database schema
sqlite3 .forge/graph.db ".schema"

# Read existing source code
Read /home/feanor/Projects/forge/forge_core/src/graph/mod.rs

# Check existing tests
# Read test module to understand patterns
```

### Step 2: PLAN

Document architectural decision with alternatives and trade-offs.

### Step 3: PROVE (Write Failing Test)

```rust
#[tokio::test]
async fn test_find_symbol_returns_exact_location() {
    // Given: A symbol exists in database
    let forge = setup_test_forge().await;
    let expected_id = insert_test_symbol(&forge, "main").await;

    // When: We query for it
    let result = forge.graph()
        .find_symbol("main")
        .await
        .unwrap();

    // Then: Should return symbol with correct location
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, expected_id);
    assert_eq!(result[0].name, "main");
}
```

**Show test fails:**
```bash
$ cargo test test_find_symbol

running 1 test
test test_find_symbol_returns_exact_location ... FAILED
```

### Step 4: IMPLEMENT

Write minimum code to make test pass.

### Step 5: VERIFY

```bash
$ cargo test test_find_symbol

running 1 test
test test_find_symbol_returns_exact_location ... ok

test result: ok. 1 passed; 0 failed
```

Then run `cargo check` to ensure compilation.

---

## Test Categories

### Unit Tests

**Scope:** Test individual functions and methods in isolation.

**Conventions:**
- Located in `#[cfg(test)]` modules within source files
- Test public API and private behavior
- Use mocks/stubs for external dependencies
- Focus: Single function or method

**Example from:** `forge_core/src/error.rs` (lines 72-95)
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = ForgeError::SymbolNotFound("test".to_string());
        assert_eq!(err.to_string(), "Symbol not found: test");
    }

    #[test]
    fn test_span_is_empty() {
        let span = Span { start: 10, end: 10 };
        assert!(span.is_empty());
    }

    #[test]
    fn test_span_contains() {
        let span = Span { start: 10, end: 20 };
        assert!(span.contains(15));
        assert!(!span.contains(20));
        assert!(!span.contains(5));
    }
}
```

**Example from:** `forge_core/src/types.rs` (lines 264-693)
```rust
#[cfg(test)]
mod tests {
    use super::*;

    // SymbolId Tests (4 tests)

    #[test]
    fn test_symbol_id_display() {
        let id = SymbolId(42);
        assert_eq!(id.to_string(), "42");
    }

    #[test]
    fn test_symbol_id_from_i64() {
        let id: SymbolId = 123.into();
        assert_eq!(id.0, 123);
    }

    #[test]
    fn test_symbol_id_ord() {
        let id1 = SymbolId(10);
        let id2 = SymbolId(20);
        assert!(id1 < id2);
        assert!(id2 > id1);
    }

    #[test]
    fn test_symbol_id_zero() {
        let id = SymbolId(0);
        assert_eq!(id.to_string(), "0");
    }

    // BlockId Tests (3 tests)
    // ...
}
```

### Integration Tests

**Scope:** Test cross-module interactions.

**Conventions:**
- Located in `tests/` directory
- Test real interactions between modules
- Use real filesystem when appropriate
- Focus: Multiple components working together

**Example from:** `tests/integration/builder_tests.rs`
```rust
#[tokio::test]
async fn test_forge_creates_database_file() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join(".forge").join("graph.db");

    // Verify database doesn't exist initially
    assert!(!db_path.exists());

    // Create Forge
    let forge = Forge::open(temp.path()).await.unwrap();

    // Verify database was created
    assert!(db_path.exists());
    drop(forge);
}
```

**Example from:** `tests/integration/accessor_tests.rs`
```rust
#[tokio::test]
async fn test_accessor_returns_different_instances() {
    let temp = tempfile::tempdir().unwrap();
    let forge = Forge::open(temp.path()).await.unwrap();

    // Accessors should return new instances each call
    let g1 = forge.graph();
    let g2 = forge.graph();
    // They are equal values but different references (can't directly test this)
    // but we can verify both work
    drop(g1);
    drop(g2);
}
```

### Module-Specific Tests

#### Cache Tests (`forge_core/src/cache.rs`)

**Lines:** 198-399 (201 lines of tests)

Test coverage for `QueryCache`:
- Insert and get
- Cache miss scenarios
- Expiration (TTL)
- Eviction (FIFO)
- Invalidation
- Clear operations
- LRU behavior (touch updates)
- Concurrent access
- Stress testing

**Example test:**
```rust
#[tokio::test]
async fn test_cache_insert_get() {
    let cache = QueryCache::new(10, Duration::from_secs(60));

    cache.insert("key1".to_string(), "value1".to_string()).await;
    let value = cache.get(&"key1".to_string()).await;

    assert_eq!(value, Some("value1".to_string()));
}
```

**Example test:**
```rust
#[tokio::test]
async fn test_cache_expiration() {
    let cache = QueryCache::new(10, Duration::from_millis(50));

    cache.insert("key".to_string(), "value".to_string()).await;

    // Wait for expiration
    tokio::time::sleep(Duration::from_millis(100)).await;

    let value = cache.get(&"key".to_string()).await;
    assert!(value.is_none());
}
```

#### Connection Pool Tests (`forge_core/src/pool.rs`)

**Lines:** 179-372 (193 lines of tests)

Test coverage for `ConnectionPool`:
- Pool creation
- Permit acquisition
- Try-acquire (non-blocking)
- Concurrent access (with barriers)
- Timeout behavior
- Permit drop and return
- Stress testing

**Example test:**
```rust
#[tokio::test]
async fn test_pool_acquire() {
    let pool = ConnectionPool::new("/tmp/test.db", 2);

    let permit1 = pool.acquire().await.unwrap();
    assert_eq!(pool.available_connections(), 1);

    let permit2 = pool.acquire().await.unwrap();
    assert_eq!(pool.available_connections(), 0);

    // Dropping permit returns it to pool
    drop(permit1);
    assert_eq!(pool.available_connections(), 1);
}
```

**Example concurrent test:**
```rust
#[tokio::test]
async fn test_pool_concurrent_acquires() {
    use tokio::sync::Barrier;

    let pool = Arc::new(ConnectionPool::new("/tmp/test.db", 5));
    let barrier = Arc::new(Barrier::new(10));
    let mut handles = vec![];

    // Spawn 10 tasks trying to acquire
    for _i in 0..10 {
        let pool_clone = pool.clone();
        let barrier_clone = barrier.clone();
        handles.push(tokio::spawn(async move {
            barrier_clone.wait().await; // Coordinate start
            let _permit = pool_clone.acquire().await.unwrap();
            // Hold permit briefly
            tokio::time::sleep(Duration::from_millis(50)).await;
            // Permit releases here when dropped
        }));
    }

    // Wait for all to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // All 10 should have completed eventually
    assert_eq!(pool.available_connections(), 5);
}
```

#### Watcher Tests (`forge_core/src/watcher.rs`)

**Lines:** 140-436 (296 lines of tests)

Test coverage for `Watcher`:
- Watcher creation
- Channel communication
- Watch event equality
- File creation events
- File modification events
- File deletion events
- Recursive watching
- Multiple events sequence
- Debounce behavior

**Example test:**
```rust
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
```

#### Indexer Tests (`forge_core/src/indexing.rs`)

**Lines:** 197-401 (204 lines of tests)

Test coverage for `IncrementalIndexer`:
- Indexer creation
- Event queueing
- Flush operations
- Statistics tracking
- Clear pending
- Duplicate handling
- Concurrent operations

**Example test:**
```rust
#[tokio::test]
async fn test_indexer_flush_clears_pending() {
    let store = Arc::new(UnifiedGraphStore::memory().await.unwrap());
    let indexer = IncrementalIndexer::new(store);

    indexer.queue(WatchEvent::Modified(PathBuf::from("src/lib.rs")));
    tokio::time::sleep(Duration::from_millis(50)).await;

    let stats = indexer.flush().await.unwrap();
    assert_eq!(stats.indexed, 1);
    assert_eq!(indexer.pending_count().await, 0);
}
```

#### Runtime Tests (`forge_core/src/runtime.rs`)

**Lines:** 179-395 (216 lines of tests)

Test coverage for `Runtime`:
- Runtime creation
- Cache operations
- Pending changes
- Watcher lifecycle
- Event processing
- Full orchestration
- Error handling

**Example test:**
```rust
#[tokio::test]
async fn test_runtime_creation() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().to_path_buf();

    let runtime = Runtime::new(path).await.unwrap();

    assert!(!runtime.is_watching());
    assert_eq!(runtime.pending_changes().await, 0);
    assert!(runtime.pool().is_some()); // Pool should always be available now
}
```

**Example orchestration test:**
```rust
#[tokio::test]
async fn test_runtime_full_orchestration() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().to_path_buf();

    let mut runtime = Runtime::new(path).await.unwrap();

    // Perform cache operation
    runtime.cache.insert("query".to_string(), "result".to_string()).await;
    let cached = runtime.cache.get(&"query".to_string()).await;
    assert_eq!(cached, Some("result".to_string()));

    // Queue file event (simulates watcher)
    runtime.indexer.queue(WatchEvent::Modified(PathBuf::from("modified.rs")));
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Flush indexer
    let stats = runtime.process_events().await.unwrap();

    // Verify pool is accessible
    let pool = runtime.pool().unwrap();
    assert!(pool.available_connections() > 0);

    // No panics or errors - full orchestration works
}
```

### Graph Module Tests (`forge_core/src/graph/mod.rs`)

**Lines:** 150-168 (18 lines of tests)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_module_creation() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("test.db");

        let graph = GraphModule::new(&db_path);
        assert_eq!(graph.db_path(), &db_path);
    }

    #[test]
    fn test_detect_language_rust() {
        let lang = GraphModule::detect_language_from_path("src/main.rs");
        assert!(matches!(lang, crate::types::Language::Rust));
    }
}
```

### CFG Module Tests (`forge_core/src/cfg/mod.rs`)

**Lines:** 577-871 (294 lines of tests)

Comprehensive CFG testing:
- DominatorTree construction and manipulation
- Loop detection and analysis
- Path enumeration
- TestCfg helpers (chain, if_else, simple_loop)
- Edge cases and boundary conditions

**Example dominator test:**
```rust
#[test]
fn test_dominator_tree_dominates() {
    let mut tree = DominatorTree::new(BlockId(0));
    tree.insert(BlockId(1), BlockId(0));
    tree.insert(BlockId(2), BlockId(1));

    assert!(tree.dominates(BlockId(0), BlockId(0)));
    assert!(tree.dominates(BlockId(0), BlockId(1)));
    assert!(tree.dominates(BlockId(0), BlockId(2)));
    assert!(tree.dominates(BlockId(1), BlockId(1)));
    assert!(!tree.dominates(BlockId(1), BlockId(0)));
}
```

---

## Test Conventions

### Naming

Test functions use `test_` prefix with descriptive names:

```rust
// Good
async fn test_find_symbol_returns_empty_vector_when_not_found() { ... }

// Good
async fn test_search_builder_kind_filter() { ... }

// Bad
async fn test_it_works() { ... }
async fn test_main() { ... }
```

### AAA Pattern

Tests follow Arrange-Act-Assert structure:

```rust
#[tokio::test]
async fn test_example() {
    // ARRANGE: Set up test data
    let temp = tempfile::tempdir().unwrap();
    let forge = Forge::open(temp.path()).await.unwrap();
    let expected = SymbolId(123);

    // ACT: Execute function under test
    let result = forge.graph()
        .find_symbol_by_id(expected)
        .await
        .unwrap();

    // ASSERT: Verify result
    assert_eq!(result.id, expected);
}
```

Used throughout all test files.

### Async Tests

All async tests use `#[tokio::test]`:

```rust
#[tokio::test]
async fn test_async_operation() {
    let result = async_function().await.unwrap();
    assert!(result.is_some());
}
```

### Temporary Test Data

Use `tempfile` crate for test isolation:

```rust
#[tokio::test]
async fn test_with_temp_dir() {
    let temp_dir = tempfile::tempdir().unwrap();
    let store = UnifiedGraphStore::open(temp_dir.path()).await.unwrap();

    // Test using temp_dir
    // Directory is automatically cleaned up
}
```

---

## Coverage Requirements

### Minimum Coverage Goals

| Component | Target | Status |
|------------|---------|--------|
| Public API | 100% | Required |
| Internal logic | 80%+ | Target |
| Error paths | 100% | Required |
| Edge cases | Explicit | Required |

### Critical Paths

The following must have explicit test coverage:

1. **All public API functions** - Every public method tested
2. **All error variants** - Every error path tested
3. **All enum variants** - Every variant exercised
4. **Edge cases** - Empty inputs, boundary conditions
5. **Async cancellation** - For long-running operations
6. **Concurrent access** - For thread-safe structures

### What to Test

| Test Type | What to Cover |
|-----------|---------------|
| **Unit** | Individual function behavior, edge cases |
| **Integration** | Cross-module interactions, real I/O |
| **Error** | All error paths, error messages |
| **Async** | Cancellation, timeout scenarios |
| **Concurrency** | Thread-safety, race conditions |

---

## Test Utilities

### Common Test Module

**Location:** `tests/common/mod.rs`

**Utilities provided:**
- `test_forge()` - Creates test Forge instance with temp storage
- `create_test_file()` - Creates test file with content
- `create_test_rust_project()` - Creates Cargo project structure
- `test_symbol()` - Creates standardized Symbol
- `test_location()` - Creates standardized Location
- `test_span()` - Creates standardized Span
- `assert_error_variant()` - Verifies error content
- `wait_for()` - Async polling helper with timeout

**Self-testing:** `tests/common/mod.rs` contains 11 tests:
```rust
#[tokio::test]
async fn test_wait_for_timeout() {
    let result = wait_for(|| false, 50).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Timeout"));
}
```

### Fixtures

Test data helpers (not fixture files, but factory functions):

```rust
/// Creates a test Symbol with standard test values.
pub fn test_symbol() -> forge_core::Symbol {
    forge_core::Symbol {
        id: SymbolId(1),
        name: "test_function".to_string(),
        fully_qualified_name: "my_crate::test_function".to_string(),
        kind: SymbolKind::Function,
        language: Language::Rust,
        location: test_location(),
        parent_id: None,
        metadata: serde_json::Value::Null,
    }
}
```

---

## Running Tests

### All Tests

```bash
# Run all workspace tests
cargo test --workspace

# Run with output
cargo test -- --nocapture

# Run single test
cargo test test_find_symbol

# Run tests for specific crate
cargo test -p forge_core
```

### Test Filtering

```bash
# Run only unit tests
cargo test --lib

# Run only integration tests
cargo test --test integration

# Skip slow tests
cargo test -- --skip slow

# Run tests matching pattern
cargo test graph
cargo test cache
cargo test pool
```

### Test Output

```bash
# Show test output
cargo test -- --nocapture

# Show stdout of passed tests
cargo test -- --show-output

# Concise output
cargo test -- --quiet
```

---

## Test File Size Limits

| File Type | LOC Limit | Rationale |
|------------|------------|-----------|
| Test modules | 500 LOC | Maintainability |
| Unit test per function | ~20 LOC | Focused testing |
| Integration test | ~50 LOC | Clear scenario |

**When exceeding limits:**
- Extract to helper functions
- Use parameterized tests
- Split into multiple test files
- Create test utilities in `tests/common/mod.rs`

---

## Common Test Patterns

### Result Testing

```rust
#[tokio::test]
async fn test_result_ok() {
    let result = operation().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_result_error() {
    let result = operation().await;
    assert!(result.is_err());
    assert!(matches!(result, Err(ForgeError::SymbolNotFound(_))));
}
```

### Option Testing

```rust
#[tokio::test]
async fn test_option_some() {
    let result = maybe_find().await;
    assert!(result.is_some());
}

#[tokio::test]
async fn test_option_none() {
    let result = maybe_find().await;
    assert!(result.is_none());
}
```

### Builder Testing

```rust
#[tokio::test]
async fn test_builder_filters() {
    let store = Arc::new(UnifiedGraphStore::open(
        tempfile::tempdir().unwrap()
    ).await.unwrap());
    let module = GraphModule::new(store.db_path());

    let builder = module.symbol("test")
        .kind(SymbolKind::Function)
        .limit(10);

    assert_eq!(builder.name_filter, Some("test".to_string()));
    assert!(matches!(builder.kind_filter, Some(SymbolKind::Function)));
    assert_eq!(builder.limit, Some(10));
}
```

### Module Creation Testing

```rust
#[tokio::test]
async fn test_module_creation() {
    let store = Arc::new(UnifiedGraphStore::open(
        tempfile::tempdir().unwrap()
    ).await.unwrap());
    let module = GraphModule::new(store.clone());

    // Verify module is properly initialized
    assert_eq!(module.store.db_path(), store.db_path());
}
```

### Cache Testing Patterns

```rust
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
}
```

### Pool Testing Patterns

```rust
#[tokio::test]
async fn test_pool_permit_drop_returns() {
    let pool = ConnectionPool::new("/tmp/test.db", 3);
    assert_eq!(pool.available_connections(), 3);

    // Acquire permit
    let permit = pool.acquire().await.unwrap();
    assert_eq!(pool.available_connections(), 2);

    // Drop permit
    drop(permit);
    assert_eq!(pool.available_connections(), 3);
}
```

### Watcher Testing Patterns

```rust
#[tokio::test]
async fn test_watcher_debounce() {
    let temp_dir = TempDir::new().unwrap();
    let (tx, mut rx) = Watcher::channel();
    let store = Arc::new(UnifiedGraphStore::memory().await.unwrap());
    let watcher = Watcher::new(store, tx);

    // Start watching the temp directory
    watcher.start(temp_dir.path().to_path_buf()).await.unwrap();

    // Give watcher a moment to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Create a test file first
    let test_file = temp_dir.path().join("test_debounce.rs");
    tokio::fs::write(&test_file, "fn test() {}").await.unwrap();

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
    assert!(events.len() < 3);
}
```

---

## CI/CD Requirements

### Pre-commit Checks

Before pushing, ensure:

```bash
# Run all tests
cargo test --workspace

# Check compilation (faster than build)
cargo check --workspace

# Format check
cargo fmt -- --check

# Lint
cargo clippy --all-targets
```

### CI Pipeline

The CI pipeline must run:

1. All tests across all workspace members
2. `cargo check` for quick compilation verification
3. `cargo clippy` for lint verification
4. `cargo fmt --check` for formatting verification
5. Documentation builds without warnings

---

## Debugging Failed Tests

### Show Backtrace

```bash
RUST_BACKTRACE=1 cargo test
```

### Run Single Test with Output

```bash
cargo test test_name -- --nocapture --show-output
```

### Use Debugger

```bash
# Use lldb for debugging
cargo test test_name -- --nocapture
# Then use rust-lldb
```

### Test with Filters

```bash
# Run only tests matching pattern
cargo test cache

# Run tests in specific file
cargo test -- --test cache

# Run specific test
cargo test test_cache_insert_get
```

---

## Performance Testing

### When to Add Benchmarks

- Algorithm changes
- Data structure changes
- Optimization work
- When investigating performance

### Benchmark Conventions

- Use `criterion` framework (planned for v0.2+)
- Include realistic data sizes
- Document what's being measured
- Include comparison with baseline

### Performance Test Locations

Not yet implemented (planned for v0.2+).
Should go in `benches/` directory.

---

## Documentation Examples

Tests can serve as documentation examples:

```rust
/// Finds a symbol by name.
///
/// # Examples
///
/// ```rust
/// use forge_core::Forge;
/// # let forge = unimplemented!();
/// let symbols = forge.graph().find_symbol("main").await?;
/// # Ok::<(), anyhow::Error>(())
/// ```
pub async fn find_symbol(&self, name: &str) -> Result<Vec<Symbol>> {
    // ...
}
```

Run doctests with:
```bash
cargo test --doc
```

---

## Module-Specific Testing Guidelines

### Graph Module Testing

**File:** `forge_core/src/graph/mod.rs`
**Lines:** 150-168 (18 lines of tests)

Focus areas:
- Language detection from file extensions
- Symbol kind conversion from Magellan types
- Module creation and initialization

**Test example:**
```rust
#[test]
fn test_detect_language_rust() {
    let lang = GraphModule::detect_language_from_path("src/main.rs");
    assert!(matches!(lang, crate::types::Language::Rust));
}
```

### Storage Module Testing

**File:** `forge_core/src/storage/mod.rs`
**Lines:** 295-347 (52 lines of tests)

Focus areas:
- Store opening (default and custom paths)
- In-memory store creation
- Symbol and reference queries (currently stubbed)
- Parse helper functions

**Test example:**
```rust
#[tokio::test]
async fn test_open_with_custom_path() {
    let temp_dir = TempDir::new().unwrap();
    let custom_db = temp_dir.path().join("custom").join("db.sqlite");

    let store = UnifiedGraphStore::open_with_path(temp_dir.path(), &custom_db).await.unwrap();

    assert_eq!(store.db_path(), custom_db);
}
```

### Search Module Testing

**File:** `forge_core/src/search/mod.rs`
**Status:** NOT YET IMPLEMENTED - Phase 08.2

### CFG Module Testing

**File:** `forge_core/src/cfg/mod.rs`
**Lines:** 577-871 (294 lines of tests)

Comprehensive CFG testing:
- Dominator tree computation
- Path enumeration
- Loop detection
- TestCfg helpers

### Edit Module Testing

**File:** `forge_core/src/edit/mod.rs`
**Status:** NOT YET IMPLEMENTED - Phase 08.4

### Analysis Module Testing

**File:** `forge_core/src/analysis/mod.rs`
**Status:** NOT YET IMPLEMENTED - Phase 08.5

### Agent Module Testing

**File:** `forge_agent/src/lib.rs`
**Lines:** 322-333 (11 lines of tests)

Focus areas:
- Agent creation
- Module initialization

**Test example:**
```rust
#[tokio::test]
async fn test_agent_creation() {
    let temp = tempfile::tempdir().unwrap();
    let agent = Agent::new(temp.path()).await.unwrap();

    assert_eq!(agent.codebase_path, temp.path());
}
```

---

## Test Statistics by Module

| Module | Test Lines | Coverage | Focus |
|---------|------------|----------|-------|
| `types.rs` | 429 | Types, ID, Span, Location |
| `error.rs` | 23 | Error display, conversions |
| `graph/mod.rs` | 18 | Language detection, creation |
| `storage/mod.rs` | 52 | Store operations, parsing |
| `cache.rs` | 201 | LRU, TTL, eviction |
| `pool.rs` | 193 | Concurrency, permits |
| `watcher.rs` | 296 | File events, debounce |
| `indexing.rs` | 204 | Event queueing, flush |
| `runtime.rs` | 216 | Orchestration, integration |
| `cfg/mod.rs` | 294 | Dominators, paths, loops |
| `lib.rs` (forge_core) | 276 | Forge creation, builders |
| `lib.rs` (forge_agent) | 11 | Agent creation |
| `tests/common/mod.rs` | 101 | Test utilities |
| Integration tests | ~100 | Cross-module workflows |

**Total:** Approximately 2,100+ lines of tests

---

## Testing Best Practices

### 1. Always Use Temporary Directories

```rust
let temp_dir = tempfile::tempdir().unwrap();
// Use temp_dir.path() for test data
// Automatically cleaned up
```

### 2. Use Meaningful Test Names

```rust
// Good
async fn test_cache_eviction_when_full() { ... }

// Bad
async fn test_cache_1() { ... }
async fn test_it_works() { ... }
```

### 3. Test Error Paths Explicitly

```rust
#[tokio::test]
async fn test_error_returns_symbol_not_found() {
    let result = forge.graph().find_symbol("nonexistent").await;

    assert!(result.is_err());
    assert!(matches!(result, Err(ForgeError::SymbolNotFound(_))));
}
```

### 4. Use Timeouts for Async Tests

```rust
use tokio::time::{timeout, Duration};

let event = timeout(Duration::from_secs(2), rx.recv())
    .await
    .expect("Timeout waiting for event");
```

### 5. Test Boundary Conditions

```rust
#[test]
fn test_span_boundary() {
    // Test at boundaries
    let span = Span { start: 0, end: 0 };
    assert!(span.is_empty());

    let span = Span { start: u32::MAX, end: u32::MAX };
    assert_eq!(span.len(), 0);
}
```

### 6. Use Test Helpers for Common Patterns

```rust
// Use common test utilities from tests/common/mod.rs
let (_temp, forge) = test_forge().await.unwrap();
let location = test_location();
let symbol = test_symbol();
```

### 7. Test Concurrent Access for Thread-Safe Code

```rust
use tokio::sync::Barrier;

let barrier = Arc::new(Barrier::new(10));
// Spawn multiple tasks
// Verify no race conditions
```

### 8. Use Assert Macros Appropriately

```rust
// Equality
assert_eq!(result, expected);

// Inequality
assert_ne!(result, unexpected);

// Boolean
assert!(condition);

// Matches
assert!(matches!(value, Some(_)));

// Error messages
assert!(result.is_err(), "Expected error");
```

---

*Testing analysis: 2025-02-13*

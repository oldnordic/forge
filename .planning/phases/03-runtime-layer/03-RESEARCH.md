# Phase 03: Test Infrastructure - Research

**Date:** 2026-02-12
**Phase:** Test Infrastructure for forge_core
**Status:** Research Complete

---

## Executive Summary

This document captures research findings needed to plan and implement Phase 03: Test Infrastructure for the ForgeKit project. Based on analysis of the existing codebase, this phase requires building a comprehensive test suite for the runtime layer components (`watcher.rs`, `indexing.rs`, `cache.rs`, `pool.rs`, `runtime.rs`) using Rust testing best practices.

**Key Finding:** The codebase already has a solid foundation with 38 existing unit tests across Phase 1 modules. The test infrastructure pattern is established: using `tempfile` for test isolation, `tokio::test` for async tests, and `#[cfg(test)]` modules within each source file.

---

## Current State Analysis

### Existing Test Infrastructure

The project already has some test infrastructure in place:

**File:** `/home/feanor/Projects/forge/tests/common/mod.rs`

```rust
pub async fn test_forge() -> anyhow::Result<(TempDir, forge_core::Forge)>
pub async fn create_test_file(dir: &Path, name: &str, content: &str) -> anyhow::Result<PathBuf>
pub async fn create_test_rust_project(dir: &Path, name: &str) -> anyhow::Result<()>
```

**Observations:**
- Common test helpers are already defined
- Uses `tempfile::TempDir` for temporary directory management
- Returns `(TempDir, Forge)` tuple - TempDir must be kept alive for tests
- Helpers are async, using tokio runtime
- Includes self-tests (testing the test utilities themselves)

### Existing Test Patterns

Analyzing tests across the codebase reveals consistent patterns:

1. **Module-Scoped Tests:** All tests are in `#[cfg(test)]` modules within each source file
2. **Async Tests:** Using `#[tokio::test]` macro for async operations
3. **Tempfile Usage:** `tempfile::tempdir()` for isolated test environments
4. **Assertion Style:** Standard `assert_eq!`, `assert!` macros
5. **Test Organization:** Tests are co-located with implementation

**Example Pattern (from `graph/mod.rs`):**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_graph_module_creation() {
        let store = Arc::new(UnifiedGraphStore::open(
            tempfile::tempdir().unwrap()
        ).await.unwrap());
        let module = GraphModule::new(store.clone());

        assert_eq!(module.store.db_path(), store.db_path());
    }
}
```

### Dependencies for Testing

**From `forge_core/Cargo.toml`:**

```toml
[dev-dependencies]
tokio = { version = "1", features = ["test-util", "macros"] }
tempfile = "3"
```

**Observation:** `tempfile` is already in dev-dependencies. No additional dependencies needed for basic testing.

---

## Components Requiring Tests

### Runtime Layer Components (Phase 2)

The runtime layer consists of 5 modules that need comprehensive test coverage:

| Module | File | Purpose | Current Test Coverage |
|--------|------|---------|----------------------|
| Watcher | `watcher.rs` | File system monitoring | 3 tests (basic creation, channel, event equality) |
| Indexing | `indexing.rs` | Incremental reindexing | 5 tests (creation, queue, flush, stats, clear) |
| Cache | `cache.rs` | LRU caching with TTL | Not yet analyzed |
| Pool | `pool.rs` | Connection pooling | Not yet analyzed |
| Runtime | `runtime.rs` | Orchestration layer | 5 tests (creation, cache, pending changes, process events, watching) |

**Total Runtime Tests:** 13 existing tests

### Core SDK Modules (Phase 1)

Already tested with good coverage:

| Module | File | Test Count | Status |
|--------|------|-----------|--------|
| Types | `types.rs` | 0 tests | Needs tests |
| Error | `error.rs` | 3 tests | ✅ Complete |
| Storage | `storage/mod.rs` | 6 tests | ✅ Complete |
| Graph | `graph/mod.rs` | 6 tests | ✅ Complete |
| Search | `search/mod.rs` | 5 tests | ✅ Complete |
| CFG | `cfg/mod.rs` | 15 tests | ✅ Complete |
| Edit | `edit/mod.rs` | 11 tests | ✅ Complete |
| Analysis | `analysis/mod.rs` | 4 tests | ✅ Complete |
| Lib | `lib.rs` | 0 tests | Needs tests (Forge builders) |

**Total Phase 1 Tests:** 50 tests (13 existing across runtime + 37 existing/needed across core)

**Gap:** `types.rs` needs comprehensive tests for all type definitions. `lib.rs` needs tests for Forge builder patterns.

---

## Rust Testing Best Practices

### tempfile Crate Usage

**Purpose:** Create temporary directories that are automatically cleaned up

**Installation:** Already in dev-dependencies

**Common Patterns:**

```rust
// 1. Basic temp directory
#[tokio::test]
async fn test_with_temp_dir() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path(); // Use path for test

    // temp_dir is automatically deleted when it goes out of scope
}

// 2. TempDir that must be kept alive
#[tokio::test]
async fn test_forge_instance() {
    let (_temp, forge) = test_forge().await.unwrap();
    // _temp must not be dropped or forge loses its database
    // Prefix with _ to indicate intentionally unused

    // Use forge in test...
}
```

**Key Insight:** The `TempDir` must outlive any operations that use its path. When returned from a helper function, both should be returned as a tuple.

### Unit vs Integration Test Strategies

**Unit Tests (`src/*/tests` modules):**
- Test individual functions and types
- Fast to run (no external dependencies)
- Test edge cases and error conditions
- Located within source files for co-location

**Integration Tests (`tests/` directory):**
- Test public API surfaces
- Test interactions between modules
- May use real filesystem and database
- Located in workspace `tests/` directory

**ForgeKit Pattern:**
- Use `#[cfg(test)]` modules for unit tests within each file
- Use `tests/` directory for:
  - Cross-module integration tests
  - Common test utilities (`tests/common/mod.rs`)
  - Builder pattern tests
  - Module accessor tests

### Test Organization for Workspace

**Workspace Structure:**
```
forge/
├── forge_core/
│   ├── src/
│   │   ├── lib.rs (with #[cfg(test)] mod tests)
│   │   ├── types.rs (with #[cfg(test)] mod tests)
│   │   └── ...
│   └── Cargo.toml
├── forge_runtime/
│   └── ...
├── forge_agent/
│   └── ...
└── tests/
    ├── common/
    │   └── mod.rs (shared test utilities)
    ├── integration/
    │   └── (cross-module integration tests)
    └── fixtures/
        └── (test data/fixtures)
```

**Running Tests:**
```bash
# All workspace tests
cargo test --workspace

# Specific crate tests
cargo test -p forge_core

# Specific test function
cargo test test_graph_module_creation

# Run tests with output
cargo test -- --nocapture

# Run tests in parallel
cargo test -- --test-threads=4
```

### Coverage Goals and Approaches

**Target Metrics:**
- **Line Coverage:** 80%+ (industry standard for production code)
- **Branch Coverage:** 70%+ (all important branches covered)
- **Critical Paths:** 100% (error handling, database operations)

**Coverage Tools:**
```bash
# Install tarpaulin for coverage reports
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --workspace --out Html

# Generate line coverage report
cargo tarpaulin --workspace --out Stdout
```

**What to Test (Priority Order):**
1. **Public API:** All public functions must have tests
2. **Error Paths:** All error variants must be exercised
3. **Edge Cases:** Empty inputs, maximum values, boundary conditions
4. **Integration Points:** Where modules interact
5. **Performance Paths:** Hot code paths (identified via profiling)

**What NOT to Test:**
- Trivial getters/setters (e.g., `field.value()`)
- Generated implementations (Debug, Clone, etc.)
- External crate behavior (assume tempfile works correctly)

---

## SQLite-Based Test Patterns

### In-Memory Database Testing

**From `storage/mod.rs`:**

```rust
pub async fn memory() -> Result<Self> {
    #[cfg(feature = "sqlite")]
    let graph = Some(Arc::new(sqlitegraph::SqliteGraph::open_in_memory_with_config(
        &sqlitegraph::SqliteConfig::default(),
    ).map_err(|e| ForgeError::DatabaseError(e.to_string()))?));

    // ...
}
```

**Usage in Tests:**
```rust
#[tokio::test]
async fn test_with_memory_db() {
    let store = UnifiedGraphStore::memory().await.unwrap();
    // Use store - completely isolated, no filesystem cleanup needed
}
```

**Advantages:**
- No filesystem cleanup required
- Faster than file-based databases
- Perfect for isolated unit tests
- Each test gets a fresh database

### Temporary File Database Testing

**From existing tests:**
```rust
#[tokio::test]
async fn test_with_temp_db() {
    let temp_dir = tempfile::tempdir().unwrap();
    let store = UnifiedGraphStore::open(temp_dir.path()).await.unwrap();
    // Database at temp_dir/.forge/graph.db
}
```

**Use Cases:**
- Testing file-based operations
- Testing database persistence
- Testing file watcher integration
- Integration tests that need real filesystem

---

## Testing Async Code with Tokio

### Async Test Patterns

**All tests must be `#[tokio::test]` for async operations:**

```rust
#[tokio::test]
async fn test_async_function() {
    let result = async_operation().await.unwrap();
    assert_eq!(result, expected);
}
```

### Time-Based Testing

**From `runtime.rs` tests:**
```rust
#[tokio::test]
async fn test_runtime_pending_changes() {
    runtime.indexer.queue(WatchEvent::Modified(PathBuf::from("test.rs")));
    tokio::time::sleep(Duration::from_millis(50)).await; // Wait for async task

    assert_eq!(runtime.pending_changes().await, 1);
}
```

**Key Pattern:** When testing async code that spawns background tasks:
1. Use `tokio::time::sleep()` to allow tasks to execute
2. Small delays (10-100ms) are usually sufficient
3. Avoid arbitrary time delays when possible (prefer channels/notifications)

### Channel-Based Testing

**From `watcher.rs` tests:**
```rust
#[tokio::test]
async fn test_watcher_channel() {
    let (tx, mut rx) = Watcher::channel();

    tx.send(WatchEvent::Created(path.clone())).unwrap();

    let received = rx.recv().await.unwrap();
    assert_eq!(received, WatchEvent::Created(path));
}
```

**Pattern:** Use `mpsc::channel()` or `mpsc::unbounded_channel()` for:
- Testing event emission
- Testing concurrent operations
- Testing producer-consumer patterns

---

## Test Fixtures and Builders

### Test Fixture Pattern

**Purpose:** Reusable test data structures

**Example (Proposed for types.rs tests):**
```rust
#[cfg(test)]
mod fixtures {
    use super::*;

    pub fn test_symbol() -> Symbol {
        Symbol {
            id: SymbolId(1),
            name: "test_function".to_string(),
            fully_qualified_name: "my_crate::test_function".to_string(),
            kind: SymbolKind::Function,
            language: Language::Rust,
            location: Location {
                file_path: PathBuf::from("src/lib.rs"),
                byte_start: 0,
                byte_end: 100,
                line_number: 10,
            },
            parent_id: None,
            metadata: serde_json::Value::Null,
        }
    }

    pub fn test_location() -> Location {
        Location {
            file_path: PathBuf::from("src/test.rs"),
            byte_start: 42,
            byte_end: 84,
            line_number: 7,
        }
    }
}
```

### Builder Pattern for Tests

**Purpose:** Construct complex test data incrementally

**Example (for Forge tests):**
```rust
pub struct TestForgeBuilder {
    path: Option<PathBuf>,
    cache_ttl: Option<Duration>,
}

impl TestForgeBuilder {
    pub fn new() -> Self {
        Self { path: None, cache_ttl: None }
    }

    pub fn path(mut self, path: PathBuf) -> Self {
        self.path = Some(path);
        self
    }

    pub fn cache_ttl(mut self, ttl: Duration) -> Self {
        self.cache_ttl = Some(ttl);
        self
    }

    pub async fn build(self) -> (TempDir, Forge) {
        let temp = TempDir::new().unwrap();
        let path = self.path.unwrap_or_else(|| temp.path().to_path_buf());

        let forge = Forge::open(&path).await.unwrap();
        (temp, forge)
    }
}
```

---

## Custom Test Utilities

### Assert Helpers

**Purpose:** Reduce boilerplate in common assertions

**Examples:**
```rust
/// Asserts that a symbol has the given name
fn assert_symbol_name(symbol: &Symbol, expected: &str) {
    assert_eq!(symbol.name, expected,
        "Expected symbol named {}, got {}", expected, symbol.name);
}

/// Asserts that a result is an error of the given variant
fn assert_error_variant<T>(result: Result<T>, expected_variant: &str) {
    match result {
        Err(e) => {
            let error_string = e.to_string();
            assert!(error_string.contains(expected_variant),
                "Expected error containing '{}', got: {}", expected_variant, error_string);
        }
        Ok(_) => panic!("Expected error, got Ok"),
    }
}
```

### Async Test Helpers

**Purpose:** Handle common async test patterns

**Examples:**
```rust
/// Runs a function with a timeout
async fn with_timeout<F, T>(duration: Duration, f: F) -> anyhow::Result<T>
where
    F: Future<Output = anyhow::Result<T>>,
{
    tokio::time::timeout(duration, f).await?
}

/// Waits for a condition to be true (with polling)
async fn wait_for<F>(mut condition: F, timeout_ms: u64) -> anyhow::Result<()>
where
    F: FnMut() -> bool,
{
    let start = std::time::Instant::now();
    while !condition() {
        if start.elapsed().as_millis() > timeout_ms as u128 {
            anyhow::bail!("Timeout waiting for condition");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    Ok(())
}
```

---

## Specific Test Requirements by Module

### types.rs Tests Needed

**SymbolId:**
- Display formatting
- From<i64> conversion
- Hash consistency
- Ord ordering

**BlockId:**
- Display formatting
- From<i64> conversion

**PathId:**
- Display formatting (hex with colons)
- Hash stability (same blocks → same ID)
- Hash uniqueness (different blocks → different IDs)

**Location:**
- span() method
- len() method
- File path handling

**Span:**
- is_empty()
- contains(offset)
- merge()
- len()

**SymbolKind:**
- is_type() predicate
- is_function() predicate

**Symbol:**
- All fields constructable
- Clone behavior
- Eq behavior

**Reference, Cycle, Loop, Path:**
- Construction
- Serialization/deserialization (via serde_json)

### lib.rs Tests Needed

**Forge:**
- `open()` creates valid instance
- `open()` creates database at correct path
- Module accessors return correct types
- Clone behavior
- Builder pattern

**ForgeBuilder:**
- Default configuration
- Path setter
- Database path setter
- Cache TTL setter
- Build creates valid Forge

---

## Integration Test Strategy

### Builder Tests

**Location:** `tests/integration/builder_tests.rs`

**Test Cases:**
```rust
#[tokio::test]
async fn test_forge_builder_default() {
    let temp = tempfile::tempdir().unwrap();
    let forge = ForgeBuilder::new()
        .path(temp.path())
        .build()
        .await
        .unwrap();

    // Verify defaults: no runtime, default cache, etc.
}

#[tokio::test]
async fn test_forge_builder_custom_db_path() {
    let temp = tempfile::tempdir().unwrap();
    let custom_db = temp.path().join("custom").join("db.sqlite");

    let forge = ForgeBuilder::new()
        .path(temp.path())
        .database_path(&custom_db)
        .build()
        .await
        .unwrap();

    assert!(custom_db.exists());
}
```

### Module Accessor Tests

**Location:** `tests/integration/module_accessor_tests.rs`

**Test Cases:**
```rust
#[tokio::test]
async fn test_forge_graph_accessor() {
    let (_temp, forge) = test_forge().await.unwrap();

    let graph = forge.graph();
    // Verify graph module is functional
    let symbols = graph.find_symbol("nonexistent").await.unwrap();
    assert_eq!(symbols.len(), 0);
}

#[tokio::test]
async fn test_forge_all_accessors() {
    let (_temp, forge) = test_forge().await.unwrap();

    // All accessors should return valid instances
    let _graph = forge.graph();
    let _search = forge.search();
    let _cfg = forge.cfg();
    let _edit = forge.edit();
    let _analysis = forge.analysis();
}
```

---

## Test-Driven Development Workflow

### TDD Cycle for This Phase

1. **Red:** Write failing test first
   - Start with `types.rs` tests (no external dependencies)
   - Add test, verify it fails with `cargo test`

2. **Green:** Make test pass
   - Implement minimal code to pass
   - Run `cargo test` to verify

3. **Refactor:** Clean up
   - Extract common test utilities
   - Improve test organization
   - Ensure tests still pass

### Quick Verification Commands

```bash
# Fast check during development
cargo check --tests

# Run specific module tests
cargo test -p forge_core types

# Run with output for debugging
cargo test test_symbol_id_display -- --nocapture

# Run tests in documentation examples
cargo test --doc
```

---

## Open Questions and Considerations

### 1. Should we use a test assertion crate?

**Options:**
- `pretty_assertions`: Better diff output for assert_eq!
- `claim`: Assertion builder with better error messages
- Standard `assert!` macros: Simple, no dependencies

**Recommendation:** Stick with standard macros unless diff output becomes problematic. Can add later if needed.

### 2. How to test file watcher reliably?

**Challenge:** File watching is OS-dependent and timing-sensitive

**Approaches:**
- Create temp directory structure
- Trigger file operations (create/modify/delete)
- Wait for events via channel
- Assert received events

**Example Pattern:**
```rust
#[tokio::test]
async fn test_file_watcher_detects_changes() {
    let temp = tempfile::tempdir().unwrap();
    let (tx, mut rx) = Watcher::channel();

    let store = Arc::new(UnifiedGraphStore::open(temp.path()).await.unwrap());
    let watcher = Watcher::new(store, tx);

    watcher.start(temp.path().to_path_buf()).await.unwrap();

    // Create a file
    let test_file = temp.path().join("test.rs");
    tokio::fs::write(&test_file, "fn test() {}").await.unwrap();

    // Wait for event with timeout
    let event = tokio::time::timeout(
        Duration::from_secs(2),
        rx.recv()
    ).await.unwrap().unwrap();

    assert!(matches!(event, WatchEvent::Created(_)));
}
```

### 3. Testing with real vs mock databases?

**Current Approach:** Use real SQLiteGraph with in-memory/temp files

**Pros:**
- Tests actual integration
- Catches SQLite-specific bugs
- No mocking overhead

**Cons:**
- Slower than pure mocks
- Requires sqlite feature

**Recommendation:** Continue with real databases. The in-memory backend is fast enough for unit tests.

### 4. Property-based testing?

**Options:**
- `proptest`: Generate random test inputs
- `quickcheck`: Similar to proptest
- Example-based testing: Manual test cases

**Recommendation:** Skip for this phase. Property-based testing is valuable but adds complexity. Can be added in Phase 15 (Polish) if edge cases emerge.

---

## Recommended Test Structure

### Proposed additions to `tests/common/mod.rs`:

```rust
/// Test fixture builder for Forge instances
pub struct TestForgeBuilder {
    // ...
}

/// Creates a symbol with default test values
pub fn test_symbol() -> Symbol { /* ... */ }

/// Creates a test location
pub fn test_location() -> Location { /* ... */ }

/// Assert helper for error variants
pub fn assert_error_variant(result: Result<anyhow::()>, expected: &str) { /* ... */ }

/// Async test helper for polling conditions
pub async fn wait_for<F>(condition: F, timeout_ms: u64) -> anyhow::Result<()>
where
    F: Fn() -> bool { /* ... */ }
```

### New test files to create:

```
tests/
├── common/
│   └── mod.rs (expanded with new utilities)
├── integration/
│   ├── builder_tests.rs (ForgeBuilder tests)
│   ├── accessor_tests.rs (Module accessor tests)
│   └── runtime_integration_tests.rs (Cross-runtime tests)
└── fixtures/
    └── rust_projects/ (Sample project structures for indexing tests)
        ├── simple_crate/
        ├── workspace_crate/
        └── with_tests/
```

---

## Estimated Test Counts

### Breakdown by Module:

| Module | Current Tests | Recommended Tests | Gap |
|--------|---------------|-------------------|-----|
| types.rs | 0 | 20-25 | +25 |
| error.rs | 3 | 5-8 | +5 |
| storage/mod.rs | 6 | 8-10 | +4 |
| graph/mod.rs | 6 | 8-10 | +4 |
| search/mod.rs | 5 | 6-8 | +3 |
| cfg/mod.rs | 15 | 15-18 | +3 |
| edit/mod.rs | 11 | 12-14 | +3 |
| analysis/mod.rs | 4 | 5-6 | +2 |
| watcher.rs | 3 | 6-8 | +5 |
| indexing.rs | 5 | 8-10 | +5 |
| cache.rs | ? | 10-12 | +12 (estimated) |
| pool.rs | ? | 8-10 | +10 (estimated) |
| runtime.rs | 5 | 8-10 | +5 |
| lib.rs | 0 | 6-8 | +8 |
| Integration | 0 | 15-20 | +20 |

**Total:**
- Current: ~53 tests
- Recommended: ~130-150 tests
- **Gap: ~80-100 new tests needed**

---

## Success Criteria for Phase 03

### Must Have (Blocking):
- [ ] All runtime modules have >80% line coverage
- [ ] All public API functions have at least one test
- [ ] All error code paths are tested
- [ ] Integration tests for builder patterns pass
- [ ] Integration tests for module accessors pass
- [ ] Common test utilities documented
- [ ] All tests pass with `cargo test --workspace`

### Should Have (Important):
- [ ] types.rs has comprehensive tests for all types
- [ ] Property-based tests for Span operations (merge, contains)
- [ ] File watcher integration tests pass on Linux
- [ ] Cache TTL behavior verified with time-based tests
- [ ] Connection pool limit behavior tested

### Could Have (Nice to Have):
- [ ] Performance benchmarks for critical paths
- [ ] Fuzzing setup for parsing code
- [ ] Documentation examples tested via `cargo test --doc`

---

## Recommended Implementation Order

### Week 1: Core Types and Utilities (3-4 days)
1. **Day 1:** Expand `tests/common/mod.rs` with new utilities
2. **Day 2:** Write comprehensive `types.rs` tests (25 tests)
3. **Day 3:** Write `lib.rs` tests for Forge and ForgeBuilder (8 tests)
4. **Day 4:** Expand error.rs tests (add 5 tests)

### Week 2: Runtime Layer Tests (5 days)
1. **Day 1:** Expand watcher.rs tests (add 5 tests, including file operations)
2. **Day 2:** Expand indexing.rs tests (add 5 tests, including flush scenarios)
3. **Day 3:** Write comprehensive cache.rs tests (12 tests)
4. **Day 4:** Write comprehensive pool.rs tests (10 tests)
5. **Day 5:** Expand runtime.rs tests (add 5 tests, including integration)

### Week 3: Integration Tests (2-3 days)
1. **Day 1:** Write builder integration tests (10 tests)
2. **Day 2:** Write module accessor integration tests (5 tests)
3. **Day 3:** Write runtime integration tests (5 tests)

### Final: Verification (1 day)
- Run coverage report
- Fix any uncovered critical paths
- Document test utilities
- Verify all tests pass

**Total Estimated Time:** 10-12 days

---

## Additional Resources

### Rust Testing Documentation
- [The Rust Book - Testing](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Rust By Example - Testing](https://doc.rust-lang.org/rust-by-example/testing.html)
- [tokio::test documentation](https://docs.rs/tokio/latest/tokio/attr.test.html)

### tempfile Crate Documentation
- [tempfile on crates.io](https://crates.io/crates/tempfile)
- [tempfile documentation](https://docs.rs/tempfile/)

### SQLiteGraph Testing
- [sqlitegraph test patterns](https://docs.rs/sqlitegraph/)

### Code Coverage Tools
- [cargo-tarpaulin](https://github.com/xd009642/tarpaulin)
- [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov)

---

## Appendix: Existing Test Examples

### Example 1: Complex Async Test with Tempfile

**From `storage/mod.rs`:**
```rust
#[tokio::test]
async fn test_open_with_custom_path() {
    let temp_dir = tempfile::tempdir().unwrap();
    let custom_db = temp_dir.path().join("custom").join("db.sqlite");

    let store = UnifiedGraphStore::open_with_path(temp_dir.path(), &custom_db).await.unwrap();

    assert_eq!(store.db_path(), custom_db);
}
```

### Example 2: Graph Algorithm Test

**From `cfg/mod.rs`:**
```rust
#[test]
fn test_paths_if_else() {
    let cfg = TestCfg::if_else();
    let paths = cfg.enumerate_paths();

    assert_eq!(paths.len(), 2);

    let paths_set: HashSet<_> = paths.iter().map(|p| p.blocks.clone()).collect();

    assert!(paths_set.contains(&vec![BlockId(0), BlockId(1), BlockId(3)]));
    assert!(paths_set.contains(&vec![BlockId(0), BlockId(2), BlockId(3)]));
}
```

### Example 3: Builder Pattern Test

**From `search/mod.rs`:**
```rust
#[tokio::test]
async fn test_search_builder() {
    let store = Arc::new(UnifiedGraphStore::open(
        tempfile::tempdir().unwrap()
    ).await.unwrap());
    let module = SearchModule::new(store);

    let builder = module.symbol("test")
        .kind(SymbolKind::Function)
        .limit(10);

    assert_eq!(builder.name_filter, Some("test".to_string()));
    assert!(matches!(builder.kind_filter, Some(SymbolKind::Function)));
    assert_eq!(builder.limit, Some(10));
}
```

---

*Research completed: 2026-02-12*
*Next step: Create implementation plan (03-PLAN.md)*

# Testing Guide for ForgeKit

Comprehensive testing documentation for ForgeKit developers.

## Table of Contents

1. [Test Organization](#test-organization)
2. [Running Tests](#running-tests)
3. [Test Categories](#test-categories)
4. [Writing Tests](#writing-tests)
5. [Feature Flag Testing](#feature-flag-testing)
6. [Backend Testing](#backend-testing)
7. [Pub/Sub Testing](#pubsub-testing)
8. [Continuous Integration](#continuous-integration)

## Test Organization

```
forge/
├── forge_core/
│   ├── src/
│   │   └── *.rs              # Unit tests in #[cfg(test)] modules
│   └── tests/
│       ├── accessor_tests.rs      # Module accessor tests
│       ├── builder_tests.rs       # Builder pattern tests
│       ├── pubsub_integration_tests.rs  # Pub/Sub + backends
│       └── tool_integration_tests.rs    # Tool integrations
├── forge_runtime/
│   └── src/
│       └── lib.rs            # Embedded tests
├── forge_agent/
│   └── src/
│       └── *.rs              # Embedded tests
└── tests/
    └── integration/          # Cross-crate integration tests
```

## Running Tests

### Basic Commands

```bash
# Run all tests
cargo test --workspace

# Run tests for specific crate
cargo test -p forge_core
cargo test -p forge_runtime
cargo test -p forge-agent

# Run with all features
cargo test --workspace --all-features

# Run specific test
cargo test test_name

# Run tests matching pattern
cargo test pubsub

# Run with output visible
cargo test -- --nocapture
```

### Feature-Specific Testing

```bash
# Test SQLite backend only
cargo test -p forge_core --features sqlite

# Test Native V3 backend only
cargo test -p forge_core --features native-v3

# Test with all tools on SQLite
cargo test -p forge_core --features tools-sqlite

# Test with all tools on V3
cargo test -p forge_core --features tools-v3

# Test full stack with V3
cargo test --features full-v3
```

## Test Categories

### 1. Unit Tests

Embedded in source files using `#[cfg(test)]`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_forge_creation() {
        let temp = tempfile::tempdir().unwrap();
        let forge = Forge::open(temp.path()).await.unwrap();
        assert_eq!(forge.backend_kind(), BackendKind::default());
    }
}
```

### 2. Integration Tests

Located in `tests/` directories:

```rust
// forge_core/tests/pubsub_integration_tests.rs
#[tokio::test]
async fn test_backend_connection_sqlite() {
    let temp = create_test_repo().await;
    let forge = Forge::open_with_backend(temp.path(), BackendKind::SQLite)
        .await
        .expect("Failed to open with SQLite backend");
    
    assert_eq!(forge.backend_kind(), BackendKind::SQLite);
}
```

### 3. Doc Tests

Embedded in documentation comments:

```rust
/// Opens a Forge instance on given codebase path.
///
/// # Example
/// ```
/// # async fn example() -> anyhow::Result<()> {
/// let forge = Forge::open("./my-project").await?;
/// # Ok(())
/// # }
/// ```
pub async fn open(path: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
    // ...
}
```

## Writing Tests

### Test Structure

```rust
// 1. Imports
use forge_core::{Forge, BackendKind};

// 2. Helper functions
async fn create_test_repo() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    // Create test files...
    temp
}

// 3. Test function
#[tokio::test]
async fn test_descriptive_name() {
    // Arrange
    let temp = create_test_repo().await;
    
    // Act
    let result = Forge::open(temp.path()).await;
    
    // Assert
    assert!(result.is_ok());
}
```

### Test Naming Conventions

- `test_<feature>_<scenario>` - General tests
- `test_<backend>_<feature>` - Backend-specific tests
- `test_<tool>_<operation>` - Tool integration tests

Examples:
- `test_sqlite_backend_basic`
- `test_native_v3_persistence`
- `test_magellan_symbol_lookup`

### Async Testing

All tests should use `#[tokio::test]`:

```rust
#[tokio::test]
async fn test_async_operation() {
    let forge = Forge::open("./project").await.unwrap();
    let result = forge.graph().find_symbol("main").await;
    assert!(result.is_ok());
}
```

### Temporary Directories

Always use `tempfile` for test directories:

```rust
use tempfile::TempDir;

#[tokio::test]
async fn test_with_temp_dir() {
    let temp = TempDir::new().unwrap();
    // Use temp.path() for database location
    let forge = Forge::open(temp.path()).await.unwrap();
    // Test...
}
```

## Feature Flag Testing

### Testing Individual Features

```bash
# Test each tool individually
cargo test -p forge_core --features magellan-sqlite
cargo test -p forge_core --features llmgrep-sqlite
cargo test -p forge_core --features mirage-sqlite
cargo test -p forge_core --features splice-sqlite

# Test V3 variants
cargo test -p forge_core --features magellan-v3
cargo test -p forge_core --features llmgrep-v3
cargo test -p forge_core --features mirage-v3
cargo test -p forge_core --features splice-v3
```

### Testing Feature Combinations

```bash
# Test mixed backends
cargo test -p forge_core --features "magellan-v3,llmgrep-sqlite"

# Test full stacks
cargo test --features full-sqlite
cargo test --features full-v3
```

### Feature Flag Test Example

```rust
#[cfg(all(feature = "magellan", feature = "sqlite"))]
#[tokio::test]
async fn test_magellan_sqlite_integration() {
    // Test only runs when both features are enabled
}
```

## Backend Testing

### Backend Parity Tests

Always test both backends:

```rust
#[tokio::test]
async fn test_sqlite_backend() {
    test_backend(BackendKind::SQLite).await;
}

#[tokio::test]
async fn test_native_v3_backend() {
    test_backend(BackendKind::NativeV3).await;
}

async fn test_backend(kind: BackendKind) {
    let temp = create_test_repo().await;
    let forge = Forge::open_with_backend(temp.path(), kind).await.unwrap();
    
    // Common test logic
    let symbols = forge.graph().find_symbol("test_function").await.unwrap();
    assert!(!symbols.is_empty());
}
```

### Persistence Tests

Critical for V3 backend:

```rust
#[tokio::test]
async fn test_database_persistence_native_v3() {
    let temp = create_test_repo().await;
    let db_path = temp.path().join(".forge").join("graph.v3");
    
    // Create and populate
    {
        let forge = Forge::open_with_backend(temp.path(), BackendKind::NativeV3)
            .await
            .unwrap();
        assert!(db_path.exists());
    }
    
    // Reopen and verify
    {
        let forge = Forge::open_with_backend(temp.path(), BackendKind::NativeV3)
            .await
            .expect("Failed to reopen V3 database");
        assert!(forge.analysis().storage().is_connected());
    }
}
```

## Pub/Sub Testing

### Event Subscription Tests

```rust
#[tokio::test]
async fn test_pubsub_events() {
    let forge = create_test_forge().await;
    
    // Subscribe to events
    let (id, rx) = forge.subscribe(SubscriptionFilter::all()).await.unwrap();
    
    // Trigger event (e.g., index code)
    forge.graph().index().await.unwrap();
    
    // Receive event
    let event = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(matches!(event, PubSubEvent::SnapshotCommitted { .. }));
    
    // Cleanup
    forge.unsubscribe(id).await.unwrap();
}
```

### Filter Testing

```rust
#[tokio::test]
async fn test_subscription_filter_matching() {
    let filter = SubscriptionFilter::nodes_only();
    
    assert!(filter.matches(&PubSubEvent::NodeChanged { node_id: 1, snapshot_id: 1 }));
    assert!(!filter.matches(&PubSubEvent::EdgeChanged { edge_id: 1, from_node: 1, to_node: 2, snapshot_id: 1 }));
}
```

## Continuous Integration

### CI Test Matrix

```yaml
# .github/workflows/test.yml
strategy:
  matrix:
    backend: [sqlite, native-v3]
    features: [minimal, tools, full]
    
include:
  - backend: sqlite
    features: minimal
    test_cmd: "cargo test -p forge_core --features sqlite"
  - backend: native-v3
    features: minimal
    test_cmd: "cargo test -p forge_core --features native-v3"
  - backend: sqlite
    features: tools
    test_cmd: "cargo test -p forge_core --features tools-sqlite"
  - backend: native-v3
    features: tools
    test_cmd: "cargo test -p forge_core --features tools-v3"
  - backend: native-v3
    features: full
    test_cmd: "cargo test --features full-v3"
```

### Pre-commit Checks

```bash
#!/bin/sh
# .git/hooks/pre-commit

cargo fmt -- --check || exit 1
cargo clippy --all-targets --all-features -- -D warnings || exit 1
cargo test --workspace --all-features || exit 1
```

## Test Coverage

### Generating Coverage

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --workspace --all-features --out Html

# View report
open tarpaulin-report.html
```

### Coverage Goals

- Unit tests: 80%+ coverage
- Integration tests: 60%+ coverage
- Critical paths: 90%+ coverage
- Error handling: 70%+ coverage

## Debugging Failed Tests

### Verbose Output

```bash
# Show all output
cargo test -- --nocapture

# Show on failure
cargo test -- --show-output

# Run single test with output
cargo test test_name -- --exact --nocapture
```

### Logging

```bash
# Enable tracing
cargo test --features tracing

# Set log level
RUST_LOG=debug cargo test
```

### GDB Debugging

```bash
# Run test under GDB
rust-gdb --args cargo test test_name -- --exact

# Set breakpoint
(gdb) break forge_core::storage::UnifiedGraphStore::open
(gdb) run
```

## Common Testing Issues

### Issue: Feature flag not found

**Solution:** Check feature flag spelling:
```bash
# Wrong
cargo test --features magellan

# Correct
cargo test --features magellan-sqlite
```

### Issue: Test hangs

**Solution:** Check for deadlocks in async code:
```rust
// Use timeout
let result = tokio::time::timeout(Duration::from_secs(5), operation).await;
```

### Issue: Database locked (SQLite)

**Solution:** Ensure tests close connections:
```rust
// Use RAII pattern
{
    let forge = Forge::open(path).await.unwrap();
    // Test...
} // Forge dropped, connection closed
```

## Best Practices

1. **Always test both backends** - SQLite and Native V3
2. **Use temporary directories** - Never use real project paths
3. **Clean up resources** - Unsubscribe from pub/sub, close connections
4. **Test error cases** - Not just happy path
5. **Keep tests fast** - Aim for <1 second per test
6. **Document test intent** - Clear test names and comments

---

For more information, see [Architecture](ARCHITECTURE.md) and [Development Workflow](DEVELOPMENT_WORKFLOW.md).
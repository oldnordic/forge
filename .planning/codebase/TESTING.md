# Testing Strategy

**Version**: 0.1.0
**Last Updated**: 2026-02-12

This document defines the testing standards and conventions for ForgeKit. Testing is mandatory for all code changes.

---

## Test Framework

### Primary Testing Tools

| Tool | Purpose | Usage |
|------|---------|--------|
| **tokio::test** | Async test runtime | `#[tokio::test]` for async functions |
| **Standard assert macros** | Assertions | `assert!`, `assert_eq!`, `assert_matches!` |
| **tempfile** | Temporary directories | `tempfile::tempdir()` for test isolation |

### Test Dependencies

```toml
[dev-dependencies]
tokio = { version = "1", features = ["test-util", "macros"] }
tempfile = "3"
```

---

## Test Organization

### Directory Structure

```
forge_core/
├── src/
│   ├── lib.rs
│   ├── graph/
│   │   └── mod.rs        # Contains #[cfg(test)] mod tests
│   └── ...
└── tests/                # Integration tests (when implemented)
    ├── common/
    │   └── mod.rs        # Shared test utilities
    └── fixtures/         # Test data
        └── ...
```

### Unit Test Placement

Unit tests live in the same file as the code, in a `#[cfg(test)]` module:

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

### Integration Test Placement

Integration tests go in `tests/` directory (planned for v0.2+).

---

## TDD Workflow (Mandatory)

ForgeKit enforces strict Test-Driven Development. All code changes must follow this workflow:

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
# Check the database schema
sqlite3 .forge/graph.db ".schema"

# Read existing source code
Read /home/feanor/Projects/forge/forge_core/src/graph/mod.rs

# Check existing tests
# Read the test module to understand patterns
```

### Step 2: PLAN

Document the architectural decision:

```markdown
## Decision: Add symbol search by kind

**Context**: Need to filter symbols by their type (function, struct, etc.)

**Proposed Solution**: Add `kind()` method to `SearchBuilder`

**Alternatives Considered**:
- Filter in Rust vs SQL: SQL is faster for large datasets

**Trade-offs**:
- Benefit: Type-safe filtering API
- Cost: Additional complexity in builder

**Implementation Notes**:
- Files: `forge_core/src/search/mod.rs`
- Tests: Add test for kind filtering
```

### Step 3: PROVE (Write Failing Test)

```rust
#[tokio::test]
async fn test_search_by_kind_filters_results() {
    // Given: A store with mixed symbol kinds
    let store = Arc::new(UnifiedGraphStore::open(
        tempfile::tempdir().unwrap()
    ).await.unwrap());
    let module = SearchModule::new(store);

    // When: We search with kind filter
    let builder = module.symbol("test")
        .kind(SymbolKind::Function);

    // Then: Should have kind filter set
    assert!(matches!(builder.kind_filter, Some(SymbolKind::Function)));
}
```

**Show the test fails:**

```bash
$ cargo test test_search_by_kind

running 1 test
test test_search_by_kind ... FAILED
```

### Step 4: IMPLEMENT

Write the minimum code to make the test pass.

### Step 5: VERIFY

```bash
$ cargo test test_search_by_kind

running 1 test
test test_search_by_kind ... ok

test result: ok. 1 passed; 0 failed
```

Then run `cargo check` to ensure compilation.

---

## Test Categories

### Unit Tests

**Scope**: Test individual functions and methods in isolation.

**Conventions**:
- Located in `#[cfg(test)]` modules within source files
- Test public API and private behavior
- Use mocks/stubs for external dependencies

**Example**:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_span_is_empty() {
        let span = Span { start: 10, end: 10 };
        assert!(span.is_empty());
    }

    #[tokio::test]
    async fn test_span_contains() {
        let span = Span { start: 10, end: 20 };
        assert!(span.contains(15));
        assert!(!span.contains(20));
        assert!(!span.contains(5));
    }
}
```

### Integration Tests

**Scope**: Test cross-module interactions (planned for v0.2+).

**Conventions**:
- Located in `tests/` directory
- Test real interactions between modules
- Use real filesystem when appropriate

**Example** (future):

```rust
// tests/integration/graph_search_tests.rs
use forge::Forge;

#[tokio::test]
async fn test_graph_search_integration() {
    let temp = tempfile::tempdir().unwrap();
    // Create real project structure
    // Open real Forge instance
    // Test interaction
}
```

### Benchmark Tests

**Scope**: Performance measurement for critical paths.

**Framework**: Use `criterion` for benchmarks (planned for v0.2+).

**Conventions**:
- Located in `benches/` directory
- Test operations that may be performance-critical
- Include before/after comparisons for optimizations

**Example** (future):

```rust
// benches/symbol_lookup.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_find_symbol(c: &mut Criterion) {
    let forge = setup_bench_forge().await;
    c.bench_function("find_symbol", |b| {
        b.iter(|| {
            black_box(forge.graph().find_symbol("main"))
        });
    });
}

criterion_group!(benches, bench_find_symbol);
criterion_main!(benches);
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
```

### AAA Pattern

Tests follow Arrange-Act-Assert structure:

```rust
#[tokio::test]
async fn test_example() {
    // ARRANGE: Set up test data
    let store = Arc::new(UnifiedGraphStore::open(
        tempfile::tempdir().unwrap()
    ).await.unwrap());
    let module = GraphModule::new(store);
    let expected = SymbolId(123);

    // ACT: Execute the function under test
    let result = module.find_symbol_by_id(expected).await.unwrap();

    // ASSERT: Verify the result
    assert_eq!(result.id, expected);
}
```

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

1. **All public API functions**
2. **All error variants**
3. **All enum variants**
4. **Edge cases** (empty inputs, boundary conditions)
5. **Async cancellation** (for long-running operations)

### What to Test

| Test Type | What to Cover |
|-----------|---------------|
| **Unit** | Individual function behavior, edge cases |
| **Integration** | Cross-module interactions, real I/O |
| **Error** | All error paths, error messages |
| **Async** | Cancellation, timeout scenarios |

---

## Test Utilities

### Common Test Module

Shared test utilities (planned for v0.2):

```rust
// tests/common/mod.rs
use forge::Forge;

/// Creates a test Forge instance with temporary storage.
pub async fn setup_test_forge() -> Forge {
    let temp = tempfile::tempdir().unwrap();
    Forge::open(temp.path()).await.unwrap()
}

/// Inserts a test symbol into the graph.
pub async fn insert_test_symbol(forge: &Forge, name: &str) -> SymbolId {
    // Implementation
}
```

### Fixtures

Test data files go in `tests/fixtures/`:

```
tests/fixtures/
├── simple_project/
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs
└── multi_file_project/
    └── ...
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

When exceeding limits:
- Extract to helper functions
- Use parameterized tests
- Split into multiple test files

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
    assert!(matches!(result, Err(ForgeError::SymbolNotFound(_)));
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

---

## Performance Testing

### When to Add Benchmarks

- Algorithm changes
- Data structure changes
- Optimization work
- When investigating performance

### Benchmark Conventions

- Use `criterion` framework
- Include realistic data sizes
- Document what's being measured
- Include comparison with baseline

---

## Documentation Examples

Tests can serve as documentation examples:

```rust
/// Finds a symbol by name.
///
/// # Examples
///
/// ```rust
/// use forge::Forge;
/// # let forge = unimplemented!();
/// let symbols = forge.graph().find_symbol("main").await?;
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

*Last updated: 2026-02-12*

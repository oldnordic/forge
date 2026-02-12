# Contributing to ForgeKit

**Thank you for your interest in contributing!**

This document outlines how to contribute to ForgeKit effectively.

---

## Code of Conduct

- Be respectful and constructive
- Provide reasoned arguments, not opinions
- Cite evidence (code, benchmarks, docs)
- Assume good faith

---

## Getting Started

### Prerequisites

- Rust 1.75+ (2021 edition)
- Git
- SQLite 3.38+ (for SQLite backend)

### Clone and Build

```bash
git clone https://github.com/oldnordic/forge.git
cd forge
cargo build
cargo test
```

### Development Setup

```bash
# Install pre-commit hooks (optional)
./scripts/install-hooks.sh

# Set up git aliases
git config alias.test-all '!cargo test --workspace'
```

---

## Development Workflow

### 1. Create an Issue

Before starting work:

1. Check existing issues
2. Create detailed issue with:
   - Problem statement
   - Proposed solution
   - Alternatives considered
   - Testing approach

### 2. Create a Branch

```bash
git checkout main
git pull
git checkout -b feature/your-feature-name
# or
git checkout -b fix/issue-number
```

### 3. Follow the Development Workflow

See [DEVELOPMENT_WORKFLOW.md](DEVELOPMENT_WORKFLOW.md) for mandatory workflow steps.

Summary:
1. UNDERSTAND (read code/schema)
2. PLAN (document decision)
3. PROVE (write failing test)
4. IMPLEMENT (write code)
5. VERIFY (ensure tests pass)

### 4. Commit Guidelines

Use Conventional Commits:

```
feat: add support for Go language
fix: resolve symbol collision in duplicate names
docs: update API reference with new methods
refactor: extract caching into separate module
test: add integration tests for cross-file rename
perf: optimize symbol lookup with prepared statements
```

Commit body should:
- Reference issue number
- Explain WHAT and WHY
- Cite relevant files

### 5. Create Pull Request

PR must include:
- **Description**: What and why
- **Links**: Related issues
- **Tests**: New or updated
- **Docs**: Updated if API changed
- **Transcript**: CLI output for CLI changes

---

## Code Standards

### Rust Style

- Use `rustfmt` (default settings)
- Use `clippy` with zero warnings
- 4-space indentation
- 100-character line limit (soft)

### File Organization

```
src/
├── mod.rs              # Module exports
├── types.rs            # Shared types (if any)
├── public.rs           # Public API
├── operations/         # Core operations
│   ├── mod.rs
│   ├── query.rs       # ≤300 LOC
│   └── update.rs      # ≤300 LOC
└── internal/           # Private implementation
    ├── mod.rs
    └── helper.rs      # ≤300 LOC
```

### Error Handling

```rust
// Use anyhow::Result for internal errors
pub fn internal_function() -> anyhow::Result<()> {
    // ...
}

// Use forge::Error for public API
pub fn public_api() -> Result<Output, ForgeError> {
    // ...
}
```

### Naming Conventions

| Category | Convention | Example |
|-----------|-------------|----------|
| Types | `PascalCase` | `GraphModule` |
| Functions | `snake_case` | `find_symbol` |
| Constants | `SCREAMING_SNAKE_CASE` | `MAX_CACHE_SIZE` |
| Traits | `PascalCase` + `Trait` suffix | `GraphBackend` |
| Async functions | `*_async` or explicit `async fn` | `query_async` |

---

## Testing Standards

### Test Coverage Goal

- Unit tests: 80%+ coverage
- Integration tests: All public APIs
- Edge cases: Explicit test coverage

### Test Structure

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::*;

    #[tokio::test]
    async fn test_specific_behavior() {
        // Given
        let fixture = setup_fixture().await;

        // When
        let result = fixture.sut().action().await;

        // Then
        assert!(result.is_ok());
        assert_eq!(result.unwrap().value, expected);
    }
}
```

### Test Fixtures

Place shared test utilities in:
- `forge_core/tests/fixtures/` for test data
- `forge_core/tests/common/mod.rs` for utilities

### Running Tests

```bash
# All tests
cargo test --workspace

# Specific member
cargo test -p forge_core

# Specific test
cargo test test_find_symbol

# With output
cargo test -- --nocapture

# Ignoring slow tests
cargo test -- --skip slow
```

---

## Documentation Standards

### Public API Documentation

Every public item must have rustdoc:

```rust
/// Finds a symbol by name.
///
/// # Arguments
///
/// * `name` - The symbol name to search for
///
/// # Returns
///
/// A vector of matching symbols, or error if query fails
///
/// # Examples
///
/// ```rust
/// let symbols = forge.graph().find_symbol("main")?;
/// ```
pub async fn find_symbol(&self, name: &str) -> Result<Vec<Symbol>>;
```

### Module Documentation

Each module should have a top-level comment:

```rust
//! Graph query operations.
//!
//! This module provides symbol and reference queries backed by the SQLiteGraph database.
//! All operations are deterministic and return structured results.
```

### Documentation Updates

When changing public API:
1. Update rustdoc comments
2. Update docs/API.md
3. Add example to docs/EXAMPLES.md if applicable

---

## Performance Guidelines

### Benchmark Requirements

- Add benchmark for any algorithm change
- Use criterion for benchmarking
- Include before/after comparison

### Benchmark Structure

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_symbol_find(c: &mut Criterion) {
    let forge = setup_bench_forge().await;
    c.bench_function("find_symbol", |b| {
        b.iter(|| {
            black_box(
                forge.graph().find_symbol("main")
            )
        });
    });
}

criterion_group!(benches, bench_symbol_find);
criterion_main!(benches);
```

### Running Benchmarks

```bash
# All benchmarks
cargo bench

# Specific benchmark
cargo bench -- symbol_find

# Compare with baseline
cargo bench -- --baseline main
```

---

## Pull Request Review Process

### Review Checklist

PR will be reviewed against:

- [ ] Follows development workflow
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Documentation updated
- [ ] Benchmarks included (if applicable)
- [ ] Backward compatible (or breaking change documented)
- [ ] Commits follow conventional format

### Review Criteria

| Criteria | Weight | Notes |
|----------|---------|--------|
| Correctness | Required | Tests pass, logic sound |
| Performance | Required | No regressions, benchmarks improve |
| Documentation | Required | Public API documented |
| Style | Required | rustfmt, clippy clean |
| Testing | Required | Coverage maintained/improved |

---

## Release Process

### Version Bump

Before release, update:

1. `Cargo.toml` versions in workspace
2. `CHANGELOG.md` with release notes
3. Tag version: `git tag v0.x.y`

### Release Checklist

- [ ] All tests pass
- [ ] Benchmarks run successfully
- [ ] Documentation complete
- [ ] CHANGELOG updated
- [ ] Version bumped
- [ ] Tag pushed

---

## Questions?

- Open an issue for discussion
- Ask in existing PR before submitting
- Check documentation first

---

*Last updated: 2025-12-30*

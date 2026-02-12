# Code Conventions

**Version**: 0.1.0
**Last Updated**: 2026-02-12

This document defines the coding standards and conventions for ForgeKit. All contributors must follow these guidelines to maintain consistency and quality across the codebase.

---

## Naming Conventions

| Category | Convention | Example |
|-----------|-------------|----------|
| **Types** | `PascalCase` | `GraphModule`, `SymbolId`, `ForgeError` |
| **Functions** | `snake_case` | `find_symbol`, `callers_of`, `reachable_from` |
| **Methods** | `snake_case` | `verify()`, `apply()`, `rollback()` |
| **Variables** | `snake_case` | `symbol_name`, `byte_start`, `file_path` |
| **Constants** | `SCREAMING_SNAKE_CASE` | `MAX_CACHE_SIZE`, `DEFAULT_TTL` |
| **Modules** | `snake_case` | `graph`, `search`, `cfg`, `edit` |
| **Traits** | `PascalCase` | `EditOperation`, `GraphBackend` |
| **Enums** | `PascalCase` | `SymbolKind`, `ReferenceKind`, `PathKind` |

### ID Type Conventions

Stable identifier types use `PascalCase` with `Id` suffix:
- `SymbolId` - Stable symbol identifier
- `BlockId` - CFG block identifier
- `PathId` - Execution path identifier

---

## File Organization

### Module Structure

```
src/
├── lib.rs              # Public API, module re-exports
├── types.rs           # Shared types (if any)
├── error.rs           # Error types
├── graph/             # Graph operations module
│   └── mod.rs        # Module implementation (<= 300 LOC)
├── search/            # Search operations module
│   └── mod.rs        # Module implementation (<= 300 LOC)
├── cfg/              # CFG operations module
│   └── mod.rs        # Module implementation (<= 300 LOC)
└── edit/             # Edit operations module
    └── mod.rs        # Module implementation (<= 300 LOC)
```

### File Size Limits

| Component | Limit | Rationale |
|------------|--------|------------|
| Core modules (forge_core) | 300 LOC | Maintainability, single responsibility |
| Runtime modules (forge_runtime) | 300 LOC | Focused behavior |
| Agent modules (forge_agent) | 300 LOC | Clear purpose |
| Test files | 500 LOC | Comprehensive coverage without bloat |

**When to exceed:**
- Only with explicit justification in comments
- Consider module extraction first
- File MUST be cohesive (single purpose)

### Module Declaration Order

```rust
// 1. Inner attributes (#![...])
// 2. Outer attributes (#[...])
// 3. Use statements
use std::sync::Arc;
use crate::error::{ForgeError, Result};

// 4. Module documentation (//!)
//! Module description.

// 5. Public types
pub struct PublicModule { ... }

// 6. Public traits
pub trait PublicTrait { ... }

// 7. Impl blocks
impl PublicModule { ... }

// 8. Tests
#[cfg(test)]
mod tests { ... }
```

---

## Rust Style Guidelines

### Edition and Version

- **Rust Edition**: 2021
- **Minimum Rust Version**: 1.75+
- **Workspace Resolver**: 3

### Formatting

- Use `rustfmt` with default settings
- 4-space indentation (no tabs)
- 100-character line limit (soft, not enforced)

### Linting

- Use `clippy` with zero warnings
- No `#[allow(...)]` without justification
- Fix warnings, don't suppress them

### Visibility

- Default to private
- Use `pub(crate)` for module-internal APIs
- Use `pub` for library public APIs
- Use `pub(super)` for parent module access

---

## Error Handling

### Error Type Hierarchy

```rust
/// Main error type for ForgeKit
#[derive(thiserror::Error, Debug)]
pub enum ForgeError {
    // Storage errors
    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Backend not available: {0}")]
    BackendNotAvailable(String),

    // Query errors
    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),

    #[error("Invalid query: {0}")]
    InvalidQuery(String),

    // Edit errors
    #[error("Edit conflict in {file:?} at {span:?}")]
    EditConflict { file: PathBuf, span: Span },

    #[error("Verification failed: {0}")]
    VerificationFailed(String),

    #[error("Policy violation: {0}")]
    PolicyViolation(String),

    // I/O and serialization
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    // Underlying graph errors
    #[error("Graph error: {0}")]
    Graph(#[from] anyhow::Error),
}
```

### Error Conversion Patterns

| Context | Error Type | Usage |
|----------|-------------|--------|
| **Internal functions** | `anyhow::Result` | `pub fn internal() -> anyhow::Result<()>` |
| **Public API** | `forge::Result<T>` | `pub fn api() -> Result<T>` |
| **Library crates** | `thiserror` enum | Define domain-specific errors |
| **Application crates** | `anyhow`/`eyre` allowed | For top-level error handling |

### Result Type Alias

```rust
/// Type alias for Result with ForgeError
pub type Result<T> = std::result::Result<T, ForgeError>;
```

---

## Async Patterns

### Runtime

- **Async Runtime**: Tokio (full features)
- **Test Utilities**: `tokio::test`, `tokio::test-util`

### Async Function Signatures

```rust
// Public async API
pub async fn find_symbol(&self, name: &str) -> Result<Vec<Symbol>> {
    // Implementation
}

// Async with lifetime
pub async fn search_pattern<'a>(&self, pattern: &'a str) -> Result<Vec<Symbol>> {
    // Implementation
}
```

### Yield Points

For long-running CPU-bound tasks in async context:

```rust
use tokio::task::yield_now;

pub async fn expensive_operation(&self) -> Result<()> {
    // Do some work
    yield_now().await;  // Yield to runtime
    // Do more work
    Ok(())
}
```

### Cancellation

- Design for cooperative cancellation
- Use `tokio::select!` for cancel-aware operations
- Clean up resources on cancel

---

## Documentation Standards

### Public API Documentation

Every public item must have rustdoc with:
- Brief description (first sentence, < 15 words)
- `# Arguments` section for parameters
- `# Returns` section for return values
- `# Examples` section with runnable code
- `# Errors` section for fallible operations

```rust
/// Finds a symbol by name.
///
/// Searches the code graph for symbols matching the given name.
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
/// ```rust,no_run
/// use forge::Forge;
/// # let forge = unimplemented!();
/// let symbols = forge.graph().find_symbol("main").await?;
/// ```
///
/// # Errors
///
/// - `ForgeError::DatabaseError` if the query fails
/// - `ForgeError::BackendNotAvailable` if backend is not configured
pub async fn find_symbol(&self, name: &str) -> Result<Vec<Symbol>> {
    // ...
}
```

### Module Documentation

Each module should have a top-level `//!` comment:

```rust
//! Graph module - Symbol and reference queries.
//!
//! This module provides access to code graph for querying symbols,
//! finding references, and running graph algorithms.
//!
//! # Examples
//!
//! ```rust,no_run
//! use forge::Forge;
//! let graph = forge.graph();
//! let symbols = graph.find_symbol("main").await?;
//! ```
```

### Documentation Requirements

| Item | Documentation Required |
|------|---------------------|
| `pub struct` | Yes, with examples |
| `pub enum` | Yes, with variant descriptions |
| `pub fn` | Yes, with arguments/returns/errors |
| `pub trait` | Yes, with method documentation |
| `pub mod` | Yes, module-level overview |
| `pub const` | Yes, with meaning/usage |
| Private items | No, unless complex |

### First Sentence Convention

The first sentence of documentation must be brief (< 15 words):
- Use plain text, no markup
- End with a period
- Describe what, not how

**Bad**: "This function provides the capability to locate symbols within the codebase."

**Good**: "Finds a symbol by name."

---

## Type System Patterns

### Newtype Pattern for IDs

```rust
/// Stable identifier for a symbol across reindexing
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SymbolId(pub i64);

impl std::fmt::Display for SymbolId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<i64> for SymbolId {
    fn from(id: i64) -> Self {
        Self(id)
    }
}
```

### Builder Pattern

For complex queries:

```rust
#[derive(Clone)]
pub struct SearchBuilder {
    module: SearchModule,
    name_filter: Option<String>,
    kind_filter: Option<SymbolKind>,
    limit: Option<usize>,
}

impl SearchBuilder {
    pub fn kind(mut self, kind: SymbolKind) -> Self {
        self.kind_filter = Some(kind);
        self
    }

    pub fn limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }

    pub async fn execute(self) -> Result<Vec<Symbol>> {
        // ...
    }
}
```

### Trait for Operations

```rust
pub trait EditOperation: Sized {
    type Output;

    fn verify(self) -> Result<Self>;
    fn preview(self) -> Result<Diff>;
    fn apply(self) -> Result<Self::Output>;
    fn rollback(self) -> Result<()>;
}
```

---

## Dependencies

### Allowed Dependencies

| Category | Crates |
|----------|--------|
| **Async runtime** | `tokio` (full features) |
| **Error handling** | `anyhow`, `thiserror` |
| **Serialization** | `serde`, `serde_json` |
| **Graph backend** | `sqlitegraph` (optional) |
| **Testing** | `tempfile` (dev-only) |

### Adding Dependencies

Before adding a dependency:
1. Check if existing dependency can suffice
2. Prefer minimal, well-maintained crates
3. Document why the dependency is needed
4. Update `Cargo.toml` with version justification

---

## Anti-Patterns (Prohibited)

| Don't Do | Correct Approach |
|------------|-------------------|
| `grep "function"` | `magellan find --name "function"` |
| `cat file.rs` | `Read /path/to/file.rs` |
| Assume schema | `sqlite3 .forge/graph.db ".schema"` first |
| `#[allow(...)]` | Fix the warning |
| `TODO/FIXME` | Do it now or create issue |
| Comment out code | Delete or fix properly |
| String search for spans | Use graph-provided byte spans |
| Hardcode paths | Use `PathBuf` from config |

---

## Testing Conventions

See `TESTING.md` for complete testing guidelines. Key points:

- Write tests before implementation (TDD)
- Use AAA pattern (Arrange, Act, Assert)
- Async tests use `#[tokio::test]`
- Test files in `#[cfg(test)]` modules
- Use `tempfile` for temporary test data

---

## Performance Considerations

### Cloning vs Arc

- Use `Arc` for shared stores
- Clone small types (`SymbolId`, `Span`) freely
- Clone modules only if they contain `Arc` handles

### Prepared Statements

When using SQLite queries, use prepared statements for performance:

```rust
// Prepare once, reuse many times
let stmt = db.prepare("SELECT * FROM symbols WHERE name = ?")?;
stmt.execute(["symbol_name"])?;
```

---

*Last updated: 2026-02-12*

# Code Conventions

**Analysis Date:** 2025-02-13
**Version:** 0.1.0

This document defines the coding standards and conventions for ForgeKit. All contributors must follow these guidelines to maintain consistency and quality across the codebase.

---

## Naming Conventions

| Category | Convention | Example | Location |
|-----------|-------------|---------|------------|
| **Types** | `PascalCase` | `GraphModule`, `SymbolId`, `ForgeError` | Throughout |
| **Functions** | `snake_case` | `find_symbol`, `callers_of`, `reachable_from` | All functions |
| **Methods** | `snake_case` | `verify()`, `apply()`, `rollback()` | All impl blocks |
| **Variables** | `snake_case` | `symbol_name`, `byte_start`, `file_path` | All local vars |
| **Constants** | `SCREAMING_SNAKE_CASE` | `MAX_CACHE_SIZE`, `DEFAULT_TTL` | Global consts |
| **Modules** | `snake_case` | `graph`, `search`, `cfg`, `edit` | `src/` directory |
| **Traits** | `PascalCase` | `EditOperation`, `GraphBackend` | Trait definitions |
| **Enums** | `PascalCase` | `SymbolKind`, `ReferenceKind`, `PathKind` | Enum definitions |
| **Type Parameters** | `Single uppercase` | `K`, `V`, `T` | Generics |

### ID Type Conventions

Stable identifier types use `PascalCase` with `Id` suffix:
- `SymbolId` - Stable symbol identifier (wrapper around `i64`)
- `BlockId` - CFG block identifier (wrapper around `i64`)
- `PathId` - Execution path identifier (16-byte BLAKE3 hash)

ID types are newtype wrappers with purposeful methods:

```rust
/// Stable identifier for a symbol across reindexing
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SymbolId(pub i64);

impl std::fmt::Display for SymbolId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
```

### Enum Variant Naming

Enum variants use `PascalCase` with no redundant prefix:

```rust
/// Good - concise variants
pub enum PathKind {
    Normal,
    Error,
    Degenerate,
    Infinite,
    Switch,
}

/// Bad - redundant prefix
pub enum PathKind {
    PathKindNormal,
    PathKindError,
    // Don't do this
}
```

---

## File Organization

### Module Structure

```
src/
├── lib.rs              # Public API, module re-exports, top-level types
├── types.rs           # Shared types used across modules
├── error.rs           # Error types for crate
├── graph/             # Graph operations module
│   └── mod.rs        # Module implementation (<= 300 LOC)
├── search/            # Search operations module
│   └── mod.rs        # Module implementation (<= 300 LOC)
├── cfg/              # CFG operations module
│   └── mod.rs        # Module implementation (<= 300 LOC)
├── edit/             # Edit operations module
│   └── mod.rs        # Module implementation (<= 300 LOC)
├── analysis/          # Combined operations
│   └── mod.rs        # Module implementation (<= 300 LOC)
├── storage/           # Storage abstraction
│   └── mod.rs        # Module implementation (<= 300 LOC)
├── watcher.rs         # File watching (can exceed 300 LOC due to async)
├── indexing.rs        # Incremental indexing (can exceed 300 LOC)
├── cache.rs           # Query caching (can exceed 300 LOC)
├── pool.rs           # Connection pooling (can exceed 300 LOC)
└── runtime.rs         # Runtime orchestration (can exceed 300 LOC)
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

**Enforcement:** `docs/DEVELOPMENT_WORKFLOW.md` mandates these limits.

### Module Declaration Order

Within each source file, follow this structure:

```rust
// 1. Inner attributes (#![...])
// 2. Outer attributes (#[...])
// 3. Use statements (std, crates, re-exports, local)
// 4. Module documentation (//!)
// 5. Public types
// 6. Public traits
// 7. Impl blocks
// 8. Tests

//! Module doc comment - describes purpose and usage.

use std::sync::Arc;
use std::collections::HashMap;

use crate::error::{ForgeError, Result};
use crate::types::{SymbolId, BlockId};

/// Public struct with documentation.
pub struct PublicModule { ... }

/// Public trait with documentation.
pub trait PublicTrait { ... }

impl PublicModule { ... }

#[cfg(test)]
mod tests {
    use super::*;
    // Test code
}
```

### Re-exports Pattern

Top-level types are re-exported from `lib.rs`:

```rust
// Re-export commonly used types
pub use error::{ForgeError, Result};
pub use types::{SymbolId, BlockId, CfgBlock, CfgBlockKind, Location, Span};
```

---

## Rust Style Guidelines

### Edition and Version

- **Rust Edition**: 2021
- **Minimum Rust Version**: 1.75+
- **Workspace Resolver**: 3 (see `Cargo.toml`)

### Formatting

- Use `rustfmt` with default settings
- 4-space indentation (no tabs)
- 100-character line limit (soft, not enforced)
- Run `cargo fmt` before committing

### Linting

- Use `clippy` with zero warnings
- No `#[allow(...)]` without justification
- Fix warnings, don't suppress them
- Run `cargo clippy --all-targets` before committing

### Visibility

Default to private. Use visibility modifiers minimally:

| Modifier | Use Case | Example |
|----------|-----------|---------|
| `pub(crate)` | Module-internal APIs | `pub(crate) fn internal_helper()` |
| `pub` | Library public APIs | `pub fn api_function()` |
| `pub(super)` | Parent module access | `pub(super) fn shared_with_parent()` |
| (private) | Default | `fn helper()` |

### Module Visibility

Modules are declared in `lib.rs` with clear public/private distinction:

```rust
// Public API modules
pub mod storage;
pub mod graph;
pub mod search;
pub mod cfg;
pub mod edit;
pub mod analysis;

// Runtime layer modules (still public but separate phase)
pub mod watcher;
pub mod indexing;
pub mod cache;
pub mod pool;
pub mod runtime;
```

---

## Error Handling

### Error Type Hierarchy

ForgeKit uses `thiserror` for structured error types:

```rust
/// Main error type for ForgeKit
#[derive(thiserror::Error, Debug)]
pub enum ForgeError {
    // Storage errors
    #[error("Database error: {0}")]
    DatabaseError(String),

    /// Symbol could not be found.
    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),

    /// Invalid query syntax or parameters.
    #[error("Invalid query: {0}")]
    InvalidQuery(String),

    /// Edit conflict detected.
    #[error("Edit conflict in {file:?} at {span:?}")]
    EditConflict {
        file: PathBuf,
        span: Span,
    },

    /// Pre-commit verification failed.
    #[error("Verification failed: {0}")]
    VerificationFailed(String),

    /// Policy constraint violated.
    #[error("Policy violation: {0}")]
    PolicyViolation(String),

    /// Requested backend is not available.
    #[error("Backend not available: {0}")]
    BackendNotAvailable(String),

    /// CFG not available for the requested function.
    #[error("CFG not available for symbol: {0:?}")]
    CfgNotAvailable(SymbolId),

    /// Path enumeration overflow (too many paths).
    #[error("Path overflow for symbol: {0:?}")]
    PathOverflow(SymbolId),

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Error from underlying sqlitegraph.
    #[error("Graph error: {0}")]
    Graph(#[from] anyhow::Error),

    /// External tool execution error (Magellan, LLMGrep, etc.).
    #[error("Tool error: {0}")]
    ToolError(String),
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

Every crate defines a Result type alias:

```rust
/// Type alias for Result with ForgeError
pub type Result<T> = std::result::Result<T, ForgeError>;
```

### Error Propagation

Use `?` operator for error propagation:

```rust
pub async fn find_symbol(&self, name: &str) -> Result<Vec<Symbol>> {
    let facts = self.inner
        .symbols_in_file(path)
        .map_err(|e| ForgeError::Graph(e.into()))?;

    Ok(facts.into_iter().map(|f| convert_fact(f)).collect())
}
```

### Context on Errors

Add context to errors using `.map_err()`:

```rust
.map_err(|e| ForgeError::DatabaseError(
    format!("Failed to open database at {:?}: {}", db_path, e)
))
```

---

## Async Patterns

### Runtime

- **Async Runtime**: Tokio (full features)
- **Test Utilities**: `tokio::test`, `tokio::test-util`
- **Version**: 1.x with `features = ["full"]`

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

// Async with &mut self
pub async fn flush(&mut self) -> anyhow::Result<FlushStats> {
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

Used in: `forge_core/src/runtime.rs`

### Cancellation

- Design for cooperative cancellation
- Use `tokio::select!` for cancel-aware operations
- Clean up resources on cancel

Example pattern:
```rust
tokio::select! {
    _ = tokio::time::sleep(timeout) => {
        return Err(ForgeError::Timeout);
    }
    result = operation() => {
        return result;
    }
}
```

### Async Locking

Use `tokio::sync::RwLock` for async-safe locking:

```rust
use tokio::sync::RwLock;

pub struct QueryCache<K, V> {
    inner: Arc<RwLock<CacheInner<K, V>>>,
}

impl<K, V> QueryCache<K, V> {
    pub async fn get(&self, key: &K) -> Option<V> {
        let mut inner = self.inner.write().await;
        // ... access data
    }
}
```

Used in: `forge_core/src/cache.rs`

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
/// A vector of matching symbols, or error if query fails.
///
/// # Examples
///
/// ```rust,no_run
/// use forge_core::Forge;
/// # let forge = unimplemented!();
/// let symbols = forge.graph().find_symbol("main").await?;
/// # Ok::<(), anyhow::Error>(())
/// ```
///
/// # Errors
///
/// - `ForgeError::DatabaseError` if the query fails
/// - `ForgeError::SymbolNotFound` if no symbol exists
pub async fn find_symbol(&self, name: &str) -> Result<Vec<Symbol>> {
    // ...
}
```

### Module Documentation

Each module should have a top-level `//!` comment:

```rust
//! Graph module - Symbol and reference queries.
//!
//! This module provides access to the code graph for querying symbols,
//! finding references, and running graph algorithms.
//!
//! # Examples
//!
//! ```rust,no_run
//! use forge_core::Forge;
//! let forge = Forge::open("./project").await?;
//! let symbols = forge.graph().symbols_in_file("src/main.rs")?;
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

### Doc Tests

Examples in documentation should be runnable when possible:

```rust
/// # Examples
///
/// ```rust,no_run
/// use forge_core::Forge;
/// let forge = Forge::open("./project").await?;
/// # Ok::<(), anyhow::Error>(())
/// ```
```

Run doc tests with:
```bash
cargo test --doc
```

---

## Type System Patterns

### Newtype Pattern for IDs

Wrapper types provide type safety and implement useful traits:

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

Used in: `forge_core/src/types.rs`

### Builder Pattern

For complex queries, use builder pattern with fluent API:

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

Used in: `forge_core/src/cfg/mod.rs` (PathBuilder)

### Trait for Operations

Define traits for common operations:

```rust
pub trait EditOperation: Sized {
    type Output;

    fn verify(self) -> Result<Self>;
    fn preview(self) -> Result<Diff>;
    fn apply(self) -> Result<Self::Output>;
    fn rollback(self) -> Result<()>;
}
```

Planned for: `forge_core/src/edit/mod.rs`

### Derive Macros

Always derive these traits when applicable:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Symbol {
    // ...
}
```

For newtype wrappers (IDs), also derive:
- `Copy`
- `Hash`
- `PartialOrd`
- `Ord`

### Generic Bounds

Use explicit bounds for generics:

```rust
pub struct QueryCache<K, V>
where
    K: Clone + Eq + std::hash::Hash + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    // ...
}
```

Used in: `forge_core/src/cache.rs`

---

## Dependency Management

### Allowed Dependencies

| Category | Crates |
|----------|--------|
| **Async runtime** | `tokio` (full features) |
| **Error handling** | `anyhow`, `thiserror` |
| **Serialization** | `serde`, `serde_json` |
| **Graph backend** | `sqlitegraph` (optional), `magellan` |
| **File watching** | `notify` (v8+) |
| **Hashing** | `blake3` |
| **Testing** | `tempfile`, `tokio/test-util` |

### Dependency Organization

```toml
[dependencies]
# Tool libraries
magellan = "2.2"
sqlitegraph = { version = "1.6", default-features = false, optional = true }

# Async runtime
tokio = { version = "1", features = ["full"] }

# Error handling
anyhow = "1"
thiserror = "1"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Other utilities
blake3 = "1"
notify = "8"

[dev-dependencies]
tokio = { version = "1", features = ["test-util", "macros"] }
tempfile = "3"
```

Used in: `forge_core/Cargo.toml`

### Adding Dependencies

Before adding a dependency:
1. Check if existing dependency can suffice
2. Prefer minimal, well-maintained crates
3. Document why dependency is needed in Cargo.toml comments
4. Update `STACK.md` with version justification

### Feature Flags

Use feature flags for optional functionality:

```toml
[features]
default = ["sqlite"]
sqlite = ["sqlitegraph/sqlite-backend"]
native-v2 = ["sqlitegraph/native-v2"]
```

---

## Concurrency Patterns

### Arc for Shared State

Use `Arc` for shared, thread-safe data:

```rust
#[derive(Clone)]
pub struct Runtime {
    store: Arc<UnifiedGraphStore>,
    watcher: Option<Watcher>,
    indexer: IncrementalIndexer,
    cache: QueryCache<String, String>,
    pool: Option<ConnectionPool>,
}
```

Used in: `forge_core/src/runtime.rs`

### RwLock for Interior Mutability

Use `tokio::sync::RwLock` for async-safe locking:

```rust
pub struct QueryCache<K, V> {
    inner: Arc<RwLock<CacheInner<K, V>>>,
}

// Read access
let inner = self.inner.read().await;

// Write access
let mut inner = self.inner.write().await;
```

Used in: `forge_core/src/cache.rs`

### Mutex for Simple Cases

Use `std::sync::Mutex` for non-async cases:

```rust
pub struct IncrementalIndexer {
    pending: Arc<tokio::sync::Mutex<HashSet<PathBuf>>>,
    deleted: Arc<tokio::sync::Mutex<HashSet<PathBuf>>>,
}
```

Used in: `forge_core/src/indexing.rs`

### Channels for Communication

Use `tokio::sync::mpsc` for async channels:

```rust
pub fn channel() -> (mpsc::UnboundedSender<WatchEvent>, mpsc::UnboundedReceiver<WatchEvent>) {
    mpsc::unbounded_channel()
}
```

Used in: `forge_core/src/watcher.rs`

### Semaphore for Limiting

Use `tokio::sync::Semaphore` for resource limiting:

```rust
#[derive(Clone)]
pub struct ConnectionPool {
    semaphore: Arc<Semaphore>,
    max_connections: usize,
}

pub async fn acquire(&self) -> anyhow::Result<ConnectionPermit> {
    let permit = self.semaphore.clone().acquire_owned().await?;
    Ok(ConnectionPermit { _permit: permit, ... })
}
```

Used in: `forge_core/src/pool.rs`

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

Example from `forge_core/src/runtime.rs`:
```rust
#[derive(Clone)]
pub struct Runtime {
    store: Arc<UnifiedGraphStore>,  // Arc for sharing
    // ...
}
```

### Prepared Statements

When using SQLite queries, use prepared statements:

```rust
// Prepare once, reuse many times
let stmt = db.prepare("SELECT * FROM symbols WHERE name = ?")?;
stmt.execute(["symbol_name"])?;
```

Planned for: Phase 05 (Storage Layer)

### Caching Strategy

Use LRU with TTL for query results:

```rust
let cache = QueryCache::new(1000, Duration::from_secs(300));
// max_size: 1000 entries
// ttl: 5 minutes
```

Used in: `forge_core/src/cache.rs`

---

## Anti-Patterns (Prohibited)

| Don't Do | Correct Approach |
|------------|-------------------|
| `grep "function"` | `magellan find --name "function"` |
| `cat file.rs` | `Read /absolute/path/to/file.rs` |
| Assume schema exists | `sqlite3 .forge/graph.db ".schema"` first |
| `#[allow(...)]` | Fix the warning |
| `TODO/FIXME` in prod | Do it now or create issue |
| Comment out code | Delete or fix properly |
| String search for spans | Use graph-provided byte spans |
| Hardcode paths | Use `PathBuf` from config |
| Skip reading source | Read before editing |
| Edit without tests | Write failing test first (TDD) |

---

## Module-Specific Conventions

### Graph Module (`forge_core/src/graph/mod.rs`)

- Wraps Magellan's `CodeGraph`
- Converts between Magellan and ForgeKit types
- Uses synchronous operations (Magellan is sync)

```rust
pub struct GraphModule {
    inner: magellan::graph::CodeGraph,
    db_path: PathBuf,
}

impl GraphModule {
    pub fn new<P: AsRef<Path>>(db_path: P) -> Self {
        // ...
    }
}
```

### Search Module (`forge_core/src/search/mod.rs`)

- Placeholder for LLMGrep integration
- Will use builder pattern for queries
- NOT YET IMPLEMENTED - Phase 08.2

### CFG Module (`forge_core/src/cfg/mod.rs`)

- Provides builder-based path enumeration
- Implements dominator analysis
- Detects natural loops
- Uses `TestCfg` for unit testing

```rust
pub fn paths(&self, function: SymbolId) -> PathBuilder {
    PathBuilder {
        module: self.clone(),
        function_id: function,
        normal_only: false,
        error_only: false,
        max_length: None,
        limit: None,
    }
}
```

### Edit Module (`forge_core/src/edit/mod.rs`)

- Placeholder for Splice integration
- Will provide span-safe refactoring
- NOT YET IMPLEMENTED - Phase 08.4

### Storage Module (`forge_core/src/storage/mod.rs`)

- Wraps SQLiteGraph backend
- Provides `UnifiedGraphStore` abstraction
- Uses feature flags for optional SQLite

```rust
#[cfg(feature = "sqlite")]
let graph = Some(Arc::new(sqlitegraph::SqliteGraph::open(&db_path)?));

#[cfg(not(feature = "sqlite"))]
let graph = None;
```

### Runtime Layer (`forge_core/src/runtime.rs`)

- Combines watcher, indexer, cache, pool
- Provides unified orchestration
- Uses async operations throughout

```rust
pub struct Runtime {
    pub store: Arc<UnifiedGraphStore>,
    pub watcher: Option<Watcher>,
    pub indexer: IncrementalIndexer,
    pub cache: QueryCache<String, String>,
    pub pool: Option<ConnectionPool>,
}
```

### Agent Layer (`forge_agent/src/lib.rs`)

- Implements deterministic AI loop
- Uses observe → constrain → plan → mutate → verify → commit
- Each phase is a separate module

---

## Code Review Checklist

Before submitting code, verify:

- [ ] All tests pass (`cargo test --workspace`)
- [ ] No clippy warnings (`cargo clippy --all-targets`)
- [ ] Code is formatted (`cargo fmt`)
- [ ] Public API has documentation
- [ ] Examples compile and run
- [ ] File size limits respected
- [ ] No TODO/FIXME in production code
- [ ] Error paths are tested
- [ ] Anti-patterns avoided

---

*Convention analysis: 2025-02-13*

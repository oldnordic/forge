# Architecture

**Version**: 0.1.0 (Design Phase)
**Created**: 2025-12-30
**Status**: DRAFT

---

## Overview

ForgeKit is a layered SDK that provides deterministic code intelligence through a unified API. The architecture follows strict separation of concerns with clear boundaries between components.

---

## Design Principles

### 1. Graph-First Design

The SQLiteGraph database is the authoritative source of truth.

```rust
// All operations flow through the graph
let forge = Forge::open("./repo")?;
let graph = forge.graph();  // Direct graph access
```

**Invariants:**
- Never assume code structure without querying
- All symbol locations are exact spans
- All references are graph-verified

### 2. Deterministic Operations

Every operation is verifiable and auditable.

```rust
forge.edit()
    .rename_symbol("OldName", "NewName")?
    .verify()?      // Pre-commit validation
    .apply()?       // Atomic mutation
```

**Invariants:**
- Span-safety is mandatory
- Rollback is always available
- No silent failures

### 3. Backend Agnosticism

The SDK works with any SQLiteGraph backend.

```rust
Forge::builder()
    .backend(ForgeBackend::Sqlite)    // Current
    // .backend(ForgeBackend::NativeV3)  // Future
    .build()?
```

**Invariants:**
- Backend selection is runtime configuration
- API is identical across backends
- Features may vary by backend capability

---

## System Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        Application Layer                         │
│  (IDE, CLI, Agent, Custom Tool)                               │
└────────────────────────────┬───────────────────────────────────────────┘
                         │
┌────────────────────────────┴───────────────────────────────────────────┐
│                       forge_core API                            │
│                                                                  │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌─────────┐ │
│  │   Graph    │  │  Search    │  │    CFG     │  │  Edit    │ │
│  │  Module    │  │  Module     │  │  Module    │  │  Module  │ │
│  └─────┬──────┘  └─────┬──────┘  └─────┬──────┘  └────┬────┘ │
│        │                 │                 │               │         │
└────────┼─────────────────┼─────────────────┼───────────────┼─────┘
         │                 │                 │               │
┌────────┼─────────────────┼─────────────────┼───────────────┼─────┐
│        ▼                 ▼                 ▼               ▼       │
│                    forge_core Internals                           │
│  ┌─────────────────────────────────────────────────────────┐        │
│  │              Unified Graph Store                    │        │
│  │  (wraps sqlitegraph with convenience methods)        │        │
│  └─────────────────────────────────────────────────────────┘        │
└──────────────────────────────┬────────────────────────────────────┘
                               │
┌──────────────────────────────┴────────────────────────────────────┐
│                     forge_runtime                         │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐       │
│  │  Indexer   │  │   Cache    │  │  Watcher   │       │
│  └────────────┘  └────────────┘  └────────────┘       │
└──────────────────────────────┬───────────────────────────────────┘
                               │
┌──────────────────────────────┴───────────────────────────────────┐
│                   sqlitegraph                             │
│  ┌─────────────────────────────────────────────────────┐        │
│  │         GraphBackend (trait)                    │        │
│  │  ┌────────────┐  ┌──────────────────┐       │        │
│  │  │  SQLite     │  │  Native V3      │       │        │
│  │  │  Backend    │  │  Backend (WIP)  │       │        │
│  │  └────────────┘  └──────────────────┘       │        │
│  └─────────────────────────────────────────────────────┘        │
└───────────────────────────────────────────────────────────────┘
```

---

## Module Structure

```
forge/
├── Cargo.toml                    # Workspace configuration
├── README.md                     # Project overview
├── LICENSE                       # GPL-3.0-or-later
│
├── forge_core/                   # Core library (required)
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs              # Public API, Forge type
│   │   ├── graph/              # Graph operations (Magellan)
│   │   │   ├── mod.rs
│   │   │   ├── symbols.rs       # Symbol queries
│   │   │   ├── references.rs    # Reference queries
│   │   │   ├── algorithms.rs    # Graph algorithms
│   │   │   └── types.rs       # Core types
│   │   ├── search/             # Semantic search (LLMGrep)
│   │   │   ├── mod.rs
│   │   │   ├── query.rs        # Search queries
│   │   │   ├── semantic.rs     # Semantic search
│   │   │   └── ast.rs          # AST queries
│   │   ├── cfg/                # CFG analysis (Mirage)
│   │   │   ├── mod.rs
│   │   │   ├── paths.rs        # Path enumeration
│   │   │   ├── dominators.rs   # Dominance analysis
│   │   │   ├── loops.rs        # Loop detection
│   │   │   └── types.rs       # CFG types
│   │   ├── edit/               # Span-safe editing (Splice)
│   │   │   ├── mod.rs
│   │   │   ├── patch.rs        # Patch operations
│   │   │   ├── rename.rs       # Rename operations
│   │   │   ├── delete.rs       # Delete operations
│   │   │   └── validation.rs   # Edit validation
│   │   ├── analysis/           # Combined operations
│   │   │   ├── mod.rs
│   │   │   ├── impact.rs       # Impact analysis
│   │   │   ├── dead_code.rs    # Dead code detection
│   │   │   └── cycles.rs       # Cycle detection
│   │   ├── storage/            # Storage abstraction
│   │   │   ├── mod.rs
│   │   │   ├── backend.rs      # Backend trait wrapper
│   │   │   └── transaction.rs  # Transaction management
│   │   └── error.rs           # Error types
│   └── tests/
│
├── forge_runtime/               # Runtime layer (optional)
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs
│   │   ├── indexer.rs          # Indexing orchestration
│   │   ├── watcher.rs         # File watching
│   │   ├── cache.rs           # Query caching
│   │   └── metrics.rs         # Performance metrics
│   └── tests/
│
├── forge_agent/                 # Agent layer (optional)
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs
│   │   ├── agent.rs           # Deterministic agent loop
│   │   ├── policy.rs          # Policy DSL and validation
│   │   ├── observe.rs         # Observation phase
│   │   ├── plan.rs           # Planning phase
│   │   ├── mutate.rs          # Mutation phase
│   │   ├── verify.rs          # Verification phase
│   │   └── commit.rs          # Commit phase
│   └── tests/
│
├── docs/
│   ├── ARCHITECTURE.md          # This document
│   ├── API.md                 # API reference
│   ├── PHILOSOPHY.md          # Design philosophy
│   ├── CONTRIBUTING.md         # Contribution guide
│   ├── DEVELOPMENT_WORKFLOW.md  # Development workflow
│   └── ROADMAP.md            # Project roadmap
│
├── tests/
│   ├── integration/
│   │   ├── graph_tests.rs
│   │   ├── search_tests.rs
│   │   ├── cfg_tests.rs
│   │   └── edit_tests.rs
│   └── fixtures/
│
└── .planning/
    ├── milestones/
    │   ├── v0.1-ROADMAP.md
    │   └── v0.1-REQUIREMENTS.md
    └── phases/
        ├── 01-project-organization/
        ├── 02-core-sdk/
        ├── 03-runtime-layer/
        └── 04-agent-layer/
```

---

## Data Flow

### Graph Query Flow

```
Application
    │
    ▼
forge.graph().find_symbol("main")
    │
    ▼
GraphModule -> UnifiedGraphStore
    │
    ▼
sqlitegraph::BackendClient
    │
    ▼
SQLite / Native V3 Backend
    │
    ▼
Structured Result
    │
    ▼
Application
```

### Edit Operation Flow

```
Application
    │
    ▼
forge.edit().rename_symbol("A", "B")
    │
    ├─► GraphModule: Find all references
    │         │
    │         ▼
    │    sqlitegraph: Query all refs
    │         │
    │         ▼
    │    Return: [(file, span), ...]
    │
    ├─► EditModule: Validate each edit
    │         │
    │         ▼
    │    tree-sitter: Parse and verify
    │         │
    │         ▼
    │    All valid? Continue / Abort
    │
    ├─► EditModule: Apply patches
    │         │
    │         ▼
    │    ropey: Apply text edits
    │         │
    │         ▼
    │    Write files
    │
    └─► Storage: Update graph
              │
              ▼
         sqlitegraph: Re-index affected files
              │
              ▼
         Transaction commit
```

---

## Component Interfaces

### Graph Module (Magellan Integration)

```rust
pub struct GraphModule {
    store: Arc<UnifiedGraphStore>,
}

impl GraphModule {
    // Symbol queries
    pub fn find_symbol(&self, name: &str) -> Result<Vec<Symbol>>;
    pub fn find_symbol_by_id(&self, id: SymbolId) -> Result<Symbol>;
    pub fn symbols_in_file(&self, path: &str) -> Result<Vec<Symbol>>;

    // Reference queries
    pub fn callers_of(&self, symbol: &str) -> Result<Vec<Reference>>;
    pub fn callees_of(&self, symbol: &str) -> Result<Vec<Reference>>;
    pub fn references(&self, symbol: &str) -> Result<Vec<Reference>>;

    // Graph algorithms
    pub fn reachable_from(&self, id: SymbolId) -> Result<Vec<SymbolId>>;
    pub fn cycles(&self) -> Result<Vec<Cycle>>;
    pub fn dead_code(&self, entry: SymbolId) -> Result<Vec<SymbolId>>;
}
```

### Search Module (LLMGrep Integration)

```rust
pub struct SearchModule {
    store: Arc<UnifiedGraphStore>,
}

impl SearchModule {
    // Symbol search
    pub fn symbol(&self, name: &str) -> SearchBuilder;
    pub fn pattern(&self, pattern: &str) -> SearchBuilder;

    // AST queries
    pub fn ast_query(&self, query: AstQuery) -> Result<Vec<AstNode>>;

    // Semantic search
    pub fn embedding_query(&self, query: &str) -> Result<Vec<Symbol>>;
}

pub struct SearchBuilder {
    // Filters
    pub fn kind(self, kind: SymbolKind) -> Self;
    pub fn file(self, path: &str) -> Self;
    pub fn limit(self, n: usize) -> Self;

    // Execution
    pub fn execute(self) -> Result<Vec<Symbol>>;
}
```

### CFG Module (Mirage Integration)

```rust
pub struct CfgModule {
    store: Arc<UnifiedGraphStore>,
}

impl CfgModule {
    // Path enumeration
    pub fn paths(&self, function: SymbolId) -> PathBuilder;
    pub fn path_count(&self, function: SymbolId) -> Result<usize>;

    // Dominance
    pub fn dominators(&self, function: SymbolId) -> Result<DominatorTree>;
    pub fn post_dominators(&self, function: SymbolId) -> Result<DominatorTree>;

    // Analysis
    pub fn loops(&self, function: SymbolId) -> Result<Vec<Loop>>;
    pub fn unreachable_blocks(&self, function: SymbolId) -> Result<Vec<BlockId>>;
}

pub struct PathBuilder {
    pub fn normal_only(self) -> Self;
    pub fn error_only(self) -> Self;
    pub fn max_length(self, n: usize) -> Self;
    pub fn execute(self) -> Result<Vec<Path>>;
}
```

### Edit Module (Splice Integration)

```rust
pub struct EditModule {
    store: Arc<UnifiedGraphStore>,
}

impl EditModule {
    // High-level operations
    pub fn rename_symbol(&self, old: &str, new: &str) -> RenameOperation;
    pub fn delete_symbol(&self, name: &str) -> DeleteOperation;
    pub fn inline_function(&self, name: &str) -> InlineOperation;
    pub fn extract_trait(&self, methods: Vec<SymbolId>) -> ExtractOperation;
}

pub trait EditOperation {
    type Output;

    fn verify(mut self) -> Result<Self>;
    fn preview(mut self) -> Result<Diff>;
    fn apply(mut self) -> Result<Self::Output>;
    fn rollback(mut self) -> Result<()>;
}
```

---

## Error Handling

### Error Hierarchy

```rust
pub enum ForgeError {
    // Storage errors
    DatabaseError(String),
    BackendNotAvailable(String),
    MigrationError(String),

    // Query errors
    SymbolNotFound(String),
    InvalidQuery(String),
    Timeout(Duration),

    // Edit errors
    EditConflict { file: String, span: Span },
    VerificationFailed { file: String, reason: String },
    RollbackFailed(String),

    // CFG errors
    CfgNotAvailable(SymbolId),
    PathOverflow(SymbolId),
    CycleDetected(Vec<SymbolId>),

    // Policy/Agent errors
    PolicyViolation(String),
    PreconditionFailed(String),
    PostconditionFailed(String),
}
```

### Error Recovery

| Error Type | Recovery Strategy |
|-------------|------------------|
| `DatabaseError` | Retry with exponential backoff |
| `SymbolNotFound` | Return empty result, not error |
| `EditConflict` | Abort transaction, suggest reindex |
| `VerificationFailed` | Rollback, return detailed diagnostics |
| `PolicyViolation` | Abort, return policy details |

---

## Performance Considerations

### Caching Strategy

```rust
pub struct CacheConfig {
    // Symbol query cache
    pub symbol_cache_size: usize,        // Default: 10,000

    // CFG path cache
    pub path_cache_size: usize,          // Default: 1,000

    // Search result cache
    pub search_cache_ttl: Duration,       // Default: 5 minutes

    // Cache invalidation
    pub invalidate_on_write: bool,        // Default: true
}
```

### Incremental Indexing

On file change:
1. Detect modified files (watcher)
2. Invalidate affected graph regions
3. Re-parse only changed files
4. Update adjacency relationships
5. Clear affected cache entries

### Query Optimization

- Use prepared statements for all queries
- Leverage SQLite indexes (edges_from, edges_to)
- Batch multi-hop traversals
- Cache frequently accessed symbols

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_symbol() {
        let forge = test_forge().await;
        let symbols = forge.graph().find_symbol("main").unwrap();
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "main");
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_rename_cross_file() {
    let temp = TempRepo::new_with_files(vec![
        ("src/lib.rs", "pub fn foo() {}"),
        ("src/main.rs", "use crate::foo; fn main() { foo(); }"),
    ]);

    let forge = Forge::open(temp.path()).await.unwrap();
    forge.edit()
        .rename_symbol("foo", "bar")
        .verify()
        .unwrap()
        .apply()
        .unwrap();

    assert!(temp.contains("src/lib.rs", "pub fn bar()"));
    assert!(temp.contains("src/main.rs", "use crate::bar;"));
}
```

### Benchmark Tests

```rust
fn bench_find_symbol(c: &mut Criterion) {
    let forge = setup_large_repo().await;
    c.bench_function("find_symbol", |b| {
        b.iter(|| {
            forge.graph().find_symbol("target_function").unwrap()
        });
    });
}
```

---

*Last updated: 2025-12-30*

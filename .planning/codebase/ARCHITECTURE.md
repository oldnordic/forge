# Architecture Overview

**Version**: 0.1.0 (Design Phase)
**Generated**: 2026-02-12

---

## Design Philosophy

### Core Principle: Graph-First Deterministic Code Intelligence

ForgeKit is built on the fundamental principle that the SQLiteGraph database is the authoritative source of truth for all code intelligence operations. Rather than relying on text search or LLM inference, ForgeKit queries a structured graph representation of code.

### Invariants

1. **Never assume code structure without querying** - All symbol locations, relationships, and properties must be retrieved from the graph database
2. **Spans are immutable** - Byte spans from AST parsing are the only reliable coordinates for editing
3. **Operations are transactions** - All mutations are atomic, with rollback capability
4. **Verification is mandatory** - All edits must be validated (syntax, types) before commit
5. **Local-first** - No code leaves the machine; all operations run locally

---

## System Layers

### Application Layer

The topmost layer consumed by IDEs, CLIs, AI agents, and custom tools.

**Consumers:**
- Language servers (LSP)
- AI coding agents
- CLI tools (forge command)
- Custom developer tools

**Interface:** `Forge` struct with module accessor methods

---

### forge_core API Layer

The unified API surface that exposes all code intelligence capabilities through a single entry point.

**Main Entry Point:**
```rust
pub struct Forge {
    store: UnifiedGraphStore,
}

impl Forge {
    pub fn graph(&self) -> GraphModule;
    pub fn search(&self) -> SearchModule;
    pub fn cfg(&self) -> CfgModule;
    pub fn edit(&self) -> EditModule;
    pub fn analysis(&self) -> AnalysisModule;
}
```

**Key Abstractions:**
- `ForgeBuilder` - Fluent configuration API
- `UnifiedGraphStore` - Backend-agnostic storage interface
- Module pattern - Each capability (graph, search, cfg, edit) accessed via dedicated module type

---

### Backend Layer (SQLiteGraph)

The persistence and query engine that powers all ForgeKit operations.

**Architecture:**
```
UnifiedGraphStore
    |
    +-- sqlitegraph::GraphBackend (trait)
        |
        +-- SqliteBackend (default)
        +-- NativeV3Backend (future, WIP)
```

**Database Location:** `.forge/graph.db` within the codebase

**Supported Backends:**
- SQLite (default, via `sqlite` feature)
- Native V3 binary file (via `native-v2` feature, in progress)

---

## Module Architecture

### Graph Module (`forge_core/src/graph/mod.rs`)

**Purpose:** Symbol and reference queries via Magellan integration

**Key Types:**
```rust
pub struct GraphModule {
    store: Arc<UnifiedGraphStore>,
}
```

**Operations:**
| Method | Purpose | Status |
|--------|---------|--------|
| `find_symbol(name)` | Find symbols by name | Stub |
| `find_symbol_by_id(id)` | Find symbol by stable ID | Stub |
| `callers_of(name)` | Find all callers of a symbol | Stub |
| `references(name)` | Find all references | Stub |
| `reachable_from(id)` | Reachability analysis | Stub |
| `cycles()` | Detect call graph cycles | Stub |

**Integration Point:** Magellan (v2.2.1) for graph algorithms

**Return Types:**
- `Symbol` - Symbol metadata with location, kind, parent
- `Reference` - Edge between symbols with kind and location
- `Cycle` - Detected cycle members

---

### Search Module (`forge_core/src/search/mod.rs`)

**Purpose:** Semantic code search via LLMGrep integration

**Key Types:**
```rust
pub struct SearchModule {
    store: Arc<UnifiedGraphStore>,
}

pub struct SearchBuilder {
    name_filter: Option<String>,
    kind_filter: Option<SymbolKind>,
    file_filter: Option<String>,
    limit: Option<usize>,
}
```

**Operations:**
| Method | Purpose | Status |
|--------|---------|--------|
| `symbol(name)` | Create symbol search builder | Stub |
| `pattern(pattern)` | Search for code pattern | Stub |
| `SearchBuilder::kind()` | Filter by symbol kind | Stub |
| `SearchBuilder::file()` | Filter by file path | Stub |
| `SearchBuilder::limit()` | Limit results | Stub |
| `SearchBuilder::execute()` | Execute search | Stub |

**Integration Point:** LLMGrep for semantic search

**Builder Pattern:** Fluent API for constructing queries with filters

---

### CFG Module (`forge_core/src/cfg/mod.rs`)

**Purpose:** Control flow graph analysis via Mirage integration

**Key Types:**
```rust
pub struct CfgModule {
    store: Arc<UnifiedGraphStore>,
}

pub struct PathBuilder {
    function_id: SymbolId,
    normal_only: bool,
    error_only: bool,
    max_length: Option<usize>,
    limit: Option<usize>,
}

pub struct DominatorTree {
    root: BlockId,
    dominators: HashMap<BlockId, BlockId>,
}

pub struct Loop {
    header: BlockId,
    blocks: Vec<BlockId>,
    depth: usize,
}
```

**Operations:**
| Method | Purpose | Status |
|--------|---------|--------|
| `paths(function)` | Create path enumeration builder | Stub |
| `PathBuilder::normal_only()` | Filter to success paths | Stub |
| `PathBuilder::error_only()` | Filter to error paths | Stub |
| `PathBuilder::max_length()` | Limit path length | Stub |
| `PathBuilder::execute()` | Enumerate paths | Stub |
| `dominators(function)` | Compute dominator tree | Stub |
| `loops(function)` | Detect natural loops | Stub |

**Integration Point:** Mirage for CFG algorithms

**Analysis Types:**
- `PathId` - BLAKE3 hash of block sequence
- `BlockId` - Stable CFG block identifier
- `PathKind` - Normal, Error, Degenerate, Infinite

---

### Edit Module (`forge_core/src/edit/mod.rs`)

**Purpose:** Span-safe refactoring via Splice integration

**Key Types:**
```rust
pub struct EditModule {
    store: Arc<UnifiedGraphStore>,
}

pub trait EditOperation {
    type Output;
    fn verify(self) -> Result<Self>;
    fn preview(self) -> Result<Diff>;
    fn apply(self) -> Result<Self::Output>;
    fn rollback(self) -> Result<()>;
}

pub struct RenameOperation {
    module: EditModule,
    old_name: String,
    new_name: String,
    verified: bool,
}

pub struct Diff {
    file_path: PathBuf,
    original: String,
    modified: String,
}
```

**Operations:**
| Method | Purpose | Status |
|--------|---------|--------|
| `rename_symbol(old, new)` | Create rename operation | Stub |
| `delete_symbol(name)` | Create delete operation | Stub |
| `RenameOperation::verify()` | Verify operation safety | Stub |
| `RenameOperation::preview()` | Preview changes | Stub |
| `RenameOperation::apply()` | Apply mutation | Stub |
| `RenameOperation::rollback()` | Rollback changes | Stub |

**Integration Point:** Splice (v2.5.0) for span-safe editing

**Workflow:** verify -> preview -> apply -> rollback (if needed)

**Result Types:**
- `RenameResult` - Files modified, references updated count
- `DeleteResult` - Files modified, references removed count

---

### Analysis Module (`forge_core/src/analysis/mod.rs`)

**Purpose:** Combined operations using multiple modules

**Key Types:**
```rust
pub struct AnalysisModule {
    graph: GraphModule,
    cfg: CfgModule,
    edit: EditModule,
}

pub struct ImpactAnalysis {
    affected_symbols: Vec<SymbolId>,
    affected_files: Vec<PathBuf>,
    radius: usize,
}
```

**Operations:**
| Method | Purpose | Status |
|--------|---------|--------|
| `impact_radius(symbol)` | Analyze change impact | Stub |
| `unused_functions(entries)` | Find dead code | Stub |
| `circular_dependencies()` | Detect cycles | Delegates to graph |

**Composition Pattern:** Combines graph, CFG, and edit modules for high-level analyses

---

### Storage Module (`forge_core/src/storage/mod.rs`)

**Purpose:** Backend-agnostic storage abstraction

**Key Types:**
```rust
pub struct UnifiedGraphStore {
    codebase_path: PathBuf,
    db_path: PathBuf,
}
```

**Current State:** Placeholder implementation for v0.1

**Planned v0.2:**
- Direct SQLiteGraph integration
- Connection pooling
- Transaction management
- Backend selection (SQLite vs Native V3)

---

## Type System

### Core Types (`forge_core/src/types.rs`)

**Identifier Types:**
```rust
pub struct SymbolId(i64);        // Stable across reindexing
pub struct BlockId(i64);         // CFG block identifier
pub struct PathId([u8; 16]);    // BLAKE3 hash of path
```

**Location Types:**
```rust
pub struct Location {
    file_path: PathBuf,
    byte_start: u32,
    byte_end: u32,
    line_number: usize,
}

pub struct Span {
    start: u32,   // Inclusive
    end: u32,     // Exclusive
}
```

**Symbol Classification:**
```rust
pub enum SymbolKind {
    Function, Method, Struct, Enum, Trait, Impl,
    Module, TypeAlias, Constant, Static,
    Parameter, LocalVariable, Field,
    Macro, Use,
}

pub enum ReferenceKind {
    Call, Use, TypeReference,
    Inherit, Implementation, Override,
}
```

**Path Classification:**
```rust
pub enum PathKind {
    Normal,      // Returns successfully
    Error,       // Returns error or panics
    Degenerate,  // Unreachable
    Infinite,    // Loop without exit
}
```

**Language Support:**
```rust
pub enum Language {
    Rust, Python, C, Cpp, Java,
    JavaScript, TypeScript, Go,
    Unknown(String),
}
```

---

### Error Types (`forge_core/src/error.rs`)

**Error Hierarchy:**
```rust
pub enum ForgeError {
    DatabaseError(String),
    SymbolNotFound(String),
    InvalidQuery(String),
    EditConflict { file: PathBuf, span: Span },
    VerificationFailed(String),
    PolicyViolation(String),
    BackendNotAvailable(String),
    CfgNotAvailable(SymbolId),
    PathOverflow(SymbolId),
    Io(std::io::Error),
    Json(serde_json::Error),
    Graph(anyhow::Error),
}
```

**Result Type:**
```rust
pub type Result<T> = std::result::Result<T, ForgeError>;
```

---

## Data Flow

### Query Flow (Graph/Symbol Operations)

```
Application
    │
    │ forge.graph().find_symbol("main")
    ▼
GraphModule
    │
    │ find_symbol_by_id() via UnifiedGraphStore
    ▼
UnifiedGraphStore
    │
    │ (Planned: sqlitegraph::BackendClient)
    ▼
SQLiteGraph Backend
    │
    │ Query symbols table
    ▼
Structured Result (Vec<Symbol>)
    │
    ▼
Application
```

### Edit Flow (Mutation Operations)

```
Application
    │
    │ forge.edit().rename_symbol("OldName", "NewName")
    ▼
RenameOperation created
    │
    ├─► .verify()
    │       │
    │       ▼
    │   GraphModule: Find all references
    │       │
    │       ▼
    │   Validate syntax/types (tree-sitter/LSP)
    │       │
    │       ▼
    │   All valid? Continue / Abort
    │
    ├─► .preview()
    │       │
    │       ▼
    │   Generate diff
    │
    ├─► .apply()
    │       │
    │       ▼
    │   Apply patches (Splice)
    │       │
    │       ▼
    │   Write files
    │       │
    │       ▼
    │   Re-index affected files
    │       │
    │       ▼
    │   Transaction commit
    │
    └─► (on failure) .rollback()
            │
            ▼
        Restore original state
```

### Search Flow (Semantic Queries)

```
Application
    │
    │ forge.search().symbol("Database").kind(Struct).execute()
    ▼
SearchBuilder (with filters)
    │
    │ Execute query
    ▼
SearchModule
    │
    │ (Planned: LLMGrep integration)
    ▼
SQLiteGraph Backend
    │
    │ Semantic search over symbols
    ▼
Structured Results (Vec<Symbol>)
    │
    ▼
Application
```

---

## Key Patterns

### Builder Pattern

**ForgeBuilder:**
```rust
let forge = Forge::builder()
    .path("./my-project")
    .database_path("./custom/graph.db")
    .cache_ttl(Duration::from_secs(300))
    .build()
    .await?;
```

**SearchBuilder:**
```rust
let results = forge.search()
    .symbol("Database")
    .kind(SymbolKind::Struct)
    .file("src/")
    .limit(10)
    .execute()
    .await?;
```

**PathBuilder:**
```rust
let paths = forge.cfg()
    .paths(symbol_id)
    .normal_only()
    .max_length(10)
    .limit(100)
    .execute()
    .await?;
```

### Module Access Pattern

Each capability is accessed through a dedicated module on the `Forge` instance:

```rust
let forge = Forge::open("./project").await?;

// Access modules
let graph = forge.graph();    // GraphModule
let search = forge.search();  // SearchModule
let cfg = forge.cfg();       // CfgModule
let edit = forge.edit();     // EditModule
let analysis = forge.analysis(); // AnalysisModule
```

### Operation Trait Pattern

Edit operations implement a common trait:

```rust
pub trait EditOperation {
    type Output;

    fn verify(self) -> Result<Self>;
    fn preview(self) -> Result<Diff>;
    fn apply(self) -> Result<Self::Output>;
    fn rollback(self) -> Result<()>;
}
```

This enables a consistent workflow for all edit types:
```rust
op.verify()?.preview()?.apply()?;
```

### Async/Await Pattern

All I/O operations are async:

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let forge = Forge::open("./project").await?;
    let symbols = forge.graph().find_symbol("main").await?;
    Ok(())
}
```

---

## Error Handling Strategy

### Error Propagation

- Internal functions return `forge_core::Result<T>` (alias for `std::result::Result<T, ForgeError>`)
- Public API boundaries convert to `anyhow::Result<T>` for flexibility
- Module-level errors are wrapped in `ForgeError` variants

### Recovery Strategies

| Error Type | Recovery |
|------------|-----------|
| `DatabaseError` | Retry with exponential backoff |
| `SymbolNotFound` | Return empty result set |
| `EditConflict` | Abort transaction, suggest reindex |
| `VerificationFailed` | Rollback, return diagnostics |
| `BackendNotAvailable` | Feature not implemented (v0.1 stubs) |

---

## Feature Flags

**Backend Selection:**
- `default = ["sqlite"]` - Use SQLite backend
- `sqlite` - Enable SQLiteGraph SQLite backend
- `native-v2` - Enable Native V3 binary backend (future)

**Per-Crate Features:**
```toml
[features]
default = ["sqlite"]
sqlite = ["sqlitegraph/sqlite-backend"]
native-v2 = ["sqlitegraph/native-v2"]
```

---

## Integration Points

### External Dependencies

| Component | Version | Purpose |
|-----------|---------|---------|
| sqlitegraph | 1.5 | Graph storage and algorithms |
| tokio | 1 | Async runtime |
| anyhow | 1 | Error handling at boundaries |
| serde/serde_json | 1 | Serialization |
| thiserror | 1 | Error derives |

### Tool Integrations (Planned)

| Tool | Purpose | Module |
|------|---------|--------|
| Magellan 2.2.1 | Graph algorithms | GraphModule |
| LLMGrep | Semantic search | SearchModule |
| Mirage | CFG analysis | CfgModule |
| Splice 2.5.0 | Span-safe editing | EditModule |
| tree-sitter | Parsing | All modules |

---

## Testing Strategy

### Unit Tests

Each module includes `#[cfg(test)]` tests for:
- Type validation
- Builder state correctness
- Basic operation flows

### Integration Tests (Planned)

`tests/integration/` directory for:
- Multi-file operations
- End-to-end workflows
- Cross-module interactions

### Test Utilities

- `tempfile` for temporary repositories
- Async test harness via `tokio::test`

---

## Development Roadmap by Layer

### v0.1 (Current)
- Core API design
- Type system
- Module stubs
- Basic storage placeholder

### v0.2 (Planned)
- SQLiteGraph integration
- Graph queries functional
- Search module functional
- Basic edit operations

### v0.3 (Planned)
- Runtime layer (forge_runtime)
- File watching
- Query caching

### v0.4 (Planned)
- Agent layer (forge_agent)
- Deterministic AI loop
- Policy enforcement

---

*Generated from codebase analysis on 2026-02-12*

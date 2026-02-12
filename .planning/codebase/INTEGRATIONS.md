# External Integrations

**Version**: 0.1.0
**Generated**: 2026-02-12

---

## Code Intelligence Tools

### Magellan (Graph Indexing)
- **Crate**: `magellan` v2.2.1
- **Purpose**: Symbol and reference queries, call graph operations
- **Integration Points**:
  - `forge_core::graph` module wraps Magellan operations
  - `GraphModule` provides symbol lookup via `find_symbol()`, `symbols_in_file()`
  - `GraphModule` provides reference queries via `callers_of()`, `callees_of()`
  - Graph algorithms: cycles, reachability, dead code detection

### LLMGrep (Semantic Search)
- **Crate**: `llmgrep` latest
- **Purpose**: Semantic code search using embeddings and AST-aware queries
- **Integration Points**:
  - `forge_core::search` module wraps LLMGrep operations
  - `SearchModule` provides semantic search via `symbol()`, `pattern()` methods
  - Supports filtering by kind, file, and limits

### Mirage (CFG Analysis)
- **Crate**: `mirage` latest
- **Purpose**: Control flow graph analysis for Rust code
- **Integration Points**:
  - `forge_core::cfg` module wraps Mirage operations
  - `CfgModule` provides path enumeration, dominance analysis, loop detection
  - Works on `symbol_id` to get CFG for specific functions

### Splice (Code Editing)
- **Crate**: `splice` v2.5.0
- **Purpose**: Span-safe code editing and refactoring
- **Integration Points**:
  - `forge_core::edit` module wraps Splice operations
  - `EditModule` provides rename, delete, extract operations
  - Validates edits before applying with tree-sitter

---

## Storage Backend

### SQLiteGraph
- **Crate**: `sqlitegraph` v1.5.0
- **Purpose**: Graph database backend, provides 35+ graph algorithms
- **Schema Location**: `.forge/graph.db`
- **Integration**:
  - `UnifiedGraphStore` wraps `sqlitegraph::GraphBackend`
  - Supports both SQLite and future Native V3 backends
  - Feature-gated: `sqlite` (default), `native-v3` (WIP)

### Backend Abstraction
```rust
pub enum ForgeBackend {
    Sqlite,
    NativeV3,  // Future, WIP
}
```

---

## Async Runtime

### Tokio
- **Crate**: `tokio` v1.49.0
- **Purpose**: Async runtime for all ForgeKit operations
- **Features**: `full`, `macros`, `rt-multi-thread`
- **Integration**: All `async fn` methods use Tokio runtime

---

## Error Handling

### anyhow
- **Crate**: `anyhow` v1.0.101
- **Purpose**: Error handling at API boundaries
- **Usage**: `anyhow::Result<T>` as return type for public APIs

### thiserror
- **Crate**: `thiserror` v1.0.69
- **Purpose**: Derive error types for libraries
- **Usage**: `#[derive(Error)]` on custom error enums

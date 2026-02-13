# External Integrations

**Analysis Date:** 2026-02-13

## Code Intelligence Tools

**Magellan (Graph Indexing):**
- Crate: `magellan` v2.2
- Purpose: Symbol and reference queries, call graph operations
- Integration: Library (not CLI subprocess)
  - `forge_core/src/graph/mod.rs` wraps `magellan::graph::CodeGraph`
  - `GraphModule::new(db_path)` opens CodeGraph directly
  - Methods: `symbols_in_file()`, `find_symbol()`, `references_to_symbol()`
  - Graph algorithms: `cycles`, `reachable`, `dead_code`
  - Stats: `count_files()`, `count_symbols()`, `count_references()`

**LLMGrep (Semantic Search):**
- Crate: `llmgrep` (via sqlitegraph integration)
- Purpose: Semantic code search using embeddings and AST-aware queries
- Integration: Library (via sqlitegraph, planned)
  - `forge_core/src/search/mod.rs` - `SearchModule` (placeholder)
  - Current: Returns empty results; full integration planned for v0.2+
  - Design: Supports filtering by symbol kind, file path, result limits

**Mirage (CFG Analysis):**
- Crate: `mirage` (via sqlitegraph integration)
- Purpose: Control flow graph analysis for Rust code
- Integration: Library (planned via sqlitegraph)
  - `forge_core/src/cfg/mod.rs` - `CfgModule` (test implementation)
  - Contains: `TestCfg` for unit testing without full Mirage integration
  - Methods: `paths()`, `dominators()`, `loops()`
  - Builder pattern: `PathBuilder` with `normal_only()`, `error_only()`, `max_length()`, `limit()`

**Splice (Code Editing):**
- Crate: `splice` v2.5.0
- Purpose: Span-safe code editing and refactoring
- Integration: Library (planned)
  - `forge_core/src/edit/mod.rs` - `EditModule` (stub)
  - Current: Returns placeholder `RenameResult`, `DeleteResult`
  - Planned: `rename_symbol()`, `delete_symbol()`, `EditOperation` trait
  - Validates edits before applying with tree-sitter

---

## Data Storage

**Databases:**
- SQLiteGraph v1.5-1.6
  - Connection: `.forge/graph.db` (auto-created)
  - Client: `sqlitegraph::SqliteGraph` wrapped in `UnifiedGraphStore`
  - Schema: Graph entities (symbols, references, CFG blocks)
  - Features: `sqlite-backend` (default), `native-v2` (WIP)

**File Storage:**
- `.forge/` directory (created automatically)
  - `graph.db` - Main SQLite database
  - Configurable via `ForgeBuilder::database_path()`

**Caching:**
- `QueryCache<K, V>` in `forge_core/src/cache.rs`
  - Strategy: LRU with FIFO eviction, TTL-based expiration
  - Default: 1000 entries, 300 second TTL
  - Thread-safety: `Arc<RwLock<CacheInner>>`

---

## Authentication & Identity

**Auth Provider:**
- None (local-first design)

**Implementation:**
- No authentication required for local graph operations
- Agent operations are deterministic loops without external auth

---

## Monitoring & Observability

**Error Tracking:**
- None (development phase)

**Logs:**
- `eprintln!` for warnings (see `forge_core/src/storage/mod.rs`)
- Standard Rust error propagation via `thiserror` and `anyhow`

---

## CI/CD & Deployment

**Hosting:**
- Not applicable (library/SDK, not hosted service)

**CI Pipeline:**
- None detected (no `.github/` directory)

---

## Environment Configuration

**Required env vars:**
- None (fully functional without environment variables)

**Secrets location:**
- No secrets required for local operation
- `.forge/` directory for database (auto-created)

**Configuration via code:**
- `ForgeBuilder::database_path()` - Custom database location
- `ForgeBuilder::cache_ttl()` - Cache time-to-live
- `RuntimeConfig` - Watch enabled, cache TTLs, max cache size

---

## Webhooks & Callbacks

**Incoming:**
- None (no HTTP server)

**Outgoing:**
- None (local-only operations)

---

## Integration Architecture

### Module Integration Pattern

```rust
// All external tools integrated via UnifiedGraphStore

// Storage layer wraps sqlitegraph
pub struct UnifiedGraphStore {
    #[cfg(feature = "sqlite")]
    graph: Option<Arc<sqlitegraph::SqliteGraph>>,
}

// Graph module wraps magellan
pub struct GraphModule {
    inner: magellan::graph::CodeGraph,
}
```

### Feature Flag Gating

```toml
# All crates use consistent feature flags
[features]
default = ["sqlite"]
sqlite = ["sqlitegraph/sqlite-backend"]
native-v2 = ["sqlitegraph/native-v2"]
```

### Cross-Crate Coordination

```toml
# forge_runtime depends on forge_core without enabling features
forge_core = { path = "../forge_core", default-features = false }

# Features are explicitly enabled when needed
forge_runtime = { path = "../forge_core", default-features = false, features = ["sqlite"] }
```

---

## Tool Status Summary

| Tool | Status | Integration Point | File |
|-------|--------|-------------------|-------|
| Magellan | Integrated (v2.2) | `forge_core/src/graph/mod.rs` | `GraphModule` |
| LLMGrep | Planned | `forge_core/src/search/mod.rs` | `SearchModule` |
| Mirage | Planned | `forge_core/src/cfg/mod.rs` | `CfgModule` |
| Splice | Planned | `forge_core/src/edit/mod.rs` | `EditModule` |
| sqlitegraph | Integrated (v1.5-1.6) | `forge_core/src/storage/mod.rs` | `UnifiedGraphStore` |

---

## Future Integration Points

**Phase 08.x:**
- Full LLMGrep integration for semantic search
- Full Mirage integration for CFG path enumeration
- Full Splice integration for span-safe editing

**Phase 16: Tool Integration:**
- Direct CLI tool integration points
- Subprocess execution fallback for missing tools

---

## API Boundaries

**Public API (`forge_core`):**
- `Forge::open(path)` - Opens graph database
- `forge.graph()` - Returns `GraphModule`
- `forge.search()` - Returns `SearchModule`
- `forge.cfg()` - Returns `CfgModule`
- `forge.edit()` - Returns `EditModule`
- `forge.analysis()` - Returns `AnalysisModule`

**Internal API (crate-private):**
- `UnifiedGraphStore` - Direct database access
- `QueryCache` - Query result caching
- `ConnectionPool` - Concurrent connection limiting

**Agent API (`forge_agent`):**
- `Agent::new(path)` - Creates agent instance
- `agent.observe(query)` - Gathers context
- `agent.constrain(obs, policies)` - Validates constraints
- `agent.plan(constrained)` - Generates execution steps
- `agent.mutate(plan)` - Applies changes
- `agent.verify(result)` - Validates results
- `agent.commit(result)` - Finalizes transaction

---

## Database Schema (via sqlitegraph)

**Tables:**
- `symbols` - Symbol definitions (functions, structs, enums, etc.)
- `references` - References between symbols (calls, uses, type refs)
- `cfg_blocks` - Control flow graph blocks
- `cfg_edges` - Control flow edges between blocks

**Introspection API:**
```rust
let introspection = graph.introspect()?;
// Access schema for raw queries
```

---

## Async Runtime Integration

**Tokio Integration:**
- All `async fn` methods use tokio runtime
- File watching: `notify` with tokio spawn
- Cache locking: `tokio::sync::RwLock`
- Connection pooling: `tokio::sync::Semaphore`

---

*Integration audit: 2026-02-13*

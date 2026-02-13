# Technology Stack

**Analysis Date:** 2026-02-13

## Languages

**Primary:**
- Rust Edition 2021 - All workspace crates (forge_core, forge_runtime, forge_agent)
  - Used for: Systems programming, performance-critical code intelligence operations
  - Key files:
    - `/home/feanor/Projects/forge/forge_core/src/lib.rs` - Main SDK entry point
    - `/home/feanor/Projects/forge/forge_core/src/graph/mod.rs` - Graph queries
    - `/home/feanor/Projects/forge/forge_agent/src/lib.rs` - Agent orchestration

**Secondary:**
- Markdown (v1) - Documentation (README.md, docs/ directory)
- SQLite (SQL dialect) - Database backend via sqlitegraph
- TOML - Configuration files (Cargo.toml workspace)

## Runtime

**Environment:**
- Rust 1.56+ (edition 2021)
- Async runtime: Tokio 1.x with "full" features

**Package Manager:**
- Cargo (workspace resolver v3)
- Lockfile: Not present (development phase)
- Workspace members: `forge_core`, `forge_runtime`, `forge_agent`

**Build Profiles:**

Release profile (configured in workspace `/home/feanor/Projects/forge/Cargo.toml`):
```toml
[profile.release]
opt-level = 3
codegen-units = 1
lto = "thin"
debug = false
panic = "abort"
```

Bench profile:
```toml
[profile.bench]
inherits = "release"
opt-level = 3
codegen-units = 1
lto = "thin"
debug = true
```

Test profile:
```toml
[profile.test]
opt-level = 2
```

## Frameworks

**Core:**

- **sqlitegraph** (version 1.5-1.6) - Graph database backend with 35+ algorithms
  - Purpose: Authoritative code graph storage
  - Features: `sqlite-backend` (default), `native-v2` (in development)
  - Used by: `forge_core`, `forge_runtime`, `forge_agent`
  - Integration point: `/home/feanor/Projects/forge/forge_core/src/storage/mod.rs` - `UnifiedGraphStore`

- **Tokio** (version 1, features = "full") - Async runtime
  - Purpose: Async I/O, task spawning, synchronization primitives
  - Used by: All workspace crates
  - Key usage: File operations (`tokio::fs`), watching (`notify`), synchronization (`tokio::sync::RwLock`, `tokio::sync::Semaphore`)

**Testing:**
- **tokio::test** - Async test framework with `test-util` and `macros` features
  - Configured in dev-dependencies across all workspace members
- **tempfile v3** - Temporary directory fixtures for unit tests
- Standard Rust `#[cfg(test)]` module pattern for per-module test suites

**Build/Dev:**

- **notify** (version 6 in forge_core, version 8 in forge_runtime) - File system watching
  - Purpose: Hot-reload for codebase changes during development
  - Integration point: `/home/feanor/Projects/forge/forge_core/src/watcher.rs`
  - Uses: `notify::RecommendedWatcher` with recursive mode

**CLI:**
- **clap** (version 4.4, features = "derive") - Command-line argument parsing
  - Used by: `/home/feanor/Projects/forge/forge_agent/src/cli.rs`
  - Purpose: Optional agent CLI interface

## Key Dependencies

**Critical Graph Infrastructure:**

| Package | Version | Purpose | Usage Location |
|----------|----------|---------|------------------|
| `sqlitegraph` | 1.5-1.6 | Graph database with 35+ algorithms | All crates via `UnifiedGraphStore` |
| `tokio` | 1 (full) | Async runtime, I/O, sync primitives | Throughout codebase |
| `anyhow` | 1 | Error handling, `anyhow::Result` | `forge_core/src/lib.rs`, agent operations |
| `thiserror` | 1 | Structured error derivation | `forge_core/src/error.rs`, `forge_agent/src/lib.rs` |
| `serde` | 1 (derive) | Serialization for types, configs | All crates for data structures |
| `serde_json` | 1 | JSON parsing/generation | Configuration, metadata |
| `rusqlite` | 0.31 (bundled) | SQLite database connector | Direct SQLite access for introspection |
| `magellan` | 2.2 | Code graph indexing, symbol queries | `forge_core/src/graph/mod.rs` |

**Specialized Libraries:**

| Package | Version | Purpose | Usage Location |
|----------|----------|---------|------------------|
| `similar` | 2 | Diff generation for edit operations | `forge_core/src/edit/mod.rs` |
| `blake3` | 1 | Hashing for stable IDs, path fingerprints | `forge_core/src/types.rs` (PathId), `forge_core/src/cfg/mod.rs` |
| `notify` | 6/8 | File system events, hot-reload | `forge_core/src/watcher.rs` |
| `chrono` | 0.4 | Timestamps, timekeeping | `forge_agent/src/lib.rs` |
| `tempfile` | 3 | Test fixtures, temporary directories | All crates in tests |

## Configuration

**Feature Flags:**

All workspace crates support consistent feature sets:

**`sqlite`** (default):
- Enables `sqlitegraph/sqlite-backend`
- Provides SQLite-backed graph storage
- Database location: `.forge/graph.db` within codebase

**`native-v2`**:
- Enables `sqlitegraph/native-v2`
- Developmental native binary file format
- Future replacement for SQLite backend (WIP)

**Environment:**
- Database path: `.forge/graph.db` (configurable via `ForgeBuilder`)
- No environment variables currently required for basic operation
- Cache TTL: 300 seconds default (configurable via `RuntimeConfig`)

**Build:**
- Workspace resolver: "3" (Cargo feature resolver v3)
- Rust edition: 2021
- License: GPL-3.0-or-later (see `/home/feanor/Projects/forge/LICENSE`)

## Platform Requirements

**Development:**
- Rust 1.56+ (edition 2021)
- Cargo with workspace support
- SQLite3 (for sqlite feature, bundled via rusqlite)
- tokio 1.x runtime (Linux, macOS, Windows support)

**Production:**
- Linux/macOS/Windows (Rust supports all)
- Database file storage in `.forge/` directory
- No external service dependencies required
- Static binary deployment target (single binary distribution)

**Target Platforms:**
- Linux (primary development platform)
- macOS (supported via tokio/notify)
- Windows (supported via bundled rusqlite)

---

## Module Dependency Graph

```
sqlitegraph
    │
    ├─→ UnifiedGraphStore (forge_core/src/storage/mod.rs)
    │           │
    │           ├─→ GraphModule (graph/mod.rs)
    │           ├─→ SearchModule (search/mod.rs)
    │           ├─→ CfgModule (cfg/mod.rs)
    │           └─→ EditModule (edit/mod.rs)
    │
    └─→ Runtime ──→ IncrementalIndexer (indexing.rs)
                                      └─→ Watcher (watcher.rs)

tokio
    │
    ├─→ async/await (throughout)
    ├─→ fs (file I/O)
    ├─→ sync::RwLock (cache.rs, observe.rs)
    ├─→ sync::Mutex (cache.rs, runtime.rs)
    ├─→ sync::Semaphore (pool.rs)
    └─→ time::sleep (tests)

thiserror
    │
    └─→ ForgeError derivation (error.rs)

anyhow
    │
    └─→ anyhow::Result (internal functions, runtime.rs)

serde
    │
    └─→ Symbol, Reference, Observation derives (types.rs, agent modules)

blake3
    │
    └─→ PathId hashing (cfg/mod.rs, types.rs)

notify
    │
    └─→ RecommendedWatcher (watcher.rs)
```

---

## Inter-Crate Dependencies

### forge_core (depends on)
```
sqlitegraph (optional, feature-gated)
tokio (full)
anyhow
serde (derive)
serde_json
thiserror
similar
blake3
notify
tempfile (dev)
```

### forge_runtime (depends on)
```
forge_core (path dependency, default-features = false)
sqlitegraph (optional, feature-gated)
tokio (full)
anyhow
serde (derive)
serde_json
notify (v8 - newer than forge_core)
tempfile (dev)
```

### forge_agent (depends on)
```
forge_core (path dependency, default-features = false)
sqlitegraph (optional, feature-gated)
tokio (full)
anyhow
serde (derive)
serde_json
thiserror
chrono
clap (derive)
tempfile (dev)
```

---

## External Tool Integration Points

The codebase is designed to integrate with external code intelligence tools:

### Magellan (Symbol Graph Queries)
- **Status**: External dependency, integrated via sqlitegraph
- **Purpose**: `find`, `refs`, `cycles`, `reachable`, `dead-code`
- **Integration**: `GraphModule::find_symbol()`, `callers_of()`, `reachable_from()`, `cycles()`
- **File**: `/home/feanor/Projects/forge/forge_core/src/graph/mod.rs`

### LLMGrep (Semantic Search)
- **Status**: External dependency, integrated via sqlitegraph
- **Purpose**: Semantic code search with filters
- **Integration**: `SearchModule::symbol()`, `pattern()`, `SearchBuilder` pattern
- **File**: `/home/feanor/Projects/forge/forge_core/src/search/mod.rs`
- **Note**: Currently returns empty results; full integration planned for v0.2+

### Mirage (CFG Analysis)
- **Status**: External dependency, integrated via sqlitegraph
- **Purpose**: Control flow graph, path enumeration, dominators, loops
- **Integration**: `CfgModule::paths()`, `dominators()`, `loops()`, `PathBuilder`
- **File**: `/home/feanor/Projects/forge/forge_core/src/cfg/mod.rs`
- **Note**: Contains test CFG implementation (`TestCfg`) for unit testing without full integration

### Splice (Code Editing)
- **Status**: External dependency, integration planned
- **Purpose**: Span-safe refactoring, rename operations
- **Integration**: `EditModule::rename_symbol()`, `delete_symbol()`, `EditOperation` trait
- **File**: `/home/feanor/Projects/forge/forge_core/src/edit/mod.rs`
- **Note**: Currently stub implementation; returns placeholder `RenameResult`, `DeleteResult`

---

## Development Toolchain

**Language Server:**
- rust-analyzer support through standard Rust edition 2021

**Linting:**
- No custom `.clippy.toml` detected
- Uses Clippy defaults for Rust 2021 edition

**Formatting:**
- No explicit `rustfmt.toml` configuration
- Uses standard rustfmt defaults

**Testing:**
- Unit tests: `#[test]` attributes in `mod tests`
- Async tests: `#[tokio::test]` with `tokio::test-util` feature
- Test organization: Per-module `#[cfg(test)]` modules
- Test helpers: `TestCfg`, `create_test_forge()`, temp fixtures via `tempfile`

---

## Build System Details

**Workspace Configuration:**
```toml
[workspace]
resolver = "3"
members = ["forge_core", "forge_runtime", "forge_agent"]
```

**Shared Profiles:**
- Release: LTO thin, single codegen unit, opt-level 3, abort on panic
- Bench: Inherits release with debug info
- Test: opt-level 2 for faster tests

**Conditional Compilation:**
- `#[cfg(feature = "sqlite")]` gates SQLite-specific code
- `#[cfg(not(feature = "sqlite"))]` for fallback/no-backend builds
- Feature flags propagate through workspace (e.g., `forge-runtime` depends on `forge-core/sqlite`)

---

## Storage and Persistence

**Graph Database:**
- Backend: SQLite via `sqlitegraph` crate
- Schema: Graph entities (symbols, references, CFG blocks)
- Location: `.forge/graph.db` within target codebase
- Connection: `SqliteGraph::open()`, introspection API for schema queries

**Caching:**
- Implementation: `QueryCache<K, V>` in `/home/feanor/Projects/forge/forge_core/src/cache.rs`
- Strategy: LRU with FIFO eviction, TTL-based expiration
- Default: 1000 entries, 300 second TTL
- Thread-safety: `Arc<RwLock<CacheInner>>`

**Connection Pooling:**
- Implementation: `ConnectionPool` in `/home/feanor/Projects/forge/forge_core/src/pool.rs`
- Strategy: Semaphore-based permit acquisition
- Default: 10 concurrent connections
- Auto-release: `ConnectionPermit` drops permit

---

## Architectural Integration Patterns

### Module Access Pattern
```rust
// All modules follow consistent access pattern
let forge = Forge::open("./project").await?;

// Access modules via forge instance
let graph = forge.graph();     // GraphModule
let search = forge.search();   // SearchModule
let cfg = forge.cfg();        // CfgModule
let edit = forge.edit();       // EditModule
let analysis = forge.analysis(); // AnalysisModule
```

### Runtime Layer Pattern
```rust
// Runtime orchestration in forge_runtime/src/lib.rs
let runtime = Runtime::new("./project").await?;

// Runtime combines: watching, indexing, caching, pooling
runtime.start_with_watching().await?;
runtime.process_events().await?;
```

### Agent Loop Pattern
```rust
// Agent orchestration in forge_agent/src/lib.rs
let mut agent = Agent::new("./project").await?;

// Six-phase deterministic loop
agent.run("Add authentication").await?;
// Internally: observe -> constrain -> plan -> mutate -> verify -> commit
```

---

## Error Handling Strategy

**Public API:**
- Type: `forge_core::ForgeError` (thiserror-derived)
- Variants: `DatabaseError`, `SymbolNotFound`, `InvalidQuery`, `EditConflict`, `VerificationFailed`, `PolicyViolation`, `BackendNotAvailable`, `CfgNotAvailable`, `PathOverflow`, `Io`, `Json`, `Graph`

**Internal API:**
- Type: `anyhow::Result<T>`
- Usage: Internal functions, runtime operations, indexing

**Agent Layer:**
- Type: `forge_agent::AgentError` (thiserror-derived)
- Wraps: `ForgeError` via `From` trait
- Variants: `ObservationFailed`, `PlanningFailed`, `MutationFailed`, `VerificationFailed`, `CommitFailed`, `PolicyViolation`

---

## Workspace Member Integration

### forge_core → forge_runtime
- Path dependency: `forge_core = { path = "../forge_core", default-features = false }`
- Pattern: Runtime uses Core's SDK but does not enable its features directly
- Uses: `UnifiedGraphStore` for storage backend access

### forge_core → forge_agent
- Path dependency: `forge_core = { path = "../forge_core", default-features = false }`
- Pattern: Agent uses Core's SDK for all graph operations
- Uses: `Forge` type, all modules (graph, search, cfg, edit)

### Cross-Crate Feature Flag Coordination
- All three crates define `sqlite` and `native-v2` features
- Features are coordinated: when `sqlite` is enabled on dependents, it's also enabled on forge-core
- This ensures backend consistency across workspace

---

## Key Architectural Decisions

### Storage Abstraction
- `UnifiedGraphStore` wraps `sqlitegraph::SqliteGraph` with optional backend support
- Allows future backend switching without API changes
- All modules access store through `Arc<UnifiedGraphStore>`

### Async-First Design
- All public API methods are `async fn`
- Uses tokio runtime throughout
- Synchronization via `tokio::sync` primitives (RwLock, Mutex, Semaphore)

### Module Builder Pattern
- Complex queries use builder types: `SearchBuilder`, `PathBuilder`
- Fluent API: `search.symbol("main").kind(SymbolKind::Function).limit(10).execute()`

### Observer Pattern for Hot-Reload
- `Watcher` spawns background task with `notify::RecommendedWatcher`
- Events sent via `mpsc::UnboundedSender<WatchEvent>`
- Consumed by `IncrementalIndexer` for queued processing

---

*Stack analysis: 2026-02-13*

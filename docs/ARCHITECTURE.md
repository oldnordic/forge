# ForgeKit Architecture

System design and architecture documentation for ForgeKit.

## Table of Contents

1. [Overview](#overview)
2. [System Architecture](#system-architecture)
3. [Crate Organization](#crate-organization)
4. [Backend Design](#backend-design)
5. [Pub/Sub System](#pubsub-system)
6. [Data Flow](#data-flow)
7. [Feature Flag System](#feature-flag-system)

## Overview

ForgeKit is a code intelligence SDK that unifies multiple tools (magellan, llmgrep, mirage, splice) under a single API with support for dual backends (SQLite and Native V3).

### Design Principles

1. **Unified API**: Single interface for all code intelligence operations
2. **Backend Agnostic**: Switch between SQLite and Native V3 without code changes
3. **Modular Tools**: Enable only the tools you need
4. **Async-First**: Built on Tokio for async/await
5. **Type Safety**: Leverage Rust's type system for correctness

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         ForgeKit                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   forge_core │  │forge_runtime│  │    forge_agent      │  │
│  │  (Core SDK)  │  │(Indexing)   │  │   (AI Agent Loop)   │  │
│  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘  │
└─────────┼────────────────┼────────────────────┼─────────────┘
          │                │                    │
          └────────────────┴────────────────────┘
                           │
          ┌────────────────┼────────────────┐
          ▼                ▼                ▼
   ┌──────────────┐ ┌──────────────┐ ┌──────────────┐
   │   Magellan   │ │   LLMGrep    │ │   Mirage     │
   │   (Graph)    │ │   (Search)   │ │   (CFG)      │
   └──────────────┘ └──────────────┘ └──────────────┘
          │                │                │
          └────────────────┼────────────────┘
                           │
          ┌────────────────┴────────────────┐
          ▼                                 ▼
   ┌──────────────┐                ┌──────────────┐
   │ SQLiteGraph  │                │    Splice    │
   │   SQLite     │                │   (Edit)     │
   │   Backend    │                └──────────────┘
   └──────────────┘
          │
          │ (uses)
          ▼
   ┌──────────────┐
   │  V3 Backend  │
   │  (Native)    │
   └──────────────┘
```

## Crate Organization

### forge_core

Core SDK providing the main API.

```
forge_core/src/
├── lib.rs           # Main Forge type, re-exports
├── types.rs         # Core types (Symbol, Location, etc.)
├── error.rs         # Error types
├── storage/         # Storage abstraction
│   └── mod.rs       # UnifiedGraphStore, BackendKind
├── graph/           # Graph module (magellan integration)
│   └── mod.rs       # GraphModule, symbol queries
├── search/          # Search module (llmgrep integration)
│   └── mod.rs       # SearchModule, pattern search
├── cfg/             # CFG module (mirage integration)
│   └── mod.rs       # CfgModule, control flow
├── edit/            # Edit module (splice integration)
│   └── mod.rs       # EditModule, safe editing
└── analysis/        # Analysis module (composite ops)
    └── mod.rs       # AnalysisModule
```

### forge_runtime

Runtime layer for indexing and caching.

```
forge_runtime/src/
└── lib.rs           # ForgeRuntime, indexing, caching
```

### forge_agent

AI agent layer for deterministic code operations.

```
forge_agent/src/
├── lib.rs           # Agent types
├── planner.rs       # Plan generation
├── mutate.rs        # Mutation operations
├── verify.rs        # Verification
├── commit.rs        # Committing changes
├── policy.rs        # Policy enforcement
└── observe.rs       # Observation/logging
```

## Backend Design

### UnifiedGraphStore

Abstraction over both backends with a unified API.

```rust
pub struct UnifiedGraphStore {
    codebase_path: PathBuf,
    db_path: PathBuf,
    backend_kind: BackendKind,
    references: Mutex<Vec<StoredReference>>,
}

impl UnifiedGraphStore {
    pub async fn open(path: &Path, kind: BackendKind) -> Result<Self>;
    pub fn is_connected(&self) -> bool;
    pub fn backend_kind(&self) -> BackendKind;
}
```

### BackendKind

Enum for backend selection:

```rust
pub enum BackendKind {
    SQLite,     // Uses sqlitegraph SQLite backend
    NativeV3,   // Uses sqlitegraph V3 backend
}
```

### SQLite Backend

- **Format**: SQLite database (.forge/graph.db)
- **Transactions**: Full ACID via SQLite
- **Access**: SQL queries supported
- **Dependencies**: libsqlite3

### Native V3 Backend

- **Format**: Binary format (.forge/graph.v3)
- **Transactions**: WAL-based
- **Access**: API-only (no SQL)
- **Dependencies**: Pure Rust
- **Performance**: 10-20x faster than SQLite

### Backend Selection Matrix

| Feature | SQLite | Native V3 |
|---------|--------|-----------|
| Graph queries | ✅ | ✅ |
| Pub/Sub | ✅ | ✅ |
| Raw SQL | ✅ | ❌ |
| Performance | Baseline | 10-20x |
| Memory | Higher | Lower |
| Dependencies | libsqlite3 | None |

## Pub/Sub System

Real-time event notification system.

### Architecture

```
┌─────────────────────────────────────────┐
│           Publisher (V3)                 │
│  ┌─────────────────────────────────┐    │
│  │  Mutex<Vec<(SubscriberId,       │    │
│  │            Sender, Filter)>>    │    │
│  └─────────────────────────────────┘    │
│                   │                     │
│         ┌────────┼────────┐             │
│         ▼        ▼        ▼             │
│      ┌────┐  ┌────┐  ┌────┐            │
│      │RX 1│  │RX 2│  │RX 3│            │
│      └────┘  └────┘  └────┘            │
└─────────────────────────────────────────┘
```

### Event Flow

1. **Mutation** occurs (node/edge/KV change)
2. **Transaction commits** (snapshot created)
3. **Events emitted** to Publisher
4. **Filter matching** determines recipients
5. **Best-effort delivery** to subscribers

### Event Types

```rust
pub enum PubSubEvent {
    NodeChanged { node_id: i64, snapshot_id: u64 },
    EdgeChanged { edge_id: i64, from_node: i64, to_node: i64, snapshot_id: u64 },
    KVChanged { key_hash: u64, snapshot_id: u64 },
    SnapshotCommitted { snapshot_id: u64 },
}
```

### Subscription Filter

```rust
pub struct SubscriptionFilter {
    pub node_changes: bool,
    pub edge_changes: bool,
    pub kv_changes: bool,
    pub snapshot_commits: bool,
}
```

### Lazy Initialization

V3 Publisher is created on first subscription:

```rust
fn get_or_init_publisher(&self) -> MappedRwLockReadGuard<'_, Publisher> {
    if self.publisher.read().is_none() {
        *self.publisher.write() = Some(Publisher::new());
    }
    // Return guard to publisher
}
```

## Data Flow

### Indexing Flow

```
1. File Change Detected
         │
         ▼
2. Magellan Indexer
   - Parse AST
   - Extract symbols
   - Build references
         │
         ▼
3. Store in Backend
   - SQLite: INSERT statements
   - V3: Page writes + WAL
         │
         ▼
4. Emit Pub/Sub Events
   - NodeChanged
   - EdgeChanged
   - SnapshotCommitted
         │
         ▼
5. Notify Subscribers
```

### Query Flow

```
1. User Query
   (find_symbol, search, etc.)
         │
         ▼
2. Module Router
   - graph → GraphModule
   - search → SearchModule
         │
         ▼
3. Backend Adapter
   - SQLite: SQL queries
   - V3: API calls
         │
         ▼
4. Return Results
```

## Feature Flag System

ForgeKit uses extensive feature flags for flexibility.

### Storage Features

```toml
[features]
sqlite = ["dep:sqlitegraph", "sqlitegraph/sqlite-backend"]
native-v3 = ["dep:sqlitegraph", "sqlitegraph/native-v3"]
```

### Tool Features (Per-Backend)

```toml
# Magellan
magellan-sqlite = ["dep:magellan"]
magellan-v3 = ["dep:magellan"]

# LLMGrep
llmgrep-sqlite = ["dep:llmgrep"]
llmgrep-v3 = ["dep:llmgrep", "native-v3"]

# Mirage
mirage-sqlite = ["dep:mirage-analyzer"]
mirage-v3 = ["dep:mirage-analyzer", "native-v3"]

# Splice
splice-sqlite = ["dep:splice"]
splice-v3 = ["dep:splice", "native-v3"]
```

### Convenience Groups

```toml
tools = ["magellan", "llmgrep", "mirage", "splice"]
tools-sqlite = ["magellan-sqlite", "llmgrep-sqlite", ...]
tools-v3 = ["magellan-v3", "llmgrep-v3", ...]

full = ["tools", "sqlite", "native-v3"]
full-sqlite = ["tools-sqlite", "sqlite"]
full-v3 = ["tools-v3", "native-v3"]
```

### Feature Resolution

Dependencies are resolved as:

1. **Explicit features** win (e.g., `magellan-v3`)
2. **Default features** apply if not disabled
3. **Tool features** enable the crate dependency
4. **Backend suffix** determines which backend the tool uses

### Example Feature Combinations

| Use Case | Features |
|----------|----------|
| Minimal SQLite | `sqlite`, `magellan-sqlite` |
| Full V3 | `full-v3` |
| Mixed | `magellan-v3`, `llmgrep-sqlite` |
| All tools, SQLite default | `tools-sqlite`, `sqlite` |

## Testing Architecture

### Test Organization

```
forge_core/tests/
├── accessor_tests.rs          # Module accessors
├── builder_tests.rs           # Builder pattern
├── pubsub_integration_tests.rs # Pub/Sub + backends
└── tool_integration_tests.rs   # Tool integrations
```

### Backend Testing

Each test file tests both backends:

```rust
#[tokio::test]
async fn test_sqlite_backend() { /* ... */ }

#[tokio::test]
async fn test_native_v3_backend() { /* ... */ }
```

### Integration Test Categories

1. **Backend Tests**: SQLite vs V3 parity
2. **Pub/Sub Tests**: Event subscription/delivery
3. **Tool Tests**: Magellan, LLMGrep integration
4. **Persistence Tests**: Database survive restarts

## Performance Considerations

### Backend Performance

| Operation | SQLite | Native V3 | Speedup |
|-----------|--------|-----------|---------|
| Graph traversal | Baseline | 10-20x | ✅ |
| Node lookup | Baseline | 5-10x | ✅ |
| Edge scan | Baseline | 15-20x | ✅ |
| Startup | Fast | Faster | ✅ |
| Memory | Higher | Lower | ✅ |

### Optimization Strategies

1. **Use Native V3** for large codebases
2. **Enable caching** with `cache_ttl`
3. **Specific filters** instead of broad queries
4. **Batch operations** when possible

## Security Considerations

1. **Database files** stored in `.forge/` (user-controlled)
2. **No network access** in core library
3. **Best-effort pub/sub** prevents DoS from slow subscribers
4. **Path validation** on all file operations

## Future Architecture

### Planned Enhancements

1. **Distributed mode**: Multiple Forge instances
2. **Remote backends**: Client/server architecture
3. **Plugin system**: Custom tool integrations
4. **Incremental indexing**: Smarter change detection

---

For implementation details, see [API Reference](API.md).
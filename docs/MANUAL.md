# ForgeKit User Manual

Complete guide to using ForgeKit for code intelligence operations.

## Table of Contents

1. [Getting Started](#getting-started)
2. [Backends](#backends)
3. [Core Operations](#core-operations)
4. [Pub/Sub Events](#pubsub-events)
5. [Advanced Usage](#advanced-usage)
6. [Troubleshooting](#troubleshooting)

## Getting Started

### Installation

Add ForgeKit to your `Cargo.toml`:

```toml
[dependencies]
forge-core = "0.2"
tokio = { version = "1", features = ["full"] }
```

### Opening a Codebase

```rust
use forge_core::{Forge, BackendKind};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Option 1: Default backend (SQLite)
    let forge = Forge::open("./my-project").await?;
    
    // Option 2: Specify backend
    let forge = Forge::open_with_backend(
        "./my-project", 
        BackendKind::NativeV3
    ).await?;
    
    // Option 3: Builder pattern
    let forge = Forge::builder()
        .path("./my-project")
        .backend_kind(BackendKind::NativeV3)
        .build()
        .await?;
    
    Ok(())
}
```

### Database Location

ForgeKit stores databases in `.forge/` directory:

```
my-project/
├── src/
├── .forge/
│   ├── graph.db      # SQLite backend
│   └── graph.v3      # Native V3 backend
└── Cargo.toml
```

## Backends

### SQLite Backend

Stable, mature backend with full SQL access:

```rust
let forge = Forge::open_with_backend(
    "./project",
    BackendKind::SQLite
).await?;
```

**Pros:**
- Full ACID transactions
- Raw SQL access
- Battle-tested stability
- External tool compatibility

**Cons:**
- Slower than V3
- Requires libsqlite3

### Native V3 Backend

High-performance pure Rust backend:

```rust
let forge = Forge::open_with_backend(
    "./project",
    BackendKind::NativeV3
).await?;
```

**Pros:**
- 10-20x faster traversals
- Pure Rust (no C dependencies)
- Better for large codebases
- Lower memory overhead

**Cons:**
- No raw SQL access
- Newer (less battle-tested)

### Backend Selection Guide

| Use Case | Recommended Backend |
|----------|---------------------|
| New projects | Native V3 |
| Large codebases (>1M LOC) | Native V3 |
| Need SQL access | SQLite |
| Maximum stability | SQLite |
| CI/CD pipelines | Native V3 |

## Core Operations

### Graph Module

The graph module provides symbol and reference queries.

#### Finding Symbols

```rust
let graph = forge.graph();

// Find by name
let symbols = graph.find_symbol("main").await?;

// Find with context
for symbol in &symbols {
    println!("{}: {} at {:?}", 
        symbol.kind, 
        symbol.name, 
        symbol.location
    );
}
```

#### Finding References

```rust
// Find all references to a symbol
let refs = graph.find_references("my_function").await?;

// Find callers (functions that call this)
let callers = graph.find_callers("my_function").await?;
```

#### Call Graph Navigation

```rust
// Get outgoing calls from a function
let calls = graph.get_calls("main").await?;

// Navigate call hierarchy
for call in &calls {
    println!("main calls {} in {}", 
        call.target_name, 
        call.location.file
    );
}
```

### Search Module

Semantic code search via LLMGrep integration.

#### Pattern Search

```rust
let search = forge.search();

// Regex pattern search
let results = search.pattern(r"fn.*test.*\(").await?;

// Fuzzy symbol search
let results = search.fuzzy("myfn").await?;
```

#### Semantic Search

```rust
// Search by kind
let functions = search.by_kind(SymbolKind::Function).await?;

// Search by language
let rust_items = search.by_language(Language::Rust).await?;
```

### CFG Module

Control flow graph analysis.

```rust
let cfg = forge.cfg();

// Build CFG for a function
let graph = cfg.build_cfg("my_function").await?;

// Find paths between nodes
let paths = cfg.find_paths("start", "end").await?;

// Compute dominators
let doms = cfg.compute_dominators("entry").await?;
```

### Edit Module

Span-safe code editing via Splice integration.

```rust
let edit = forge.edit();

// Rename a symbol
edit.rename_symbol("old_name", "new_name").await?;

// Apply a patch
edit.apply_patch(Patch {
    location: span,
    replacement: "new code".to_string(),
}).await?;
```

## Pub/Sub Events

Real-time notifications for code changes.

### Subscribing to Events

```rust
use forge_core::storage::SubscriptionFilter;

// Subscribe to all events
let (id, rx) = forge.subscribe(SubscriptionFilter::all()).await?;

// Subscribe to specific events
let filter = SubscriptionFilter {
    node_changes: true,
    edge_changes: false,
    kv_changes: false,
    snapshot_commits: true,
};
let (id, rx) = forge.subscribe(filter).await?;
```

### Handling Events

```rust
use std::sync::mpsc::RecvTimeoutError;

// In a separate task/thread
std::thread::spawn(move || {
    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(event) => {
                match event {
                    PubSubEvent::NodeChanged { node_id, .. } => {
                        println!("Node {} changed", node_id);
                    }
                    PubSubEvent::SnapshotCommitted { snapshot_id } => {
                        println!("Transaction {} committed", snapshot_id);
                    }
                    _ => {}
                }
            }
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
});
```

### Unsubscribing

```rust
// Stop receiving events
forge.unsubscribe(id).await?;
```

### Event Types

| Event | Description | Fields |
|-------|-------------|--------|
| `NodeChanged` | Symbol created/modified | `node_id`, `snapshot_id` |
| `EdgeChanged` | Reference/call changed | `edge_id`, `from_node`, `to_node`, `snapshot_id` |
| `KVChanged` | Key-value entry changed | `key_hash`, `snapshot_id` |
| `SnapshotCommitted` | Transaction committed | `snapshot_id` |

## Advanced Usage

### Custom Backend Configuration

```rust
// Using builder for fine-grained control
let forge = Forge::builder()
    .path("./project")
    .backend_kind(BackendKind::NativeV3)
    .database_path("./custom/path/graph.v3")
    .build()
    .await?;
```

### Feature Flag Combinations

```rust
// Cargo.toml examples:

// 1. SQLite with only Magellan and LLMGrep
[dependencies]
forge-core = { version = "0.2", default-features = false, features = ["sqlite", "magellan-sqlite", "llmgrep-sqlite"] }

// 2. V3 with all tools
[dependencies]
forge-core = { version = "0.2", features = ["full-v3"] }

// 3. Mixed: Magellan V3, LLMGrep SQLite
[dependencies]
forge-core = { version = "0.2", default-features = false, features = ["magellan-v3", "llmgrep-sqlite"] }
```

### Working with Multiple Codebases

```rust
// Open multiple codebases concurrently
let (forge1, forge2) = tokio::join!(
    Forge::open("./project1"),
    Forge::open("./project2")
);

let forge1 = forge1?;
let forge2 = forge2?;

// Query both
let symbols1 = forge1.graph().find_symbol("main").await?;
let symbols2 = forge2.graph().find_symbol("main").await?;
```

### Cross-Backend Queries

```rust
// Query SQLite and V3 backends in same program
let forge_sqlite = Forge::open_with_backend("./project", BackendKind::SQLite).await?;
let forge_v3 = Forge::open_with_backend("./project", BackendKind::NativeV3).await?;

// Both work independently
let sqlite_results = forge_sqlite.search().pattern("test").await?;
let v3_results = forge_v3.search().pattern("test").await?;
```

## Troubleshooting

### Database Persistence Issues

**Problem:** V3 database doesn't persist between runs

**Solution:** Ensure you're using sqlitegraph 2.0.5+:

```toml
[dependencies]
forge-core = "0.2"  # Uses sqlitegraph 2.0.5+
```

### Feature Flag Errors

**Problem:** "feature not found" errors

**Solution:** Check feature flag names:

```toml
# Correct
features = ["magellan-v3", "llmgrep-sqlite"]

# Incorrect
features = ["magellan", "v3"]  # Won't work
```

### Backend Compatibility

**Problem:** Tools not working with V3 backend

**Solution:** Ensure tools are updated:

```toml
# These versions support V3:
# - magellan 2.4.5+
# - llmgrep 3.0.8+
# - mirage-analyzer 1.0+
# - splice 2.5+
```

### Performance Issues

**Problem:** Slow queries on large codebases

**Solution:**
1. Use Native V3 backend
2. Enable caching:
   ```rust
   let forge = Forge::builder()
       .path("./project")
       .cache_ttl(Duration::from_secs(60))
       .build()
       .await?;
   ```
3. Use specific filters instead of broad queries

### Getting Help

1. Check the [API documentation](API.md)
2. Review [architecture docs](ARCHITECTURE.md)
3. File an issue on [GitHub](https://github.com/oldnordic/forge/issues)

---

For more information, see the [API Reference](API.md).
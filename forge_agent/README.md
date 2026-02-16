# ForgeKit - Deterministic Code Intelligence SDK

[![Crates.io](https://img.shields.io/crates/v/forge-core)](https://crates.io/crates/forge-core)
[![Documentation](https://docs.rs/forge-core/badge.svg)](https://docs.rs/forge-core)
[![License: GPL-3.0](https://img.shields.io/badge/License-GPL%203.0-blue.svg)](https://opensource.org/licenses/GPL-3.0)

ForgeKit provides a unified SDK for code intelligence operations, integrating multiple tools into a single API with support for both SQLite and Native V3 backends.

## Features

- **🔍 Graph Queries**: Symbol lookup, reference tracking, call graph navigation
- **🔎 Semantic Search**: Pattern-based code search via LLMGrep integration  
- **🌳 Control Flow Analysis**: CFG construction and analysis via Mirage
- **✏️ Safe Code Editing**: Span-safe refactoring via Splice
- **📊 Dual Backend Support**: SQLite (stable) or Native V3 (high performance)
- **📡 Pub/Sub Events**: Real-time notifications for code changes
- **⚡ Async-First**: Built on Tokio for async/await support

## Quick Start

```rust
use forge_core::{Forge, BackendKind};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Open a codebase with default backend (SQLite)
    let forge = Forge::open("./my-project").await?;
    
    // Or use Native V3 backend for better performance
    let forge = Forge::open_with_backend("./my-project", BackendKind::NativeV3).await?;
    
    // Find symbols
    let symbols = forge.graph().find_symbol("main").await?;
    println!("Found: {:?}", symbols);
    
    // Search code
    let results = forge.search().pattern("fn.*test").await?;
    
    Ok(())
}
```

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
forge-core = "0.2"
```

### Feature Flags

ForgeKit uses feature flags for flexible backend and tool selection:

**Storage Backends:**
- `sqlite` - SQLite backend (default)
- `native-v3` - Native V3 high-performance backend

**Tool Integrations (per-backend):**
- `magellan-sqlite` / `magellan-v3` - Code indexing
- `llmgrep-sqlite` / `llmgrep-v3` - Semantic search
- `mirage-sqlite` / `mirage-v3` - CFG analysis
- `splice-sqlite` / `splice-v3` - Code editing

**Convenience Groups:**
- `tools-sqlite` - All tools with SQLite
- `tools-v3` - All tools with V3
- `full-sqlite` - Everything with SQLite
- `full-v3` - Everything with V3

### Examples

```toml
# Default: SQLite backend with all tools
forge-core = "0.2"

# Native V3 backend with all tools
forge-core = { version = "0.2", features = ["full-v3"] }

# Mix and match: Magellan with V3, LLMGrep with SQLite
forge-core = { version = "0.2", features = ["magellan-v3", "llmgrep-sqlite"] }
```

## Workspace Structure

ForgeKit is organized as a workspace with three crates:

| Crate | Purpose | Documentation |
|-------|---------|---------------|
| `forge_core` | Core SDK with graph, search, CFG, and edit APIs | [API Docs](docs/API.md) |
| `forge_runtime` | Indexing, caching, and file watching | [Architecture](docs/ARCHITECTURE.md) |
| `forge_agent` | Deterministic AI agent loop | [Manual](docs/MANUAL.md) |

## Backend Comparison

| Feature | SQLite | Native V3 |
|---------|--------|-----------|
| ACID Transactions | ✅ Full | ✅ WAL-based |
| Raw SQL Access | ✅ Yes | ❌ No |
| Dependencies | libsqlite3 | Pure Rust |
| Performance | Fast | **10-20x faster** |
| Pub/Sub | ✅ Yes | ✅ Yes |
| Tool Compatibility | All tools | All tools (v2.0.5+) |

**Recommendation:** Use Native V3 for new projects. Use SQLite if you need raw SQL access.

## Pub/Sub (Real-time Events)

ForgeKit supports real-time event notifications for code changes:

```rust
use forge_core::{Forge, BackendKind};
use std::sync::mpsc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let forge = Forge::open_with_backend("./project", BackendKind::NativeV3).await?;
    
    // Subscribe to node changes
    let (id, rx) = forge.subscribe(
        SubscriptionFilter::nodes_only()
    ).await?;
    
    // Receive events in a separate task
    tokio::spawn(async move {
        while let Ok(event) = rx.recv() {
            println!("Code changed: {:?}", event);
        }
    });
    
    Ok(())
}
```

### Event Types

- `NodeChanged` - Symbol created or modified
- `EdgeChanged` - Reference/call created or modified
- `KVChanged` - Key-value store entry changed
- `SnapshotCommitted` - Transaction committed

## Documentation

- **[API Reference](docs/API.md)** - Complete API documentation
- **[Architecture](docs/ARCHITECTURE.md)** - System design and internals
- **[Manual](docs/MANUAL.md)** - User guide and tutorials
- **[Contributing](docs/CONTRIBUTING.md)** - Contribution guidelines
- **[Changelog](CHANGELOG.md)** - Version history

## Tool Integrations

ForgeKit integrates with these code intelligence tools:

| Tool | Purpose | Backend Support |
|------|---------|-----------------|
| [magellan](https://github.com/oldnordic/magellan) | Code indexing and graph queries | SQLite, V3 |
| [llmgrep](https://github.com/oldnordic/llmgrep) | Semantic code search | SQLite, V3 |
| [mirage-analyzer](https://crates.io/crates/mirage-analyzer) | CFG analysis | SQLite, V3 |
| [splice](https://github.com/oldnordic/splice) | Span-safe editing | SQLite, V3 |

## License

This project is licensed under the GPL-3.0 License - see the [LICENSE](LICENSE) file for details.

## Support

- Issues: [GitHub Issues](https://github.com/oldnordic/forge/issues)
- Discussions: [GitHub Discussions](https://github.com/oldnordic/forge/discussions)

---

**Note:** This is an early-stage project. APIs may change until v1.0.
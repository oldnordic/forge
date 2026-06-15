# forgekit-runtime

[![Crates.io](https://img.shields.io/crates/v/forgekit-runtime)](https://crates.io/crates/forgekit-runtime)
[![Documentation](https://docs.rs/forgekit-runtime/badge.svg)](https://docs.rs/forgekit-runtime)
[![License: GPL-3.0](https://img.shields.io/badge/License-GPL%203.0-blue.svg)](https://opensource.org/licenses/GPL-3.0)

**Status: alpha — work in progress. APIs may change until v1.0.**

The runtime coordination layer for ForgeKit. Handles file watching, incremental
indexing, caching, and metrics so that a code graph stays in sync with a live
codebase without full re-indexing on every change.

> **Alpha notice:** this crate is functional but the surface is still settling.
> `ForgeRuntime`, `RuntimeConfig`, and the metrics types may see breaking
> changes. What works: file watching, incremental re-indexing, query cache
> invalidation, metrics collection. What is experimental: the native-v3 backend
> path (see `forgekit_core` feature flags).

## What this crate provides

- **File watching** — monitors a codebase for changes and emits `WatchEvent`s.
- **Incremental indexing** — updates only the affected symbols/graph regions on
  a file change, avoiding a full re-scan.
- **Query cache** — caches graph query results and invalidates them when the
  underlying graph changes.
- **Metrics** — collects runtime statistics (`RuntimeMetrics`) covering index
  flush counts, cache hit rates, and watcher health.

This crate re-exports the core watcher/indexer types (`Watcher`,
`IncrementalIndexer`, `QueryCache`, `PathFilter`, `WatchEvent`, `FlushStats`)
from `forgekit_core` and wraps them in a managed `ForgeRuntime` that wires the
pieces together.

## Quick Start

```rust
use forgekit_runtime::{ForgeRuntime, RuntimeConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = RuntimeConfig::default();
    let runtime = ForgeRuntime::start("./my-project", config).await?;

    // The runtime now watches for file changes and keeps the graph fresh.
    // Query the live metrics at any time:
    let stats: RuntimeStats = runtime.stats();
    println!("Index flushes: {}", stats.flush_count);

    Ok(())
}
```

## Relationship to other crates

| Crate | Role |
|-------|------|
| `forgekit_core` | The SDK foundation: graph, search, CFG, edit, analysis |
| **`forgekit_runtime`** | **This crate: watching, incremental indexing, caching, metrics** |
| `forgekit_agent` | The agent layer: 6-phase loop, ReAct, workflow engine |
| `forgekit-reasoning` | Temporal checkpointing for reasoning tools |

## License

GPL-3.0-only — see [LICENSE.md](../LICENSE.md).

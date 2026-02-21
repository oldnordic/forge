# ForgeKit - Deterministic Code Intelligence SDK

[![License: GPL-3.0](https://img.shields.io/badge/License-GPL%203.0-blue.svg)](https://opensource.org/licenses/GPL-3.0)
[![Build Status](https://img.shields.io/badge/build-passing-brightgreen)](https://github.com/oldnordic/forge)
[![Tests](https://img.shields.io/badge/tests-522%20passing-brightgreen)](https://github.com/oldnordic/forge)

ForgeKit provides a unified SDK for code intelligence operations, integrating multiple tools into a single API with support for both SQLite and Native V3 backends.

## Features

- **🔍 Graph Queries**: Symbol lookup, reference tracking, call graph navigation
- **📊 Impact Analysis**: k-hop traversal to find affected symbols
- **🔎 Semantic Search**: Pattern-based code search via LLMGrep integration  
- **🌳 Control Flow Analysis**: CFG construction and analysis via Mirage
- **🗑️ Dead Code Detection**: Find unused functions and methods
- **📈 Complexity Metrics**: Cyclomatic complexity and risk analysis
- **✏️ Safe Code Editing**: Span-safe refactoring via Splice
- **📊 Dual Backend Support**: SQLite (stable) or Native V3 (high performance)
- **⚡ Async-First**: Built on Tokio for async/await support

## Quick Start

```rust
use forge_core::{Forge, BackendKind};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Open a codebase with default backend (SQLite)
    let forge = Forge::open("./my-project").await?;
    
    // Find symbols
    let symbols = forge.graph().find_symbol("main").await?;
    println!("Found: {:?}", symbols);
    
    // Find all callers of a function
    let callers = forge.graph().callers_of("my_function").await?;
    println!("Callers: {}", callers.len());
    
    // Impact analysis - what would break if we change this?
    let impact = forge.graph()
        .impact_analysis("critical_function", Some(2))
        .await?;
    println!("Affected symbols: {}", impact.len());
    
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

**Storage Backends:**
- `sqlite` - SQLite backend (default)
- `native-v3` - Native V3 high-performance backend

**Tool Integrations:**
- `magellan-sqlite` / `magellan-v3` - Code indexing
- `llmgrep-sqlite` / `llmgrep-v3` - Semantic search
- `mirage-sqlite` / `mirage-v3` - CFG analysis
- `splice-sqlite` / `splice-v3` - Code editing

**Convenience Groups:**
- `tools-sqlite` - All tools with SQLite
- `tools-v3` - All tools with V3
- `full-sqlite` / `full-v3` - Everything

### Examples

```toml
# Default: SQLite backend with all tools
forge-core = "0.2"

# Native V3 backend with all tools
forge-core = { version = "0.2", features = ["full-v3"] }

# Minimal: Just storage backends
forge-core = { version = "0.2", default-features = false, features = ["sqlite"] }
```

## Workspace Structure

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
| Tool Compatibility | All tools | All tools (v2.0.5+) |

**Recommendation:** Use Native V3 for new projects. Use SQLite if you need raw SQL access.

## Working Examples

### Impact Analysis

Find all symbols that would be affected by changing a function:

```rust
let forge = Forge::open("./project").await?;

// Find all symbols within 2 hops of "process_request"
let impacted = forge.graph()
    .impact_analysis("process_request", Some(2))
    .await?;

for symbol in impacted {
    println!("{} ({} hops): {}", 
        symbol.name, 
        symbol.hop_distance,
        symbol.file_path
    );
}
```

### Dead Code Detection

Find unused functions in your codebase:

```rust
let analysis = forge.analysis();

let dead_code = analysis.find_dead_code().await?;
for symbol in dead_code {
    println!("Unused: {} in {}", symbol.name, symbol.location.file);
}
```

### Complexity Analysis

Calculate cyclomatic complexity:

```rust
let analysis = forge.analysis();

// From source code
let metrics = analysis.analyze_source_complexity(source_code);
println!("Complexity: {} ({})", 
    metrics.cyclomatic_complexity,
    metrics.risk_level().as_str()
);
```

### Control Flow Analysis

```rust
let cfg = forge.cfg();

// Get dominator tree for a function
let dominators = cfg.dominators(symbol_id).await?;

// Find loops
let loops = cfg.loops(symbol_id).await?;

// Enumerate paths
let paths = cfg.paths(symbol_id)
    .normal_only()
    .max_length(10)
    .execute()
    .await?;
```

### Pattern Search

```rust
let search = forge.search();

// Regex pattern search
let results = search.pattern(r"fn.*test.*\(").await?;

// Semantic search
let results = search.semantic("authentication logic").await?;
```

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

**Current Version:** 0.2.2

**Note:** ForgeKit is currently in active development. The crate has not yet been published to crates.io. APIs may change until v1.0.

**Compiler Warnings:** The project uses sqlitegraph 2.0.8 which has 61 intentional dead code warnings. These are kept for API completeness, feature-gated functionality, and future use - they do not indicate bugs or incomplete code.

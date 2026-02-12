# ForgeKit - Deterministic Code Intelligence SDK

**Version**: 0.1.0 (Design Phase)
**Created**: 2025-12-30
**Status**: Active Development

---

## Overview

ForgeKit is a unified Rust SDK that exposes deterministic code intelligence capabilities through a single programmable interface. It combines Magellan, LLMGrep, Mirage, and Splice into a cohesive cognition layer for building AI-native developer tools.

### Vision

> **"LLVM for AI Code Agents"**

ForgeKit provides an intermediate representation (the graph) with deterministic transformations and verified mutations. It is infrastructure, not another wrapper around LLMs.

### What ForgeKit Becomes

- **NOT**: Another AI wrapper
- **NOT**: Another CLI tool
- **NOT**: Another code search tool

- **IS**: A deterministic, graph-backed reasoning SDK
- **IS**: Local-first, single binary, auditable
- **IS**: The cognition layer for AI-native IDEs, agents, and code auditors

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         forge_core                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │   Graph      │  │    Search    │  │     CFG      │  │
│  │  (Magellan)  │  │  (LLMGrep)   │  │   (Mirage)   │  │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘  │
│         │                   │                   │            │
│         └───────────────────┴───────────────────┘            │
│                            │                                │
│                    ┌───────┴────────┐                   │
│                    │     Edit       │                   │
│                    │   (Splice)     │                   │
│                    └────────────────┘                   │
└─────────────────────────────┬────────────────────────────────────┘
                          │
         ┌────────────────┴────────────────┐
         │                                  │
┌────────┴────────┐            ┌───────────┴──────────┐
│  forge_runtime  │            │    forge_agent        │
│  (Indexing +    │            │  (Deterministic      │
│   Caching)      │            │   AI Loop)          │
└─────────────────┘            └──────────────────────┘
         │                                  │
         └────────────────┬─────────────────┘
                          │
┌─────────────────────────────┴───────────────────────────────────┐
│                  SQLiteGraph Backend                       │
│  (SQLite today, Native V3 binary file in progress)        │
└──────────────────────────────────────────────────────────────┘
```

---

## Components

### forge_core

Pure Rust library providing the unified API.

```rust
use forge::Forge;

let forge = Forge::open("./repo")?;

// Code graph operations
let graph = forge.graph();
let symbols = graph.find_symbols("main")?;
let refs = graph.references("main")?;

// Semantic search
let search = forge.search();
let results = search.symbol("Database")
    .kind("Struct")?

// CFG analysis
let cfg = forge.cfg();
let paths = cfg.paths("process_data")?;
let dominators = cfg.dominators("parse")?;

// Span-safe editing
let edit = forge.edit();
edit.rename_symbol("OldName", "NewName")?
    .verify()?
    .apply()?;
```

### forge_runtime

Indexing and caching layer.

- Watch mode for continuous reindexing
- Query result caching
- Backend-agnostic storage interface

### forge_agent (Optional)

Deterministic AI orchestration loop.

```rust
use forge::agent::Agent;

let result = Agent::new(&forge)
    .observe("Rename function foo to bar")
    .constrain(Policy::NoUnsafeInPublicAPI)
    .plan()?
    .mutate()?
    .verify()?
    .commit()?;
```

---

## Deterministic Loop

Unlike most AI coding tools (Prompt → Guess → Rewrite → Hope), ForgeKit enforces:

```
Query → Graph Reason → Validate → Safe Patch → Re-index
```

Every operation is:
- **Span-verified**: Edits target exact byte ranges
- **Validated**: Compiler/LSP gatekeeper confirms correctness
- **Atomic**: All-or-nothing mutations
- **Auditable**: Full history with rollback capability

---

## Quick Start

### Installation

```bash
cargo install forge-core
cargo install forge-runtime  # Optional
cargo install forge-agent   # Optional
```

### Basic Usage

```rust
use forge::Forge;

#[tokio::main]
async fn main() -> Result<()> {
    // Open a codebase
    let forge = Forge::builder()
        .path("./my-project")
        .backend(ForgeBackend::Sqlite)
        .build()
        .await?;

    // Query the code graph
    let main_fn = forge.graph()
        .find_symbol("main")?
        .expect("main function not found");

    println!("Found main at {:?}", main_fn.location);

    // Analyze control flow
    let paths = forge.cfg()
        .enumerate_paths(&main_fn.id)?
        .normal_only()?
        .limit(10)?
        .execute()?;

    println!("Found {} execution paths", paths.len());

    Ok(())
}
```

---

## Development Status

| Component | Status | Notes |
|-----------|----------|--------|
| forge_core | 🚧 Design | API design in progress |
| forge_runtime | 📋 Planned | Indexing layer |
| forge_agent | 📋 Planned | Optional AI loop |
| SQLiteGraph | ✅ Stable | Production-ready |
| Native V3 Backend | 🚧 In Progress | See sqlitegraph project |
| Magellan | ✅ Stable | v2.2.1 |
| LLMGrep | ✅ Stable | Semantic search |
| Mirage | ✅ Stable | CFG analysis |
| Splice | ✅ Stable | v2.5.0 |

---

## Documentation

- [ARCHITECTURE.md](docs/ARCHITECTURE.md) - System architecture and design
- [API.md](docs/API.md) - API reference
- [PHILOSOPHY.md](docs/PHILOSOPHY.md) - Design philosophy
- [CONTRIBUTING.md](docs/CONTRIBUTING.md) - Contribution guidelines
- [DEVELOPMENT_WORKFLOW.md](docs/DEVELOPMENT_WORKFLOW.md) - Development workflow
- [ROADMAP.md](docs/ROADMAP.md) - Project roadmap

---

## License

GPL-3.0-or-later

See [LICENSE](LICENSE) for details.

---

*Last updated: 2025-12-30*

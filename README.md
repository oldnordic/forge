# ForgeKit - Deterministic Code Intelligence SDK

**Version**: 0.2.0 (Active Development)
**Created**: 2025-12-30
**Status**: V3 Backend Integrated
**Last Updated**: 2026-02-13

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
- **NOT**: Tied to any specific agent framework

- **IS**: A deterministic, graph-backed reasoning SDK
- **IS**: Local-first, single binary, auditable
- **IS**: The cognition layer for ANY agent framework (LangGraph, LangChain, OdinCode, custom)

### Usage Modes

**1. Tool Mode (Direct API)**
```rust
use forge_core::{Forge, GraphModule, SearchModule};

let forge = Forge::open("./repo").await?;
let symbols = forge.graph().find_symbol("main")?;
let results = forge.search().pattern("async fn").execute()?;
```

**2. Agent Mode (Plan Kernel)**
```rust
use forge_runtime::{PlanKernel, Agent};

let kernel = PlanKernel::new("./repo").await?;
let plan_id = kernel.plan.create("Refactor to async", constraints).await?;
kernel.step.execute(step_id).await?;
```

**3. CI/CD Mode**
```bash
# Direct tool usage in pipelines
magellan --db .forge/graph.v3 find --name "main"
llmgrep --db .forge/graph.v3 search --query "TODO"
mirage --db .forge/graph.v3 cfg --function "process"
```

**ForgeKit is library-first.** Use it with:
- OdinCode (multi-agent swarm)
- LangGraph (Python/JS agents)
- LangChain (Python agents)
- Custom agent frameworks
- Direct CLI tools (magellan, llmgrep, mirage, splice)
- CI/CD pipelines

---

## Usage Modes (Menu Approach)

ForgeKit provides multiple usage modes. **You choose** based on your needs:

### 1. Tool Mode (Direct API)

**For**: Simple refactors, scripts, CI/CD pipelines, direct tool usage

```rust
use forge_core::{Forge, GraphModule, SearchModule};

let forge = Forge::open("./repo").await?;
let symbols = forge.graph().find_symbol("main")?;
let results = forge.search().pattern("async fn").execute()?;
```

**Characteristics**:
- Direct calls to graph/search/cfg/edit modules
- No planning overhead
- Suitable for single-file operations
- Works with any toolchain

### 2. Agent Mode (Plan Kernel C Mode)

**For**: Multi-step operations requiring coordination, handoffs, parallel agents

```rust
use forge_runtime::{PlanKernel, Agent};

let kernel = PlanKernel::new("./repo").await?;
let plan_id = kernel.plan.create("Refactor to async", constraints).await?;
kernel.step.execute(step_id).await?;
```

**Characteristics**:
- Plan Graph stores all operations (append-only)
- Pub/Sub coordinates multiple agents
- Handoff protocol for token budgets
- File lease system prevents conflicts

### 3. Hybrid Mode

**For**: Complex workflows mixing both approaches

```rust
// Mix direct API and Plan Kernel as needed
let forge = Forge::open("./repo").await?;
let kernel = PlanKernel::new(&forge).await?;  // Optional!

// Use direct API for simple queries
let symbols = forge.graph().find_symbol("main")?;

// Use Plan Kernel for complex multi-file refactors
if needs_planning {
    let plan_id = kernel.plan.create("Complex task", constraints).await?;
    kernel.step.execute(plan_id).await?;
}
```

**Mode Selection Guide**:

| Use Case | Recommended Mode | Reason |
|-----------|-----------------|--------|
| Single-file refactor | Tool Mode | No planning overhead |
| Multi-file project | Agent Mode | Coordination needed |
| CI/CD pipeline | Tool Mode | Deterministic, reproducible |
| One-shot query | Tool Mode | Fast, direct |
| Multi-agent swarm | Agent Mode | Handoff, scaling |
| Custom orchestrator | Agent Mode | Framework integration |

**Key Point**: ForgeKit is a **library**, not a framework. You choose your mode.

---

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
│  forge_runtime  │            │    forge_agent (Optional) │
│  (Indexing +    │            │  (Deterministic      │
│   Caching)      │            │   AI Loop)          │
└─────────────────┘            └──────────────────────┘
         │                                  │
         └────────────────┬─────────────────┘
                          │
┌─────────────────────────────┴───────────────────────────────────┐
│                  Native V3 Backend                         │
│         (High-performance binary graph storage)            │
└───────────────────────────────────────────────────────────────┘
```

---

## Components

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

### forge_runtime (Optional)

Indexing and caching layer. **Optional** — see forge_core for direct storage access.

**Note**: The Plan Kernel (C Mode) provides its own event-based coordination. Use forge_runtime only if you need custom event handling beyond the Plan Kernel.

- Watch mode for continuous reindexing
- Query result caching
- Backend-agnostic storage interface

### forge_agent (Optional)

Deterministic AI orchestration loop with Plan Kernel (C Mode).

**Note**: This is an **optional** component. Use Tool Mode (direct API) for simple operations. Use Agent Mode only when you need multi-agent coordination, handoffs, or parallel execution.

```rust
// Agent Mode - Plan Kernel coordinates work
use forge_runtime::PlanKernel;

let kernel = PlanKernel::new(&forge).await?;
let plan_id = kernel.plan.create("My goal", constraints).await?;
```

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
use forge_core::Forge;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Open a codebase - creates .forge/graph.v3
    let forge = Forge::open("./my-project").await?;

    // Query the code graph
    let symbols = forge.graph()
        .find_symbol("main")
        .await?;

    for symbol in &symbols {
        println!("Found {} at {:?}", symbol.name, symbol.location);
    }

    // Search for symbols by pattern
    let results = forge.search()
        .pattern_search("async fn")
        .await?;

    println!("Found {} async functions", results.len());

    // Find callers of a function
    let callers = forge.graph()
        .callers_of("process_data")
        .await?;

    println!("Found {} callers", callers.len());

    Ok(())
}
```

---

## Development Status

| Component | Status | Notes |
|-----------|----------|--------|
| forge_core | ✅ Active | V3 backend integrated |
| forge_runtime | 🚧 In Progress | Indexing with path filtering |
| forge_agent | 📋 Planned | Optional AI loop |
| sqlitegraph V3 | ✅ Stable | v2.0.1 - Production-ready |
| Magellan | ✅ Stable | Graph module |
| LLMGrep | ✅ Stable | Search module |
| Mirage | 📋 Planned | CFG module |
| Splice | 📋 Planned | Edit module |

### What's Working Now
- ✅ **V3 Backend**: Native binary graph storage (`.forge/graph.v3`)
- ✅ **Symbol Storage**: Insert/query symbols with metadata
- ✅ **Reference Tracking**: Store and query symbol references
- ✅ **Path Filtering**: Only indexes `src/` and `tests/` by default
- ✅ **Large Data Support**: Symbols with >64 bytes of metadata work correctly
- 🚧 **Incremental Indexing**: File watching with filtered events
- 📋 **CFG Analysis**: Planned for v0.3.0
- 📋 **Span-safe Editing**: Planned for v0.4.0

---

## Path Filtering

By default, ForgeKit only indexes source code in `src/` and `tests/` directories. This prevents indexing build artifacts, dependencies, and generated files.

### Default Behavior

**Indexed:**
- `src/**/*.rs` (Rust source)
- `tests/**/*.rs` (Test files)
- Similar patterns for `.py`, `.js`, `.ts`, `.go`, `.java`, etc.

**Ignored:**
- `target/**` (Rust build artifacts)
- `node_modules/**` (Node dependencies)
- `.git/**` (Git internals)
- `.forge/**` (ForgeKit's own data)
- `Cargo.lock`, `package-lock.json` (Lock files)
- Binary files (`.png`, `.bin`, etc.)

### Custom Path Filters

```rust
use forge_core::indexing::{IncrementalIndexer, PathFilter};

// Create a custom filter
let mut filter = PathFilter::new();
filter.add_include("**/lib/**");
filter.add_include("**/src/**");
filter.add_extension("rs");
filter.add_extension("go");

// Use with the indexer
let indexer = IncrementalIndexer::with_filter(store, filter);
```

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

*Last updated: 2026-02-13*

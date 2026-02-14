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
magellan --db .forge/graph.db find --name "main"
llmgrep --db .forge/graph.db search --query "TODO"
mirage --db .forge/graph.db cfg --function "process"
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
│                  SQLiteGraph Backend                       │
│  (SQLite today, Native V3 binary file in progress)        │
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

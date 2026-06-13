# ForgeKit - Code Intelligence SDK for Rust

[![License: GPL-3.0](https://img.shields.io/badge/License-GPL%203.0-blue.svg)](https://opensource.org/licenses/GPL-3.0)

ForgeKit provides a unified SDK for code intelligence operations — graph queries, control flow analysis, safe code editing, and LLM-driven agent workflows — integrated through a single `Forge` instance backed by [magellan](https://github.com/oldnordic/magellan) code graphs.

## Workspace Structure

| Crate | Purpose |
|-------|---------|
| `forge_core` | Core SDK: graph, search, CFG, edit, and analysis APIs |
| `forge_runtime` | File watching, caching, and indexing coordination |
| `forge_agent` | Agent loop, workflow DAG engine, chat providers, and ReAct agent |

## Quick Start

```rust
use forge_core::Forge;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let forge = Forge::open("./my-project").await?;

    let symbols = forge.graph().find_symbol("main").await?;
    for sym in &symbols {
        println!("{} ({}): {}:{}", sym.name, sym.kind, sym.location.file_path.display(), sym.location.line_number);
    }

    let callers = forge.graph().callers_of("my_function").await?;
    println!("Callers: {}", callers.len());

    let impacted = forge.graph()
        .impact_analysis("critical_function", Some(2))
        .await?;
    for sym in impacted {
        println!("{} (hop {}): {}", sym.name, sym.hop_distance, sym.file_path);
    }

    Ok(())
}
```

## Installation

```toml
[dependencies]
forge-core = "0.3"
```

For the agent layer with LLM support:

```toml
[dependencies]
forge-agent = { version = "0.4", features = ["llm-ollama"] }
```

### Feature Flags

**`forge_core`:**
- `sqlite` (default) — SQLite backend via sqlitegraph

**`forge_agent`:**
- `sqlite` (default) — SQLite backend
- `llm-ollama` — Ollama chat provider (gates `reqwest`)
- `llm-openai` — OpenAI chat provider (gates `reqwest`)
- `llm-anthropic` — Anthropic chat provider (gates `reqwest`)
- `envoy` — Multi-agent coordination via Envoy

## Core SDK (`forge_core`)

### Graph Queries

All graph queries go through the magellan code graph database:

```rust
let forge = Forge::open("./project").await?;

let symbols = forge.graph().find_symbol("process_request").await?;
let callers = forge.graph().callers_of("process_request").await?;
let refs = forge.graph().references("MyStruct").await?;
let cycles = forge.graph().cycles().await?;
let impacted = forge.graph().impact_analysis("process_request", Some(2)).await?;
```

### Search

```rust
let search = forge.search();
let results = search.pattern("fn.*test.*\\(").await?;
let semantic = search.semantic("authentication logic").await?;
```

### Control Flow Analysis

```rust
let cfg = forge.cfg();
let doms = cfg.dominators(symbol_id).await?;
let loops = cfg.loops(symbol_id).await?;
let paths = cfg.paths(symbol_id).max_length(10).execute().await?;
```

### Analysis

```rust
let analysis = forge.analysis();
let dead = analysis.find_dead_code().await?;
let metrics = analysis.analyze_source_complexity(source_code);
println!("Complexity: {} ({:?})", metrics.cyclomatic_complexity, metrics.risk_level());
```

### Safe Editing

```rust
let edit = forge.edit();
edit.rename_symbol("old_name", "new_name").await?;
edit.delete_symbol(std::path::Path::new("src/lib.rs"), "unused_fn").await?;
```

## Agent Layer (`forge_agent`)

### Fixed Pipeline (6-Phase)

The deterministic agent loop follows Observe → Constrain → Plan → Mutate → Verify → Commit:

```rust
use forge_agent::Agent;

let agent = Agent::new("./project").await?;
let result = agent.run("Add error handling to the parser").await?;
println!("Transaction: {}", result.transaction_id);
```

### ReAct Agent (LLM-Driven)

An autonomous reasoning-and-acting loop where the LLM decides which tools to call:

```rust
use forge_agent::Agent;
use forge_agent::chat::{OllamaChatProvider, ChatProvider};
use forge_agent::llm::LlmConfig;

let provider = std::sync::Arc::new(
    OllamaChatProvider::new("http://localhost:11434".to_string())
);
let config = LlmConfig::new("qwen3.5-agent:latest".to_string());

let agent = Agent::new("./project").await?
    .with_chat_provider(provider, config);

let answer = agent.run_react("Find all callers of process_request and explain the call chain").await?;
println!("{}", answer);
```

The ReAct agent has access to these tools:
- **file_read** — Read file contents (paths scoped to codebase, traversal blocked)
- **file_write** — Write file contents (creates parent directories)
- **shell_exec** — Execute shell commands (30s timeout, unsandboxed)
- **graph_query** — Query the code graph when Forge SDK is available

### Graph Query Tool

The `graph_query` tool exposes the code graph to the LLM:

| Command | Required Params | Description |
|---------|----------------|-------------|
| `find_symbol` | `name` | Find symbols by name |
| `callers_of` | `name` | Find all callers of a symbol |
| `references` | `name` | Find all cross-file references |
| `cycles` | — | Detect call-graph cycles |
| `impact_analysis` | `name`, `max_hops` (optional) | K-hop impact analysis |

### Chat Providers

| Provider | Feature Flag | Tool Calling | Streaming |
|----------|-------------|--------------|-----------|
| Ollama | `llm-ollama` | Yes | Token-by-token via NDJSON |
| OpenAI | `llm-openai` | Yes | Token-by-token via SSE |
| Anthropic | `llm-anthropic` | Yes | Token-by-token via SSE |

### Workflow Engine

DAG-based task execution with dependency resolution, parallel execution, checkpointing, compensation-based rollback, cancellation, and timeouts:

```rust
use forge_agent::workflow::{Workflow, WorkflowExecutor};

let workflow = Workflow::new()
    .add_task(graph_query_task)
    .add_task(edit_task.depends_on(&[graph_query_task.id()]));

let executor = WorkflowExecutor::new();
let result = executor.execute(workflow).await?;
```

### Multi-Agent Coordination (Envoy)

When the `envoy` feature is enabled, agents can register with an Envoy service for discovery, handoffs, and knowledge sharing via Atheneum.

## Tool Integrations

| Tool | Purpose | Used By |
|------|---------|---------|
| [magellan](https://github.com/oldnordic/magellan) | Code indexing, symbol extraction, call graph | `forge_core` graph/search modules |
| [llmgrep](https://github.com/oldnordic/llmgrep) | Semantic and structural code search | `forge_core` search module |
| [mirage-analyzer](https://crates.io/crates/mirage-analyzer) | CFG construction, dominance, loops, hotspots | `forge_core` CFG module |
| [splice](https://github.com/oldnordic/splice) | Span-safe refactoring, rename, delete | `forge_core` edit module |
| [sqlitegraph](https://crates.io/crates/sqlitegraph) | Typed graph storage over SQLite | Storage backend |

## Known Limitations

- **ShellExecTool is unsandboxed** — Executes arbitrary `sh -c` with full process privileges. No allowlist, no capability restriction.
- **LlmProviderAdapter cannot support tool calling** — The legacy `LlmProvider` trait accepts only a flat prompt string. Use a native `ChatProvider` for agent workflows.
- **Graph queries require a populated database** — If no magellan database exists or the codebase hasn't been indexed, graph methods return empty results. `Forge::open()` auto-indexes on first use when the graph is empty.

## Documentation

- **[API Reference](docs/API.md)** — Complete API documentation
- **[Architecture](docs/ARCHITECTURE.md)** — System design and internals
- **[Changelog](CHANGELOG.md)** — Version history

## License

GPL-3.0 — see [LICENSE](LICENSE).

---

**Current Version:** 0.5.0 (unreleased)

**Status:** Not published to crates.io. APIs may change until v1.0.

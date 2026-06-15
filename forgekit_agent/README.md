# forgekit-agent

[![Crates.io](https://img.shields.io/crates/v/forgekit-agent)](https://crates.io/crates/forgekit-agent)
[![Documentation](https://docs.rs/forgekit-agent/badge.svg)](https://docs.rs/forgekit-agent)
[![License: GPL-3.0](https://img.shields.io/badge/License-GPL%203.0-blue.svg)](https://opensource.org/licenses/GPL-3.0)

**Status: alpha — work in progress. APIs may change until v1.0.**

The agent layer for ForgeKit. Provides a deterministic 6-phase code-mutation
loop (observe → constrain → plan → mutate → verify → commit), an LLM-driven
ReAct tool-calling loop, a workflow DAG engine with rollback, and multi-agent
coordination via Envoy.

> **Alpha notice:** the deterministic loop is implemented and tested end-to-end
> (proven on real crates: symbol rename, dead-code removal). The ReAct loop and
> workflow engine are functional but less battle-tested. See **Known
> Limitations** below for what is unsafe or unfinished.

## What this crate provides

- **Deterministic 6-phase loop** — `Agent::run()`: observes the code graph,
  constrains the symbol set, plans discrete steps (create/modify/delete/rename),
  mutates source, verifies via `cargo check` + `cargo test`, and commits with
  evidence. No LLM needed for the planner core (regex fallback), but wiring an
  LLM via `.forge.toml` produces far better plans.
- **ReAct agent** — `Agent::run_react()`: an autonomous reasoning-and-acting
  loop where an LLM decides which tools to call (`file_read`, `file_write`,
  `shell_exec`, `graph_query`). Requires a chat provider feature flag.
- **Workflow engine** — DAG-based task execution with dependency resolution,
  parallel branches, checkpointing, compensation-based rollback, cancellation,
  and timeouts.
- **Chat providers** — Ollama (`llm-ollama`), OpenAI (`llm-openai`),
  Anthropic (`llm-anthropic`), all with tool-calling and streaming support.
- **Multi-agent coordination** — optional Envoy integration for agent
  discovery, handoffs, and knowledge sharing via Atheneum (`envoy` feature).

## Feature flags

- `sqlite` (default) — SQLite backend via sqlitegraph
- `llm-ollama` — Ollama chat provider (gates `reqwest`)
- `llm-openai` — OpenAI chat provider (gates `reqwest`)
- `llm-anthropic` — Anthropic chat provider (gates `reqwest`)
- `envoy` — Multi-agent coordination via agent-envoy

## Quick Start

### Deterministic loop

```rust
use forgekit_agent::Agent;

let agent = Agent::new("./project").await?;
let result = agent.run("Add error handling to the parser").await?;
println!("Transaction: {}", result.transaction_id);
```

### ReAct agent (LLM-driven)

```rust
use forgekit_agent::Agent;
use forgekit_agent::chat::{OllamaChatProvider, ChatProvider};
use forgekit_agent::llm::LlmConfig;

let provider = std::sync::Arc::new(
    OllamaChatProvider::new("http://localhost:11434".to_string())
);
let config = LlmConfig::new("qwen3.5-agent:latest".to_string());

let agent = Agent::new("./project").await?
    .with_chat_provider(provider, config);

let answer = agent.run_react("Find all callers of process_request and explain the call chain").await?;
println!("{}", answer);
```

## Known Limitations

- **ShellExecTool is unsandboxed** — Executes arbitrary `sh -c` with full
  process privileges. No allowlist, no capability restriction. Do not expose
  to untrusted input.
- **LlmProviderAdapter cannot support tool calling** — The legacy
  `LlmProvider` trait accepts only a flat prompt string. Use a native
  `ChatProvider` for agent workflows.
- **Graph queries require a populated database** — If no magellan database
  exists or the codebase hasn't been indexed, graph methods return empty
  results. `Forge::open()` auto-indexes on first use when the graph is empty.

## Relationship to other crates

| Crate | Role |
|-------|------|
| `forgekit_core` | The SDK foundation: graph, search, CFG, edit, analysis |
| `forgekit_runtime` | Watching, incremental indexing, caching, metrics |
| **`forgekit_agent`** | **This crate: agent loop, ReAct, workflow engine, chat providers** |
| `forgekit-reasoning` | Temporal checkpointing for reasoning tools |

## License

GPL-3.0-only — see [LICENSE.md](../LICENSE.md).

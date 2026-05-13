# LLM-Agnostic Provider Interface

**Date:** 2026-05-13
**Status:** Approved
**Scope:** forge_agent crate

## Context

The forge framework has a deterministic 6-phase agent loop (Observe → Constrain → Plan → Mutate → Verify → Commit). The Observe and Plan phases currently use keyword-based parsing and simple intent detection. An LLM provider would enable semantic understanding of natural language queries and intelligent step generation — while remaining optional so forge works fully offline without an LLM.

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Provider style | Minimal trait, feature-gated backends | Matches existing forge pattern (magellan, llmgrep are feature-gated) |
| Required? | Optional with fallback | forge must work without LLM. Deterministic logic remains the default. |
| Response mode | Request/response only | Streaming added later if needed. Code intelligence tasks are short. |
| Capabilities | Chat completion only | llmgrep already handles embeddings. Don't duplicate. |
| Config | `.forge.toml` `[llm]` section | Matches magellan's config pattern |
| Default provider | Ollama (local-first) | No API key needed. Privacy-preserving. |

## Core Trait

```rust
// forge_agent/src/llm/mod.rs

/// Role of a message in an LLM conversation.
#[derive(Clone, Debug, PartialEq)]
pub enum LlmRole {
    System,
    User,
    Assistant,
}

/// A single message in an LLM conversation.
#[derive(Clone, Debug)]
pub struct LlmMessage {
    pub role: LlmRole,
    pub content: String,
}

/// LLM provider error types.
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("Provider not configured")]
    NotConfigured,
    #[error("Request failed: {0}")]
    RequestFailed(String),
    #[error("Rate limited")]
    RateLimited,
    #[error("Response truncated: {0}")]
    Truncated(String),
}

/// LLM provider trait — abstract over backends.
///
/// Implementations are feature-gated: `llm-openai`, `llm-anthropic`, `llm-ollama`.
/// The agent holds `Option<Arc<dyn LlmProvider>>` — `None` means deterministic-only.
#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    /// Complete a single-turn prompt with an optional system message.
    async fn complete(&self, prompt: &str, system: Option<&str>) -> Result<String, LlmError>;

    /// Complete a multi-turn conversation.
    async fn complete_messages(&self, messages: &[LlmMessage]) -> Result<String, LlmError>;
}
```

## Configuration

`.forge.toml` (new file, optional):

```toml
[llm]
provider = "ollama"          # "openai" | "anthropic" | "ollama"
model = "gemma3:4b"          # provider-specific model name

[llm.options]
base_url = "http://localhost:11434"  # ollama default
# api_key = "sk-..."                  # for openai/anthropic (or FORGE_LLM_API_KEY env var)
max_tokens = 4096
temperature = 0.1
```

Config struct:

```rust
// forge_agent/src/llm/config.rs

#[derive(Clone, Debug, serde::Deserialize)]
pub struct LlmConfig {
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub options: LlmOptions,
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct LlmOptions {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}
```

API key resolution order: config file → `FORGE_LLM_API_KEY` env var → error.

## Provider Factory

```rust
// forge_agent/src/llm/factory.rs

pub fn create_provider(config: &LlmConfig) -> Result<Option<Arc<dyn LlmProvider>>> {
    match config.provider.as_str() {
        #[cfg(feature = "llm-openai")]
        "openai" => Ok(Some(Arc::new(OpenAiProvider::from_config(config)?))),
        #[cfg(feature = "llm-anthropic")]
        "anthropic" => Ok(Some(Arc::new(AnthropicProvider::from_config(config)?))),
        #[cfg(feature = "llm-ollama")]
        "ollama" => Ok(Some(Arc::new(OllamaProvider::from_config(config)?))),
        other => Err(LlmError::NotConfigured),
    }
}
```

## Integration Points

### Agent struct

```rust
pub struct Agent {
    codebase_path: PathBuf,
    forge: Option<forge_core::Forge>,
    llm: Option<Arc<dyn LlmProvider>>,  // NEW
}
```

Injected at construction via `Agent::builder().llm(provider).build()` or auto-loaded from `.forge.toml`.

### Observer enhancement (`observe.rs`)

When `llm` is available:
1. Send query to LLM with system prompt: "Extract symbol names, operation intent, and context from this code intelligence query."
2. Parse LLM response for symbol names
3. Look up parsed names in the graph
4. Merge with semantic search results (existing)

When `llm` is `None`:
- Fall back to current `semantic_search()` + `extract_name_from_query()` logic

### Planner enhancement (`planner.rs`)

When `llm` is available:
1. Send observation (query + discovered symbols with locations) to LLM
2. System prompt: "Generate an execution plan as a list of steps (rename/delete/create/inspect) with target symbols."
3. Parse LLM response into `Vec<PlanStep>`
4. Validate steps against graph (symbol exists, file exists)

When `llm` is `None`:
- Fall back to current `detect_intent()` + keyword-based step generation

### Phases NOT touched

Constrain, Mutate, Verify, and Commit remain fully deterministic. The LLM only influences *what* to do, not *how* to do it.

## Feature Flags

```toml
# forge_agent/Cargo.toml

[features]
default = ["sqlite"]

# LLM providers
llm-ollama = ["dep:reqwest"]
llm-openai = ["dep:reqwest"]
llm-anthropic = ["dep:reqwest"]
llm = ["llm-ollama"]  # convenience: defaults to ollama

[dependencies]
reqwest = { version = "0.12", features = ["json"], optional = true }
# toml crate needed for config parsing (already used by serde_yaml in workspace)
toml = "0.8"
```

## File Structure

```
forge_agent/src/
├── llm/                        # NEW
│   ├── mod.rs                  # Trait, types, errors
│   ├── config.rs               # LlmConfig, TOML parsing
│   ├── factory.rs              # create_provider()
│   └── providers/
│       ├── mod.rs              # Re-exports
│       ├── ollama.rs           # OllamaProvider
│       ├── openai.rs           # OpenAiProvider
│       └── anthropic.rs        # AnthropicProvider
├── observe.rs                  # Modified: LLM-enhanced gathering
├── planner.rs                  # Modified: LLM-enhanced planning
└── lib.rs                      # Modified: Agent gets optional LLM
```

## Provider Implementations

### OllamaProvider

- HTTP POST to `{base_url}/api/chat`
- No API key required (local)
- Request: `{ model, messages, stream: false, options: { temperature, num_predict } }`
- Response: `{ message: { content } }`

### OpenAiProvider

- HTTP POST to `https://api.openai.com/v1/chat/completions` (or custom base_url)
- API key required (`Authorization: Bearer {key}`)
- Request: `{ model, messages, max_tokens, temperature }`
- Response: `{ choices: [{ message: { content } }] }`

### AnthropicProvider

- HTTP POST to `https://api.anthropic.com/v1/messages` (or custom base_url)
- API key required (`x-api-key: {key}`)
- Request: `{ model, system, messages, max_tokens }`
- Response: `{ content: [{ text }] }`

## Testing Strategy

- **Unit tests:** Mock provider implementing `LlmProvider` that returns canned responses
- **Integration tests:** Ollama provider tested against local instance (marked `#[ignore]` if no Ollama running)
- **Fallback tests:** Verify agent loop works identically when `llm: None`
- **Config tests:** TOML parsing, env var resolution, missing config handling

## Success Criteria

1. `cargo test --workspace` passes with no LLM provider configured (zero regression)
2. `cargo test -p forge-agent --features llm-ollama` passes with mock provider
3. Observer returns richer symbol sets with LLM than without
4. Planner generates multi-step plans from natural language queries with LLM
5. 0 clippy warnings, CI green

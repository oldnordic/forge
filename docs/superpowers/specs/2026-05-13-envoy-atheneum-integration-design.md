# Envoy/Atheneum Integration Design

**Date:** 2026-05-13
**Status:** Approved
**Scope:** forge_agent crate

## Context

Forge needs multi-agent coordination. Envoy (HTTP+JSON server at 127.0.0.1:9876) provides agent registry, messaging, handoffs, and atheneum knowledge graph. This spec integrates envoy into forge following the same pattern as the LLM provider: optional via config, feature-gated, `Option<T>` in Agent struct.

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Required or optional | Required when configured | Fail clearly if envoy is down, no silent data loss |
| Transport | HTTP client (reqwest) | Already a dependency, no new crates, simple REST calls |
| Architecture | Module mirror of `llm/` | Validated pattern: one module, one config, one feature flag |
| Discovery storage | Auto-store in agent loop | After observe/plan phases, symbols become atheneum discoveries |
| Registration | Explicit `connect_envoy()` call | No hidden network calls in constructor |
| Disconnect | Best-effort on Drop | Fire-and-forget DELETE, ignore errors during shutdown |

## Files to Create

| File | Purpose |
|------|---------|
| `forge_agent/src/envoy/mod.rs` | EnvoyClient, EnvoyError, MockEnvoyClient, all HTTP methods |
| `forge_agent/src/envoy/config.rs` | EnvoyConfig, TOML parsing for `[envoy]` section |

## Files to Modify

| File | Change |
|------|--------|
| `forge_agent/Cargo.toml` | Add `envoy` feature flag |
| `forge_agent/src/lib.rs` | Add `pub mod envoy;`, add `envoy: Option<EnvoyClient>` to Agent, add `with_envoy()` and `connect_envoy()` |
| `forge_agent/src/loop.rs` | Auto-store discoveries after observe/plan phases |

## Feature Flag

```toml
[features]
envoy = ["dep:reqwest"]
```

Shares reqwest with llm-* features. No new dependencies.

## Config Format

In `.forge.toml`:

```toml
[envoy]
url = "http://127.0.0.1:9876"    # optional, default shown
agent_name = "forge"              # optional, default shown
```

No `[envoy]` section means no envoy integration. EnvoyConfig parsed alongside LlmConfig from the same file.

```rust
pub struct EnvoyConfig {
    pub url: String,
    pub agent_name: String,
}
```

Parsed via `EnvoyConfig::from_file(path)` and `EnvoyConfig::parse(toml_str)`, same pattern as `LlmConfig`.

## EnvoyError

```rust
pub enum EnvoyError {
    ConnectionFailed(String),  // envoy down or unreachable
    RequestFailed(String),     // HTTP error, non-2xx response
    ParseFailed(String),       // malformed JSON response
    NotConfigured,             // called envoy method without envoy configured
    RateLimited,               // 429 from envoy
}
```

No silent fallback. If envoy is configured and a call fails, the error propagates.

## EnvoyClient

```rust
pub struct EnvoyClient {
    url: String,
    agent_id: Mutex<Option<String>>,  // set after register()
    agent_name: String,
    client: reqwest::Client,
}
```

reqwest::Client is reused across calls for connection pooling. agent_id is Mutex<Option<String>> because register() is async and mutates shared state.

### Constructor

```rust
pub fn new(config: EnvoyConfig) -> Self
```

Creates the reqwest client and stores config. Does not connect.

### Lifecycle Methods

```rust
pub async fn register(&self) -> Result<String, EnvoyError>
// POST /agents { "name": agent_name, "kind": "forge" }
// Stores returned agent_id in self.agent_id
// Returns the agent_id

pub async fn disconnect(&self) -> Result<(), EnvoyError>
// DELETE /agents/{agent_id}
// Clears agent_id
```

### Messaging Methods

```rust
pub async fn send_message(&self, to: &str, body: &str) -> Result<(), EnvoyError>
// POST /messages { "type": "direct", "from": agent_id, "to": to, "parts": [{"text": body}] }

pub async fn poll_messages(&self, limit: Option<u32>) -> Result<Vec<EnvoyMessage>, EnvoyError>
// GET /messages?to={agent_id}&limit={limit}
// Returns deserialized message list

pub async fn ack_message(&self, message_id: &str) -> Result<(), EnvoyError>
// POST /messages/{message_id}/ack
```

EnvoyMessage is a flat struct mirroring envoy's MessageEnvelope:

```rust
pub struct EnvoyMessage {
    pub message_id: String,
    pub msg_type: String,
    pub from: String,
    pub to: String,
    pub timestamp: String,
    pub parts: Vec<EnvoyPart>,
}

pub struct EnvoyPart {
    pub text: Option<String>,
    pub data: Option<serde_json::Value>,
}
```

### Atheneum Methods

```rust
pub async fn store_discovery(
    &self,
    discovery_type: &str,   // "Symbol", "CFG", "Issue", "Pattern"
    target: &str,            // symbol name, file path, etc.
    metadata: serde_json::Value,
) -> Result<i64, EnvoyError>
// POST /atheneum/discoveries { "agent": agent_name, "discovery_type": ..., "target": ..., "metadata": ... }
// Returns discovery_id

pub async fn query_knowledge(&self, target: &str) -> Result<KnowledgeResponse, EnvoyError>
// GET /atheneum/knowledge?target={target}
// Returns aggregated knowledge for the target
```

KnowledgeResponse mirrors envoy's response:

```rust
pub struct KnowledgeResponse {
    pub target: String,
    pub discovery_count: usize,
    pub discoveries: Vec<DiscoveryData>,
    pub handoff_count: usize,
    pub handoffs: Vec<HandoffData>,
}

pub struct DiscoveryData {
    pub id: i64,
    pub agent: String,
    pub discovery_type: String,
    pub target: String,
    pub metadata: serde_json::Value,
}

pub struct HandoffData {
    pub id: i64,
    pub from_agent: String,
    pub to_agent: String,
    pub manifest: serde_json::Value,
}
```

### Handoff Methods

```rust
pub async fn get_pending_handoff(&self) -> Result<Option<HandoffData>, EnvoyError>
// GET /atheneum/handoffs/pending?agent={agent_name}
// Returns None if no pending handoff

pub async fn claim_handoff(&self, handoff_id: i64) -> Result<(), EnvoyError>
// POST /atheneum/handoffs/{handoff_id}/claim { "agent": agent_name }
```

## Agent Integration

### Agent Struct

```rust
pub struct Agent {
    codebase_path: PathBuf,
    forge: Option<forge_core::Forge>,
    llm: Option<Arc<dyn llm::LlmProvider>>,
    envoy: Option<envoy::EnvoyClient>,
}
```

### Agent::new()

Loads envoy config from `.forge.toml`. If `[envoy]` section exists, creates an EnvoyClient. The client is not connected yet.

```rust
let envoy = EnvoyConfig::from_file(&config_path)?
    .map(|c| EnvoyClient::new(c));
```

### Builder Method

```rust
pub fn with_envoy(mut self, client: EnvoyClient) -> Self
```

### Connection

```rust
pub async fn connect_envoy(&self) -> Result<String, EnvoyError>
```

Explicit call. Registers the agent with envoy, returns agent_id. Fails if envoy is unreachable.

### Auto-Disconnect

EnvoyClient implements Drop that fires a best-effort `DELETE /agents/{id}`. Uses `std::thread::spawn` to avoid async in Drop. Ignores all errors.

### Discovery Auto-Store in Agent Loop

In `AgentLoop::run()`, after the observe phase completes:

1. If `agent.envoy` is `Some`, iterate observed symbols
2. Convert each to a discovery: `store_discovery("Symbol", symbol.name, metadata)`
3. Fire-and-forget — errors logged but don't fail the agent loop

After the plan phase, store a "Pattern" discovery if the plan has notable characteristics (complexity, affected files count).

## Testing

### MockEnvoyClient

```rust
#[cfg(test)]
pub struct MockEnvoyClient {
    discoveries: Mutex<Vec<(String, String, Value)>>,
    messages: Mutex<Vec<EnvoyMessage>>,
    sent_messages: Mutex<Vec<(String, String)>>,  // (to, body)
    registered: Mutex<bool>,
    agent_id: Mutex<Option<String>>,
}
```

Provides the same methods as EnvoyClient but stores calls in-memory for assertions:
- `store_discovery()` appends to `discoveries`
- `send_message()` appends to `sent_messages`
- `poll_messages()` returns from `messages` (pre-populated by test)
- `register()` sets `registered = true`, returns a fake agent_id
- `query_knowledge()` returns empty KnowledgeResponse

### Test Categories

1. **Config parsing** — valid `[envoy]`, missing section, invalid TOML, defaults
2. **EnvoyClient construction** — from config, default URL
3. **Message serialization** — send_message builds correct JSON, poll_messages parses response
4. **Discovery storage** — store_discovery sends correct payload
5. **Agent integration** — Agent with envoy, Agent without envoy, connect_envoy success/failure
6. **Auto-store** — agent loop stores discoveries when envoy is configured

### No Integration Tests

No tests hit a real envoy server. All HTTP interactions are tested via MockEnvoyClient. The real EnvoyClient methods are thin reqwest wrappers — their correctness depends on envoy's API contract, which is tested in envoy's own test suite.

## Error Handling

| Scenario | Error | Behavior |
|----------|-------|----------|
| Envoy not configured | NotConfigured | Skip envoy operations, agent works normally |
| Envoy down | ConnectionFailed | Fail connect_envoy(), skip auto-store in loop |
| 429 rate limited | RateLimited | Caller backs off |
| Non-2xx response | RequestFailed(status, body) | Propagate to caller |
| Malformed JSON | ParseFailed | Propagate to caller |
| Disconnect fails | Ignored | Best-effort in Drop |

Agent loop behavior: envoy errors during auto-store are logged but don't fail the loop. Only `connect_envoy()` failures propagate to the caller.

## Out of Scope

- WebSocket real-time push (polling is sufficient for v1)
- Retry logic (caller responsibility)
- Agent hierarchy / parent-child relationships
- Message ordering guarantees
- Encryption or authentication beyond what envoy provides

//! Envoy coordination client.
//!
//! Connects to a running envoy server for multi-agent coordination and
//! atheneum knowledge sharing. Automatically registers on first use and
//! includes the `X-Agent-Id` header on all authenticated requests.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

// ── Config ────────────────────────────────────────────────────────────────────

/// Configuration loaded from `.forge.toml`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnvoyConfig {
    pub url: String,
    pub agent_name: String,
}

/// Evidence recording configuration from `[evidence]` section.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvidenceConfig {
    pub tool_name: String,
    #[serde(default)]
    pub project: Option<String>,
}

impl EvidenceConfig {
    pub fn from_file(path: &Path) -> std::io::Result<Option<Self>> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        #[derive(Deserialize)]
        struct Toml {
            #[serde(default)]
            evidence: Option<EvidenceConfig>,
        }
        let parsed: Toml =
            toml::from_str(&text).map_err(|e| std::io::Error::other(e.to_string()))?;
        Ok(parsed.evidence)
    }
}

#[derive(Deserialize)]
struct ForgeToml {
    #[serde(default)]
    envoy: Option<EnvoyConfig>,
}

impl EnvoyConfig {
    /// Read envoy config from a `.forge.toml` file, returning `None` if the
    /// file doesn't exist or has no `[envoy]` section.
    pub fn from_file(path: &Path) -> std::io::Result<Option<Self>> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let parsed: ForgeToml =
            toml::from_str(&text).map_err(|e| std::io::Error::other(e.to_string()))?;
        Ok(parsed.envoy)
    }
}

// ── Client ────────────────────────────────────────────────────────────────────

/// HTTP client for an envoy server.
///
/// Automatically registers with envoy on the first authenticated call and
/// caches the resulting `X-Agent-Id` for subsequent requests.
#[derive(Clone, Debug)]
pub struct EnvoyClient {
    client: Client,
    pub config: EnvoyConfig,
    /// Cached agent ID obtained after registration.
    agent_id: Arc<RwLock<Option<String>>>,
}

impl EnvoyClient {
    pub fn new(config: EnvoyConfig) -> Self {
        Self {
            client: Client::new(),
            config,
            agent_id: Arc::new(RwLock::new(None)),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.config.url.trim_end_matches('/'), path)
    }

    /// Ensures this client is registered and returns the agent ID.
    async fn ensure_registered(&self) -> Result<String, String> {
        // Fast path: already registered
        {
            let id = self.agent_id.read().await;
            if let Some(ref aid) = *id {
                return Ok(aid.clone());
            }
        }
        // Slow path: register
        let aid = self.register_raw().await?;
        *self.agent_id.write().await = Some(aid.clone());
        Ok(aid)
    }

    /// Perform a GET with `X-Agent-Id` auth.
    async fn get_auth(&self, url: &str) -> Result<reqwest::Response, String> {
        let aid = self.ensure_registered().await?;
        self.client
            .get(url)
            .header("x-agent-id", &aid)
            .send()
            .await
            .map_err(|e| format!("GET {url}: {e}"))
    }

    /// Perform a POST with `X-Agent-Id` auth.
    async fn post_auth(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<reqwest::Response, String> {
        let aid = self.ensure_registered().await?;
        self.client
            .post(url)
            .header("x-agent-id", &aid)
            .json(body)
            .send()
            .await
            .map_err(|e| format!("POST {url}: {e}"))
    }

    // ── Health ────────────────────────────────────────────────────────────────

    /// Returns `true` if the envoy server responds to `/health`.
    pub async fn is_healthy(&self) -> bool {
        self.client
            .get(self.url("/health"))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    // ── Agent registration ────────────────────────────────────────────────────

    async fn register_raw(&self) -> Result<String, String> {
        #[derive(Deserialize)]
        struct Resp {
            agent_id: String,
        }
        let payload = serde_json::json!({
            "name": self.config.agent_name,
            "kind": "forge-agent"
        });
        let resp = self
            .client
            .post(self.url("/agents"))
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("register failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("register {status}: {body}"));
        }
        let r: Resp = resp
            .json()
            .await
            .map_err(|e| format!("register parse: {e}"))?;
        Ok(r.agent_id)
    }

    /// Register this agent with envoy. Returns the assigned agent ID.
    pub async fn register(&self) -> Result<String, String> {
        self.ensure_registered().await
    }

    // ── Messaging ─────────────────────────────────────────────────────────────

    /// Send a message to another agent.
    pub async fn send_message(
        &self,
        from: &str,
        to: &str,
        content: serde_json::Value,
    ) -> Result<(), String> {
        let payload = serde_json::json!({
            "type": "message",
            "from": from,
            "to": to,
            "parts": [{"kind": "text", "text": content.to_string()}]
        });
        let resp = self.post_auth(&self.url("/messages"), &payload).await?;

        if resp.status().is_success() {
            Ok(())
        } else {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            Err(format!("send_message {status}: {body}"))
        }
    }

    /// Poll for messages addressed to `agent_name`.
    pub async fn poll_messages(
        &self,
        since: Option<i64>,
    ) -> Result<Vec<serde_json::Value>, String> {
        let mut url = format!("{}?to={}", self.url("/messages"), self.config.agent_name);
        if let Some(s) = since {
            url = format!("{url}&since={s}");
        }
        let resp = self.get_auth(&url).await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("poll_messages {status}: {body}"));
        }
        resp.json::<Vec<serde_json::Value>>()
            .await
            .map_err(|e| format!("poll_messages parse: {e}"))
    }

    // ── Atheneum: discoveries ─────────────────────────────────────────────────

    /// Store a code discovery in atheneum.
    pub async fn store_discovery(
        &self,
        discovery_type: &str,
        target: &str,
        metadata: serde_json::Value,
    ) -> Result<i64, String> {
        #[derive(Deserialize)]
        struct Resp {
            discovery_id: i64,
        }
        let payload = serde_json::json!({
            "agent": self.config.agent_name,
            "discovery_type": discovery_type,
            "target": target,
            "metadata": metadata
        });
        let resp = self
            .post_auth(&self.url("/atheneum/discoveries"), &payload)
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("store_discovery {status}: {body}"));
        }
        let r: Resp = resp
            .json()
            .await
            .map_err(|e| format!("store_discovery parse: {e}"))?;
        Ok(r.discovery_id)
    }

    /// Query atheneum discoveries for a target symbol/concept.
    pub async fn query_discoveries(&self, target: &str) -> Result<Vec<serde_json::Value>, String> {
        #[derive(Deserialize)]
        struct Resp {
            discoveries: Vec<serde_json::Value>,
        }
        let url = format!("{}?target={}", self.url("/atheneum/discoveries"), target);
        let resp = self.get_auth(&url).await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("query_discoveries {status}: {body}"));
        }
        let r: Resp = resp
            .json()
            .await
            .map_err(|e| format!("query_discoveries parse: {e}"))?;
        Ok(r.discoveries)
    }

    // ── Atheneum: knowledge ───────────────────────────────────────────────────

    /// Query the atheneum knowledge graph for a target.
    /// Returns discovery metadata entries, or empty vec if not found.
    pub async fn query_knowledge(&self, target: &str) -> Result<Vec<serde_json::Value>, String> {
        #[derive(Deserialize)]
        struct Resp {
            discoveries: Option<Vec<serde_json::Value>>,
        }
        let url = format!("{}?target={}", self.url("/atheneum/knowledge"), target);
        let resp = self.get_auth(&url).await?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(vec![]);
        }
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("query_knowledge {status}: {body}"));
        }
        let r: Resp = resp
            .json()
            .await
            .map_err(|e| format!("query_knowledge parse: {e}"))?;
        Ok(r.discoveries.unwrap_or_default())
    }

    // ── Atheneum: handoffs ────────────────────────────────────────────────────

    /// Hand off context to another agent.
    pub async fn store_handoff(
        &self,
        to_agent: &str,
        manifest: serde_json::Value,
    ) -> Result<i64, String> {
        #[derive(Deserialize)]
        struct Resp {
            handoff_id: i64,
        }
        let payload = serde_json::json!({
            "from_agent": self.config.agent_name,
            "to_agent": to_agent,
            "manifest": manifest
        });
        let resp = self
            .post_auth(&self.url("/atheneum/handoffs"), &payload)
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("store_handoff {status}: {body}"));
        }
        let r: Resp = resp
            .json()
            .await
            .map_err(|e| format!("store_handoff parse: {e}"))?;
        Ok(r.handoff_id)
    }

    /// Retrieve any pending handoff addressed to this agent.
    pub async fn get_pending_handoff(&self) -> Result<Option<serde_json::Value>, String> {
        let url = format!(
            "{}?agent={}",
            self.url("/atheneum/handoffs/pending"),
            self.config.agent_name
        );
        let resp = self.get_auth(&url).await?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("get_pending_handoff {status}: {body}"));
        }
        let val: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("get_pending_handoff parse: {e}"))?;
        Ok(Some(val))
    }

    // ── Forge Evidence Methods ────────────────────────────────────────────────

    pub async fn forge_prompt(
        &self,
        session_id: &str,
        record: &crate::evidence::PromptRecord,
    ) -> Result<(), String> {
        let body = serde_json::json!({
            "session_id": session_id,
            "sequence": record.sequence,
            "role": record.role,
            "input_hash": record.input_hash,
            "input_tokens": record.input_tokens,
            "output_hash": record.output_hash,
            "output_tokens": record.output_tokens,
            "latency_ms": record.latency_ms,
            "model": record.model,
            "cost_usd": record.cost_usd,
        });
        let resp = self
            .post_auth(&self.url("/atheneum/prompts"), &body)
            .await?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format!("forge_prompt: {}", resp.status()))
        }
    }

    pub async fn forge_tool_call(
        &self,
        session_id: &str,
        record: &crate::evidence::ToolCallEvidence,
    ) -> Result<(), String> {
        let body = serde_json::json!({
            "session_id": session_id,
            "tool_name": record.tool_name,
            "tool_version": record.tool_version,
            "input_hash": record.input_hash,
            "input_summary": record.input_summary,
            "output_hash": record.output_hash,
            "output_summary": record.output_summary,
            "exit_status": record.exit_status,
            "latency_ms": record.latency_ms,
            "input_tokens_est": record.input_tokens_est,
            "tool_category": record.tool_category,
        });
        let resp = self
            .post_auth(&self.url("/atheneum/tool-calls"), &body)
            .await?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format!("forge_tool_call: {}", resp.status()))
        }
    }

    pub async fn forge_file_write(
        &self,
        session_id: &str,
        record: &crate::evidence::FileWriteRecord,
    ) -> Result<(), String> {
        let body = serde_json::json!({
            "session_id": session_id,
            "file_path": record.file_path,
            "file_id": record.file_id,
            "before_hash": record.before_hash,
            "after_hash": record.after_hash,
            "lines_added": record.lines_added,
            "lines_deleted": record.lines_deleted,
            "lines_changed": record.lines_changed,
            "write_type": record.write_type,
        });
        let resp = self
            .post_auth(&self.url("/atheneum/file-writes"), &body)
            .await?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format!("forge_file_write: {}", resp.status()))
        }
    }

    pub async fn forge_commit(
        &self,
        session_id: &str,
        record: &crate::evidence::CommitRecord,
    ) -> Result<(), String> {
        let body = serde_json::json!({
            "session_id": session_id,
            "commit_sha": record.commit_sha,
            "parent_sha": record.parent_sha,
            "message": record.message,
            "author": record.author,
            "files_changed": record.files_changed,
            "lines_inserted": record.lines_inserted,
            "lines_deleted": record.lines_deleted,
            "commit_type": record.commit_type,
            "feature_tag": record.feature_tag,
        });
        let resp = self
            .post_auth(&self.url("/atheneum/commits"), &body)
            .await?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format!("forge_commit: {}", resp.status()))
        }
    }

    pub async fn forge_test_run(
        &self,
        session_id: &str,
        record: &crate::evidence::TestRunRecord,
    ) -> Result<(), String> {
        let body = serde_json::json!({
            "session_id": session_id,
            "test_name": record.test_name,
            "test_suite": record.test_suite,
            "test_command": record.test_command,
            "result": record.result,
            "duration_ms": record.duration_ms,
            "logs_summary": record.logs_summary,
        });
        let resp = self
            .post_auth(&self.url("/atheneum/test-runs"), &body)
            .await?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format!("forge_test_run: {}", resp.status()))
        }
    }

    pub async fn forge_fix_chain(
        &self,
        session_id: &str,
        record: &crate::evidence::FixChainRecord,
    ) -> Result<(), String> {
        let body = serde_json::json!({
            "session_id": session_id,
            "bug_commit_sha": record.bug_commit_sha,
            "fix_commit_sha": record.fix_commit_sha,
            "fix_type": record.fix_type,
            "severity": record.severity,
            "cycles_to_fix": record.cycles_to_fix,
            "time_to_fix_ms": record.time_to_fix_ms,
        });
        let resp = self
            .post_auth(&self.url("/atheneum/fix-chains"), &body)
            .await?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format!("forge_fix_chain: {}", resp.status()))
        }
    }
}

// ── KnowledgeSource impl ──────────────────────────────────────────────────────

#[async_trait::async_trait]
impl crate::observe::KnowledgeSource for EnvoyClient {
    async fn query(&self, target: &str) -> Option<Vec<serde_json::Value>> {
        self.query_knowledge(target)
            .await
            .ok()
            .filter(|v| !v.is_empty())
    }
}

// ── DiscoveryStore impl ───────────────────────────────────────────────────────

#[async_trait::async_trait]
impl crate::agent_loop::DiscoveryStore for EnvoyClient {
    async fn store(&self, discovery_type: &str, target: &str, metadata: serde_json::Value) {
        let _ = self.store_discovery(discovery_type, target, metadata).await;
    }
}

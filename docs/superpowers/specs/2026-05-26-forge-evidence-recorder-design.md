# Forge Evidence Recorder Design

**Date:** 2026-05-26
**Status:** Draft
**Scope:** forge_agent, forge_core
**Preceded by:** `llm-dev-roi-measurement.md` (sqlitegraph case study), `forge-evidence-recorder.md` (atheneum overlap analysis)

## Context

Forge needs to measure LLM-assisted development ROI. Managers need real numbers: features shipped, fix cycles, token cost, LOC survival rate, time-to-quality. Today these metrics require manual git log pipelines. This spec adds automatic evidence recording to forge's agent loop, storing everything as atheneum graph entities via envoy's existing HTTP transport.

Atheneum already has 60% of the schema (Agent, ReasoningLog, ToolCall, FileChange entities with Called/Modified/CausedBy edges, plus SQL tables for agents/reasoning_logs/tool_calls). The envoy client in forge_agent already supports atheneum discoveries, handoffs, and knowledge queries. This spec extends both.

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Storage | Atheneum via envoy HTTP | No new database, no new service, reuses existing causal chain |
| Transport | Existing `EnvoyClient` | Already in forge_agent, already feature-gated, already tested |
| Session model | New `Session` entity | Atheneum's Agent is identity; Session is temporal scope. Agnostic — any tool creates sessions |
| Prompt tracking | Extend existing `ReasoningLog` | ALTER TABLE + extended data, not a parallel table |
| Tool call tracking | Extend existing `ToolCall` | ALTER TABLE + extended data, add category/latency/hashes |
| File write tracking | Extend existing `FileChange` | Add before/after hashes and LOC stats to entity data |
| Commit tracking | New `Commit` entity + post-commit hook in `Committer` | No git integration exists in atheneum |
| Fix chain linking | Use existing `CausedBy` edge type | Wire it up between Commit entities |
| Test/bench tracking | New `TestRun` / `Benchmark` entities | Verification engine already runs tests, just needs capture |
| Release tracking | New `Release` entity | Computed at tag time from aggregated session data |
| Event log | New `event_log` SQL table in atheneum v4 migration | Append-only, never UPDATE/DELETE |
| Naming | Agnostic, not forge-specific | Tables are `sessions`, `commits`, `test_runs`. Routes are `/atheneum/sessions`. Forge is one client. Any tool can record evidence |
| Config | `[evidence]` section in `.forge.toml` | `tool_name` identifies the client. No section = no recording |

## Architecture

```
Agent Loop (existing)
  │
  ├── observe ──→ ForgeSession.start_tool_call("magellan_find", ...)
  │                  └── EnvoyClient.record_tool_call(...)  → POST /atheneum/tool-calls
  │
  ├── plan ──→ ForgeSession.record_prompt(input, output, tokens, cost)
  │               └── EnvoyClient.record_prompt(...)  → POST /atheneum/prompts
  │
  ├── mutate ──→ ForgeSession.record_file_write(path, before_hash, after_hash, diff)
  │                └── EnvoyClient.record_file_write(...)  → POST /atheneum/file-writes
  │
  ├── verify ──→ ForgeSession.record_test_run(name, result, duration_ms)
  │                └── EnvoyClient.record_test_run(...)  → POST /atheneum/test-runs
  │
  └── commit ──→ ForgeSession.record_commit(sha, message, type)
                     ├── Committer.finalize() — existing, adds post-commit hook
                     └── EnvoyClient.record_commit(...)  → POST /atheneum/commits

ForgeSession (new)
  ├── session_id: UUID v7
  ├── started_at, ended_at
  ├── envoy: &EnvoyClient
  ├── prompt_count, tool_call_count, file_write_count
  └── total_input_tokens, total_output_tokens, total_cost_usd
```

## Files to Create

| File | Purpose |
|------|---------|
| `forge_agent/src/evidence/mod.rs` | ForgeSession, EvidenceRecorder, all measurement types |
| `forge_agent/src/evidence/session.rs` | ForgeSession lifecycle (start, end, metrics) |
| `forge_agent/src/evidence/types.rs` | ToolCategory, FixType, Severity, CommitType enums |
| `forge_agent/src/evidence/recorder.rs` | EvidenceRecorder trait + EnvoyEvidenceRecorder impl |

## Files to Modify

| File | Change |
|------|--------|
| `forge_agent/src/lib.rs` | Add `pub mod evidence;`, add `session: Option<evidence::ForgeSession>` to Agent |
| `forge_agent/src/envoy.rs` | Add forge-specific HTTP methods (13 new methods) |
| `forge_agent/src/loop.rs` | Add evidence hooks at each phase transition |
| `forge_agent/src/commit.rs` | Add post-commit hook to record git commit SHA and stats |
| `forge_agent/src/verify.rs` | Add post-test hook to capture individual test results |
| `forge_agent/Cargo.toml` | Add `uuid` dependency (already present), `sha2` (already present) |
| `forge_agent/tests/evidence_tests.rs` | Unit tests for ForgeSession, recorder, types |

No changes to forge_core, forge_runtime, or forge-reasoning. All evidence recording is in forge_agent.

## Feature Flag

```toml
[features]
evidence = ["envoy"]   # evidence requires envoy (reuses reqwest)
```

Evidence recording requires envoy because the storage is atheneum. No local-only mode for v1.

## New Types

### evidence/types.rs

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolCategory {
    GroundedQuery,
    FileRead,
    FileWrite,
    Test,
    Bench,
    Git,
    Shell,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommitType {
    Feature,
    Fix,
    Refactor,
    Test,
    Docs,
    Release,
    Chore,
    Ci,
    Style,
    Merge,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FixType {
    CompileError,
    LogicBug,
    TestFailure,
    Crash,
    Deadlock,
    PerfRegression,
    Style,
    Doc,
    Ci,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PromptRecord {
    pub role: String,
    pub sequence: u32,
    pub input_hash: String,
    pub input_tokens: Option<u64>,
    pub output_hash: Option<String>,
    pub output_tokens: Option<u64>,
    pub latency_ms: Option<u64>,
    pub model: Option<String>,
    pub cost_usd: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub tool_name: String,
    pub tool_version: Option<String>,
    pub input_hash: String,
    pub input_summary: String,
    pub output_hash: Option<String>,
    pub output_summary: Option<String>,
    pub exit_status: String,
    pub latency_ms: u64,
    pub input_tokens_est: Option<u64>,
    pub tool_category: ToolCategory,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileWriteRecord {
    pub file_path: String,
    pub file_id: String,
    pub before_hash: Option<String>,
    pub after_hash: String,
    pub lines_added: u64,
    pub lines_deleted: u64,
    pub lines_changed: u64,
    pub write_type: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommitRecord {
    pub commit_sha: String,
    pub parent_sha: Option<String>,
    pub message: String,
    pub author: String,
    pub files_changed: u64,
    pub lines_inserted: u64,
    pub lines_deleted: u64,
    pub commit_type: CommitType,
    pub feature_tag: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TestRunRecord {
    pub test_name: String,
    pub test_suite: Option<String>,
    pub test_command: String,
    pub result: String,
    pub duration_ms: u64,
    pub logs_summary: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BenchRunRecord {
    pub bench_name: String,
    pub bench_suite: Option<String>,
    pub mean_ns: Option<u64>,
    pub median_ns: Option<u64>,
    pub p95_ns: Option<u64>,
    pub iterations: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FixChainRecord {
    pub bug_commit_sha: String,
    pub fix_commit_sha: String,
    pub fix_type: FixType,
    pub severity: Severity,
    pub cycles_to_fix: u32,
    pub time_to_fix_ms: u64,
}
```

## ForgeSession

### evidence/session.rs

```rust
use crate::evidence::types::*;
use crate::envoy::EnvoyClient;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionMetrics {
    pub session_id: String,
    pub project: String,
    pub trigger: String,
    pub model: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub exit_status: Option<String>,
    pub git_branch: Option<String>,
    pub git_head: Option<String>,
    pub prompt_count: u32,
    pub tool_call_count: u32,
    pub file_write_count: u32,
    pub commit_count: u32,
    pub test_run_count: u32,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cost_usd: f64,
}

pub struct ForgeSession {
    envoy: EnvoyClient,
    metrics: Arc<RwLock<SessionMetrics>>,
    prompt_sequence: AtomicU32,
    tool_call_count: AtomicU32,
    file_write_count: AtomicU32,
    commit_count: AtomicU32,
    test_run_count: AtomicU32,
    total_input_tokens: AtomicU64,
    total_output_tokens: AtomicU64,
    total_cost_usd: Arc<std::sync::Mutex<f64>>,
}
```

### Constructor and Lifecycle

```rust
impl ForgeSession {
    pub fn new(envoy: EnvoyClient, project: &str, model: Option<&str>) -> Self {
        let session_id = Uuid::now_v7().to_string();
        let started_at = chrono::Utc::now().to_rfc3339();
        let git_branch = Self::current_git_branch();
        let git_head = Self::current_git_head();

        let session = Self {
            envoy,
            metrics: Arc::new(RwLock::new(SessionMetrics {
                session_id,
                project: project.to_string(),
                trigger: "cli".to_string(),
                model: model.map(String::from),
                started_at,
                ended_at: None,
                exit_status: None,
                git_branch,
                git_head,
                prompt_count: 0,
                tool_call_count: 0,
                file_write_count: 0,
                commit_count: 0,
                test_run_count: 0,
                total_input_tokens: 0,
                total_output_tokens: 0,
                total_cost_usd: 0.0,
            })),
            prompt_sequence: AtomicU32::new(0),
            tool_call_count: AtomicU32::new(0),
            file_write_count: AtomicU32::new(0),
            commit_count: AtomicU32::new(0),
            test_run_count: AtomicU32::new(0),
            total_input_tokens: AtomicU64::new(0),
            total_output_tokens: AtomicU64::new(0),
            total_cost_usd: Arc::new(std::sync::Mutex::new(0.0)),
        };

        // Fire-and-forget: tell atheneum the session started
        let envoy = session.envoy.clone();
        let metrics = session.metrics.clone();
        tokio::spawn(async move {
            let m = metrics.read().await;
            let _ = envoy.forge_session_start(&m.session_id, &m.project, &m).await;
        });

        session
    }

    pub async fn end(&self, exit_status: &str) {
        let mut m = self.metrics.write().await;
        m.ended_at = Some(chrono::Utc::now().to_rfc3339());
        m.exit_status = Some(exit_status.to_string());
        m.prompt_count = self.prompt_sequence.load(Ordering::Relaxed);
        m.tool_call_count = self.tool_call_count.load(Ordering::Relaxed);
        m.file_write_count = self.file_write_count.load(Ordering::Relaxed);
        m.commit_count = self.commit_count.load(Ordering::Relaxed);
        m.test_run_count = self.test_run_count.load(Ordering::Relaxed);
        m.total_input_tokens = self.total_input_tokens.load(Ordering::Relaxed);
        m.total_output_tokens = self.total_output_tokens.load(Ordering::Relaxed);
        m.total_cost_usd = *self.total_cost_usd.lock().unwrap();

        let _ = self.envoy.forge_session_end(&m.session_id, exit_status, &*m).await;
    }

    fn current_git_branch() -> Option<String> {
        std::process::Command::new("git")
            .args(["branch", "--show-current"])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
    }

    fn current_git_head() -> Option<String> {
        std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
    }
}
```

### Recording Methods

Each method is fire-and-forget (spawned task). Errors are logged but don't fail the agent loop.

```rust
impl ForgeSession {
    pub fn record_prompt(&self, record: PromptRecord) {
        let seq = self.prompt_sequence.fetch_add(1, Ordering::Relaxed);
        if let Some(tokens) = record.input_tokens {
            self.total_input_tokens.fetch_add(tokens, Ordering::Relaxed);
        }
        if let Some(tokens) = record.output_tokens {
            self.total_output_tokens.fetch_add(tokens, Ordering::Relaxed);
        }
        if let Some(cost) = record.cost_usd {
            *self.total_cost_usd.lock().unwrap() += cost;
        }

        let session_id = self.session_id();
        let envoy = self.envoy.clone();
        tokio::spawn(async move {
            let _ = envoy.forge_prompt(&session_id, seq, &record).await;
        });
    }

    pub fn record_tool_call(&self, record: ToolCallRecord) {
        self.tool_call_count.fetch_add(1, Ordering::Relaxed);
        if let Some(tokens) = record.input_tokens_est {
            self.total_input_tokens.fetch_add(tokens, Ordering::Relaxed);
        }

        let session_id = self.session_id();
        let envoy = self.envoy.clone();
        tokio::spawn(async move {
            let _ = envoy.forge_tool_call(&session_id, &record).await;
        });
    }

    pub fn record_file_write(&self, record: FileWriteRecord) {
        self.file_write_count.fetch_add(1, Ordering::Relaxed);

        let session_id = self.session_id();
        let envoy = self.envoy.clone();
        tokio::spawn(async move {
            let _ = envoy.forge_file_write(&session_id, &record).await;
        });
    }

    pub fn record_commit(&self, record: CommitRecord) {
        self.commit_count.fetch_add(1, Ordering::Relaxed);

        let session_id = self.session_id();
        let envoy = self.envoy.clone();
        tokio::spawn(async move {
            let _ = envoy.forge_commit(&session_id, &record).await;
        });
    }

    pub fn record_test_run(&self, record: TestRunRecord) {
        self.test_run_count.fetch_add(1, Ordering::Relaxed);

        let session_id = self.session_id();
        let envoy = self.envoy.clone();
        tokio::spawn(async move {
            let _ = envoy.forge_test_run(&session_id, &record).await;
        });
    }

    pub fn record_fix_chain(&self, record: FixChainRecord) {
        let session_id = self.session_id();
        let envoy = self.envoy.clone();
        tokio::spawn(async move {
            let _ = envoy.forge_fix_chain(&session_id, &record).await;
        });
    }

    fn session_id(&self) -> String {
        // Read session_id from metrics without holding the write lock
        // Use try_lock to avoid blocking
        self.metrics.try_read()
            .map(|m| m.session_id.clone())
            .unwrap_or_default()
    }
}
```

## EnvoyClient Extension

### envoy.rs — New Methods

Add these methods to the existing `EnvoyClient`. All follow the same pattern as existing atheneum methods: build JSON, POST to envoy, return result or error.

```rust
impl EnvoyClient {
    // ── Forge Evidence Methods ──────────────────────────────────────────────

    pub async fn forge_session_start(
        &self,
        session_id: &str,
        project: &str,
        metrics: &SessionMetrics,
    ) -> Result<(), EnvoyError> {
        let body = serde_json::json!({
            "session_id": session_id,
            "project": project,
            "trigger": metrics.trigger,
            "model": metrics.model,
            "started_at": metrics.started_at,
            "git_branch": metrics.git_branch,
            "git_head": metrics.git_head,
        });
        self.post("/atheneum/sessions", &body).await?;
        Ok(())
    }

    pub async fn forge_session_end(
        &self,
        session_id: &str,
        exit_status: &str,
        metrics: &SessionMetrics,
    ) -> Result<(), EnvoyError> {
        let body = serde_json::json!({
            "exit_status": exit_status,
            "ended_at": metrics.ended_at,
            "prompt_count": metrics.prompt_count,
            "tool_call_count": metrics.tool_call_count,
            "file_write_count": metrics.file_write_count,
            "commit_count": metrics.commit_count,
            "test_run_count": metrics.test_run_count,
            "total_input_tokens": metrics.total_input_tokens,
            "total_output_tokens": metrics.total_output_tokens,
            "total_cost_usd": metrics.total_cost_usd,
        });
        self.patch(&format!("/atheneum/sessions/{session_id}"), &body).await?;
        Ok(())
    }

    pub async fn forge_prompt(
        &self,
        session_id: &str,
        sequence: u32,
        record: &PromptRecord,
    ) -> Result<(), EnvoyError> {
        let body = serde_json::json!({
            "session_id": session_id,
            "sequence": sequence,
            "role": record.role,
            "input_hash": record.input_hash,
            "input_tokens": record.input_tokens,
            "output_hash": record.output_hash,
            "output_tokens": record.output_tokens,
            "latency_ms": record.latency_ms,
            "model": record.model,
            "cost_usd": record.cost_usd,
        });
        self.post("/atheneum/prompts", &body).await?;
        Ok(())
    }

    pub async fn forge_tool_call(
        &self,
        session_id: &str,
        record: &ToolCallRecord,
    ) -> Result<(), EnvoyError> {
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
        self.post("/atheneum/tool-calls", &body).await?;
        Ok(())
    }

    pub async fn forge_file_write(
        &self,
        session_id: &str,
        record: &FileWriteRecord,
    ) -> Result<(), EnvoyError> {
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
        self.post("/atheneum/file-writes", &body).await?;
        Ok(())
    }

    pub async fn forge_commit(
        &self,
        session_id: &str,
        record: &CommitRecord,
    ) -> Result<(), EnvoyError> {
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
        self.post("/atheneum/commits", &body).await?;
        Ok(())
    }

    pub async fn forge_test_run(
        &self,
        session_id: &str,
        record: &TestRunRecord,
    ) -> Result<(), EnvoyError> {
        let body = serde_json::json!({
            "session_id": session_id,
            "test_name": record.test_name,
            "test_suite": record.test_suite,
            "test_command": record.test_command,
            "result": record.result,
            "duration_ms": record.duration_ms,
            "logs_summary": record.logs_summary,
        });
        self.post("/atheneum/test-runs", &body).await?;
        Ok(())
    }

    pub async fn forge_fix_chain(
        &self,
        session_id: &str,
        record: &FixChainRecord,
    ) -> Result<(), EnvoyError> {
        let body = serde_json::json!({
            "session_id": session_id,
            "bug_commit_sha": record.bug_commit_sha,
            "fix_commit_sha": record.fix_commit_sha,
            "fix_type": record.fix_type,
            "severity": record.severity,
            "cycles_to_fix": record.cycles_to_fix,
            "time_to_fix_ms": record.time_to_fix_ms,
        });
        self.post("/atheneum/fix-chains", &body).await?;
        Ok(())
    }

    pub async fn forge_dashboard(
        &self,
        project: &str,
    ) -> Result<serde_json::Value, EnvoyError> {
        self.get_json(&format!("/atheneum/dashboard?project={project}")).await
    }

    pub async fn forge_roi(
        &self,
        project: &str,
        from_tag: &str,
        to_tag: &str,
    ) -> Result<serde_json::Value, EnvoyError> {
        self.get_json(&format!(
            "/atheneum/roi?project={project}&from={from_tag}&to={to_tag}"
        )).await
    }
}
```

## Agent Loop Integration

### lib.rs — Add session to Agent

```rust
// In Agent struct (existing):
pub struct Agent {
    codebase_path: PathBuf,
    forge: Option<forge_core::Forge>,
    llm: Option<Arc<dyn llm::LlmProvider>>,
    envoy: Option<envoy::EnvoyClient>,
    session: Option<Arc<evidence::ForgeSession>>,  // NEW
}

// In Agent::run() or AgentLoop::run():
// Before loop starts:
let session = if let Some(ref envoy) = self.envoy {
    Some(Arc::new(evidence::ForgeSession::new(
        envoy.clone(),
        project_name,
        &self.config.evidence.tool_name,
        self.llm.as_ref().map(|p| p.model_name()),
    )))
} else {
    None
};

// After loop ends:
if let Some(ref session) = session {
    session.end(exit_status).await;
}
```

### loop.rs — Phase Hooks

```rust
// At observe phase start:
if let Some(ref session) = self.session {
    session.record_tool_call(ToolCallRecord {
        tool_name: "magellan_find".into(),
        tool_category: ToolCategory::GroundedQuery,
        input_hash: sha256_hex(&query),
        input_summary: format!("--name {} --db {}", symbol, db),
        exit_status: "success".into(),
        latency_ms: elapsed.as_millis() as u64,
        ..Default::default()
    });
}

// At plan phase (LLM call):
if let Some(ref session) = self.session {
    session.record_prompt(PromptRecord {
        role: "user".into(),
        sequence: 0,  // auto-incremented by session
        input_hash: sha256_hex(&prompt_text),
        input_tokens: response.usage.input_tokens,
        output_hash: Some(sha256_hex(&response_text)),
        output_tokens: response.usage.output_tokens,
        latency_ms: Some(elapsed.as_millis() as u64),
        model: Some(model_name.into()),
        cost_usd: Some(computed_cost),
    });
}

// At mutate phase (file edit):
if let Some(ref session) = self.session {
    session.record_file_write(FileWriteRecord {
        file_path: path.to_string_lossy().into(),
        file_id: sha256_hex(path.to_string_lossy().as_bytes()),
        before_hash: Some(sha256_hex(&before_content)),
        after_hash: sha256_hex(&after_content),
        lines_added: diff.added(),
        lines_deleted: diff.deleted(),
        lines_changed: diff.changed(),
        write_type: "edit".into(),
    });
}
```

### commit.rs — Post-Commit Hook

```rust
// In Committer::finalize(), after successful git commit:
if let Some(ref session) = session {
    let sha = self.get_head_sha(working_dir).await?;
    let stats = self.get_commit_stats(working_dir, &sha).await?;
    let commit_type = classify_commit_message(message);

    session.record_commit(CommitRecord {
        commit_sha: sha,
        parent_sha: stats.parent_sha,
        message: message.to_string(),
        author: stats.author,
        files_changed: stats.files_changed,
        lines_inserted: stats.lines_inserted,
        lines_deleted: stats.lines_deleted,
        commit_type,
        feature_tag: extract_feature_tag(message),
    });
}
```

### verify.rs — Post-Test Hook

```rust
// In Verifier::run_tests(), after cargo test completes:
if let Some(ref session) = session {
    for test in &parsed_results {
        session.record_test_run(TestRunRecord {
            test_name: test.name.clone(),
            test_suite: test.module.clone(),
            test_command: test.command.clone(),
            result: if test.passed { "pass" } else { "fail" }.into(),
            duration_ms: test.duration_ms,
            logs_summary: test.failure_message.clone(),
        });
    }
}
```

## Hash Utility

```rust
fn sha256_hex(data: impl AsRef<[u8]>) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data.as_ref());
    format!("{:x}", hasher.finalize())
}
```

## Commit Classification

```rust
fn classify_commit_message(msg: &str) -> CommitType {
    let lower = msg.to_lowercase();
    if lower.starts_with("feat") || lower.contains("feature") { CommitType::Feature }
    else if lower.starts_with("fix") || lower.contains("bug") { CommitType::Fix }
    else if lower.starts_with("refactor") { CommitType::Refactor }
    else if lower.starts_with("test") || lower.contains("bench") { CommitType::Test }
    else if lower.starts_with("docs") { CommitType::Docs }
    else if lower.starts_with("release") || lower.contains("bump") { CommitType::Release }
    else if lower.starts_with("chore") { CommitType::Chore }
    else if lower.starts_with("ci") { CommitType::Ci }
    else if lower.starts_with("style") { CommitType::Style }
    else if lower.starts_with("merge") { CommitType::Merge }
    else { CommitType::Feature }  // default to feature for unrecognized
}

fn extract_feature_tag(msg: &str) -> Option<String> {
    let re = regex::Regex::new(r"(?:feat|fix|refactor)\(([^)]+)\)").ok()?;
    re.captures(msg).map(|c| c[1].to_string())
}
```

## Testing

### MockEvidenceRecorder

```rust
#[cfg(test)]
pub struct MockEvidenceRecorder {
    prompts: std::sync::Mutex<Vec<PromptRecord>>,
    tool_calls: std::sync::Mutex<Vec<ToolCallRecord>>,
    file_writes: std::sync::Mutex<Vec<FileWriteRecord>>,
    commits: std::sync::Mutex<Vec<CommitRecord>>,
    test_runs: std::sync::Mutex<Vec<TestRunRecord>>,
    fix_chains: std::sync::Mutex<Vec<FixChainRecord>>,
}
```

### Test Categories

1. **Session lifecycle** — start, end, metrics aggregation
2. **Token counting** — prompt records accumulate input/output tokens
3. **Cost tracking** — cost_usd sums across prompts
4. **Tool category classification** — grounded_query vs file_read
5. **Commit classification** — conventional commit parsing
6. **Feature tag extraction** — `feat(15-04)` → `Some("15-04")`
7. **SHA-256 hashing** — deterministic, correct output
8. **Fire-and-forget** — recording methods don't block the agent loop
9. **Agent integration** — session created when envoy configured, skipped when not
10. **Graceful degradation** — envoy errors logged, not propagated

### No Integration Tests Against Live Envoy

Same pattern as existing envoy integration: all HTTP interactions tested via mocks. Real envoy routes tested in envoy's own test suite.

## Atheneum v4 Migration (Separate Repo)

The SQL schema changes live in the atheneum repo, not forge. Forge is one client. The v4 migration creates agnostic tables any tool can use:

- `sessions` table
- ALTER `reasoning_logs` (add session_id column)
- ALTER `tool_calls` (add session_id column)
- `commits` table
- `test_runs` table
- `bench_runs` table
- `releases` table
- `fix_chains` table
- `event_log` table (append-only)

### Config-driven enablement

In `.forge.toml` (or any tool's config):

```toml
[evidence]
enabled = true
tool_name = "forge"           # identifies the client in the sessions table
project = "sqlitegraph"       # project scope for queries
```

No `[evidence]` section = no recording.

Full SQL in `.remember/forge-evidence-recorder.md`.

### Envoy Route Additions (Separate Repo)

13 new routes added to envoy's `atheneum_bridge.rs`:

| Method | Path | Body |
|--------|------|------|
| POST | `/atheneum/sessions` | session start payload |
| PATCH | `/atheneum/sessions/:id` | session end payload |
| POST | `/atheneum/prompts` | prompt record |
| POST | `/atheneum/tool-calls` | tool call record |
| POST | `/atheneum/file-writes` | file write record |
| POST | `/atheneum/commits` | commit record |
| POST | `/atheneum/fix-chains` | fix chain record |
| POST | `/atheneum/test-runs` | test run record |
| POST | `/atheneum/bench-runs` | benchmark record |
| POST | `/atheneum/releases` | release record |
| GET | `/atheneum/events` | query event log |
| GET | `/atheneum/dashboard` | project ROI dashboard |
| GET | `/atheneum/roi` | ROI between two releases |

## Implementation Order

| Phase | Crate | Deliverable | Depends On |
|-------|-------|-------------|------------|
| **P1** | forge_agent | `evidence/types.rs` — all record types and enums | Nothing |
| **P2** | forge_agent | `evidence/session.rs` — ForgeSession with lifecycle | P1 |
| **P3** | forge_agent | `evidence/recorder.rs` — EvidenceRecorder trait | P2 |
| **P4** | forge_agent | `envoy.rs` — 13 new HTTP methods | P2 |
| **P5** | forge_agent | `lib.rs` — session field on Agent, creation in run() | P3, P4 |
| **P6** | forge_agent | `loop.rs` — phase hooks for prompt/tool_call/file_write | P5 |
| **P7** | forge_agent | `commit.rs` — post-commit hook, git SHA capture | P5 |
| **P8** | forge_agent | `verify.rs` — post-test hook, test result capture | P5 |
| **P9** | forge_agent | `evidence_tests.rs` — all test categories | P1-P8 |
| **P10** | atheneum | v4 migration — agnostic SQL tables | P1 (types contract) |
| **P11** | envoy | evidence routes in atheneum_bridge | P10 |
| **P12** | forge_agent | Live integration test against envoy+atheneum | P9, P11 |

## Out of Scope (v1)

- Benchmark capture (requires cargo bench output parsing)
- Release aggregation (requires git tag hooks)
- Fix chain auto-detection (requires commit message pattern matching + temporal analysis)
- Cross-project dashboard (atheneum already supports project scoping)
- Local-only mode (evidence without envoy)
- Real-time WebSocket push
- Dashboard UI (API only)

## Metrics This Enables

Once P1-P12 are complete, these metrics are computable from atheneum SQL:

```sql
-- Features per release
SELECT version_tag, features_count, fixes_count FROM releases;

-- First-attempt accuracy
SELECT ROUND(
    COUNT(CASE WHEN cycles_to_fix = 0 THEN 1 END) * 100.0 / COUNT(*), 1
) FROM fix_chains;

-- Token efficiency by tool category
SELECT tool_category, SUM(input_tokens_est), COUNT(*)
FROM tool_calls WHERE session_id IN (SELECT session_id FROM sessions WHERE project = ?)
GROUP BY tool_category;

-- Cost per production LOC
SELECT total_api_cost_usd * 1000.0 / NULLIF(production_loc, 0)
FROM releases WHERE project = ?;

-- LOC survival rate
SELECT SUM(lines_inserted), SUM(lines_deleted)
FROM commits WHERE commit_type = 'feature';

-- Time to fix by severity
SELECT severity, AVG(time_to_fix_ms) / 60000.0
FROM fix_chains GROUP BY severity;
```

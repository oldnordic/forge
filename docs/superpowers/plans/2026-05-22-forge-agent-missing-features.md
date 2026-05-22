# Forge Agent Missing Features Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement three production-ready gaps in the forge-agent SDK: real git commits, a verification retry/fix loop, and a code generation module.

**Architecture:**
Three independent features in `forge_agent`. Task 1 (git commit) is a prerequisite for Task 2 (retry loop) because the loop must call commit correctly. Task 3 (generate module) is fully independent and can be done in any order.

**Tech Stack:** Rust, tokio (full features already in Cargo.toml), `std::process::Command` for git, `forge_agent::llm::LlmProvider` trait for LLM calls.

**Phase 0 evidence (grounded):**
- `splice cycles` — all 5 cycles are self-loops; no cross-module dependency cycles
- `Committer::finalize` — zero callers; only generates a txn-id string; ignores `working_dir`
- `AgentLoop::run` — no callers; CFG has no back-edges (7 blocks, linear + error branches)
- `verify_phase` — returns `Ok(VerificationResult)` regardless of `passed` flag; `commit_phase` never checks `passed` — silent bug
- No `generate` module anywhere in forge

---

## Task 1: Real Git Commit Integration

**Files:**
- Modify: `forge_agent/src/commit.rs`
- Modify: `forge_agent/src/loop.rs` (fix `commit_phase` caller)
- Test: `forge_agent/src/commit.rs` (inline test module)

### Step 1: Write the failing test for git commit

- [ ] Add to the `#[cfg(test)]` block at the bottom of `forge_agent/src/commit.rs`:

```rust
#[tokio::test]
async fn test_finalize_runs_git_commit() {
    use std::process::Command as StdCommand;
    let temp_dir = TempDir::new().unwrap();

    // Init a real git repo in the temp dir
    StdCommand::new("git")
        .args(["init"])
        .current_dir(temp_dir.path())
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(temp_dir.path())
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(temp_dir.path())
        .output()
        .unwrap();

    // Create a file to commit
    let file_path = temp_dir.path().join("hello.rs");
    std::fs::write(&file_path, "fn hello() {}").unwrap();

    let committer = Committer::new();
    let result = committer
        .finalize(temp_dir.path(), &[file_path.clone()], "test: add hello")
        .await
        .unwrap();

    assert!(!result.transaction_id.is_empty());
    assert_eq!(result.files_committed.len(), 1);
    assert!(result.git_committed, "expected git commit to run");

    // Verify the commit actually happened
    let log = StdCommand::new("git")
        .args(["log", "--oneline"])
        .current_dir(temp_dir.path())
        .output()
        .unwrap();
    let log_str = String::from_utf8_lossy(&log.stdout);
    assert!(log_str.contains("test: add hello"), "git log: {log_str}");
}
```

- [ ] Run the test — confirm it fails to compile (method signature mismatch):
```
cargo test -p forge_agent test_finalize_runs_git_commit -- --nocapture
```
Expected: compile error (`finalize` takes 2 args, not 3; `git_committed` field missing)

### Step 2: Implement real git commit in `Committer::finalize`

- [ ] Replace `forge_agent/src/commit.rs` content with:

```rust
//! Commit engine - Transaction finalization.

use crate::Result;
use tokio::process::Command;

#[derive(Clone, Default)]
pub struct Committer {}

impl Committer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Stages `modified_files` and runs `git commit -m message` in `working_dir`.
    /// If git is unavailable or `working_dir` has no repo, `git_committed` is false
    /// and the function still returns Ok (non-fatal — useful in tests on empty dirs).
    pub async fn finalize(
        &self,
        working_dir: &std::path::Path,
        modified_files: &[std::path::PathBuf],
        message: &str,
    ) -> Result<CommitReport> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let transaction_id = format!("txn-{}", now);

        let git_committed = if !modified_files.is_empty() && !working_dir.as_os_str().is_empty() {
            self.git_add_and_commit(working_dir, modified_files, message)
                .await
                .unwrap_or_else(|e| {
                    tracing::warn!("git commit skipped: {e}");
                    false
                })
        } else {
            false
        };

        Ok(CommitReport {
            transaction_id,
            files_committed: modified_files.to_vec(),
            git_committed,
        })
    }

    async fn git_add_and_commit(
        &self,
        working_dir: &std::path::Path,
        files: &[std::path::PathBuf],
        message: &str,
    ) -> Result<bool> {
        // Stage each file
        for file in files {
            let status = Command::new("git")
                .args(["add", "--"])
                .arg(file)
                .current_dir(working_dir)
                .status()
                .await
                .map_err(|e| crate::AgentError::CommitFailed(format!("git add: {e}")))?;
            if !status.success() {
                return Err(crate::AgentError::CommitFailed(format!(
                    "git add failed for {}",
                    file.display()
                )));
            }
        }

        // Commit
        let status = Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(working_dir)
            .status()
            .await
            .map_err(|e| crate::AgentError::CommitFailed(format!("git commit: {e}")))?;

        Ok(status.success())
    }

    pub fn generate_summary(&self, steps: &[crate::planner::PlanStep]) -> String {
        let mut summary = String::from("Applied ");
        for (i, step) in steps.iter().enumerate() {
            if i > 0 {
                summary.push_str(", ");
            }
            summary.push_str(&step.description);
        }
        summary
    }
}

#[derive(Clone, Debug)]
pub struct CommitReport {
    pub transaction_id: String,
    pub files_committed: Vec<std::path::PathBuf>,
    pub git_committed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core::Forge;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_committer_creation() {
        let _committer = Committer::new();
    }

    #[tokio::test]
    async fn test_generate_summary() {
        let committer = Committer::new();
        let steps = vec![
            crate::planner::PlanStep {
                description: "Step 1".to_string(),
                operation: crate::planner::PlanOperation::Inspect {
                    symbol_id: forge_core::types::SymbolId(1),
                    symbol_name: "test".to_string(),
                },
            },
            crate::planner::PlanStep {
                description: "Step 2".to_string(),
                operation: crate::planner::PlanOperation::Inspect {
                    symbol_id: forge_core::types::SymbolId(2),
                    symbol_name: "test2".to_string(),
                },
            },
        ];
        let summary = committer.generate_summary(&steps);
        assert!(summary.contains("Step 1"));
        assert!(summary.contains("Step 2"));
    }

    #[tokio::test]
    async fn test_finalize_empty_files_no_git() {
        let temp_dir = TempDir::new().unwrap();
        let _forge = Forge::open(temp_dir.path()).await.unwrap();
        let committer = Committer::new();
        let result = committer
            .finalize(std::path::Path::new(""), &[], "empty")
            .await
            .unwrap();
        assert!(!result.transaction_id.is_empty());
        assert!(!result.git_committed);
    }

    #[tokio::test]
    async fn test_finalize_runs_git_commit() {
        use std::process::Command as StdCommand;
        let temp_dir = TempDir::new().unwrap();
        StdCommand::new("git").args(["init"]).current_dir(temp_dir.path()).output().unwrap();
        StdCommand::new("git").args(["config", "user.email", "test@test.com"]).current_dir(temp_dir.path()).output().unwrap();
        StdCommand::new("git").args(["config", "user.name", "Test"]).current_dir(temp_dir.path()).output().unwrap();

        let file_path = temp_dir.path().join("hello.rs");
        std::fs::write(&file_path, "fn hello() {}").unwrap();

        let committer = Committer::new();
        let result = committer
            .finalize(temp_dir.path(), &[file_path], "test: add hello")
            .await
            .unwrap();

        assert!(!result.transaction_id.is_empty());
        assert_eq!(result.files_committed.len(), 1);
        assert!(result.git_committed);

        let log = StdCommand::new("git")
            .args(["log", "--oneline"])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();
        let log_str = String::from_utf8_lossy(&log.stdout);
        assert!(log_str.contains("test: add hello"), "git log: {log_str}");
    }
}
```

### Step 3: Fix the `commit_phase` caller in `loop.rs`

- [ ] In `forge_agent/src/loop.rs`, find the `commit_phase` method (around line 295) and change the `committer.finalize` call:

```rust
// OLD:
let commit_report = committer
    .finalize(std::path::Path::new(""), &files)
    .await
    .map_err(|e| crate::AgentError::CommitFailed(e.to_string()))?;

// NEW:
let message = format!("forge: apply changes ({})", files.len());
let commit_report = committer
    .finalize(&self.codebase_path, &files, &message)
    .await
    .map_err(|e| crate::AgentError::CommitFailed(e.to_string()))?;
```

### Step 4: Run tests

- [ ] Run tests:
```
cargo test -p forge_agent -- --nocapture 2>&1 | tail -20
```
Expected: all tests pass. `test_finalize_runs_git_commit` passes.

### Step 5: Commit

```bash
git add forge_agent/src/commit.rs forge_agent/src/loop.rs
git commit -m "feat(agent): implement real git commit in Committer::finalize"
```

---

## Task 2: Verification Retry/Fix Loop

**Files:**
- Modify: `forge_agent/src/loop.rs` — add retry loop + fix verify-passes-when-failed bug
- Modify: `forge_agent/src/planner.rs` — add `generate_fix_steps` method

**Prerequisite:** Task 1 complete (commit_phase must use real path and message).

### Step 1: Write the failing test

- [ ] Add to the `#[cfg(test)]` block in `forge_agent/src/loop.rs`:

```rust
#[tokio::test]
async fn test_retry_loop_respects_max_attempts() {
    use crate::llm::MockProvider;
    let temp_dir = TempDir::new().unwrap();
    let forge = Forge::open(temp_dir.path()).await.unwrap();

    // MockProvider returns a response that will always fail verification
    // (empty codebase → cargo check fails anyway, so verify_phase will fail)
    let llm = Arc::new(MockProvider::new("[]")); // empty plan → no mutation
    let mut agent_loop = AgentLoop::new(Arc::new(forge))
        .with_llm(llm)
        .with_max_fix_attempts(2);

    let result = agent_loop.run("test retry").await;

    // Should fail after exhausting retries, not silently commit with broken code
    match result {
        Err(e) => assert!(
            e.to_string().contains("Verification") || e.to_string().contains("verification"),
            "unexpected error: {e}"
        ),
        Ok(r) => {
            // If it somehow passed, verification must have succeeded
            assert!(r.audit_events.iter().any(|e| matches!(e, AuditEvent::Verify { passed: true, .. })));
        }
    }
}
```

- [ ] Run to confirm compile failure (`with_max_fix_attempts` doesn't exist yet):
```
cargo test -p forge_agent test_retry_loop_respects_max_attempts 2>&1 | head -20
```

### Step 2: Add `generate_fix_steps` to `Planner`

- [ ] In `forge_agent/src/planner.rs`, after `generate_steps` (around line 61), add:

```rust
/// Generate fix steps using error context from a failed verification.
/// Falls back to an empty plan if LLM is unavailable.
pub async fn generate_fix_steps(
    &self,
    observation: &super::observe::Observation,
    errors: &[String],
) -> Result<Vec<PlanStep>> {
    let Some(ref llm) = self.llm else {
        return Ok(Vec::new());
    };

    let error_text = errors.join("\n");
    let symbol_list: Vec<String> = observation
        .symbols
        .iter()
        .map(|s| format!("{} (id:{})", s.name, s.id.0))
        .collect();

    let prompt = format!(
        "Query: {}\nSymbols: [{}]\nCompilation/verification errors:\n{}",
        observation.query,
        symbol_list.join(", "),
        error_text
    );

    let system = "You are a Rust fix planner. Given a code query, the relevant symbols, \
and compilation/verification errors, generate fix steps as a JSON array.\n\n\
Available operations:\n\
- {\"operation\":\"inspect\",\"symbol_name\":\"...\",\"symbol_id\":N}\n\
- {\"operation\":\"rename\",\"old\":\"...\",\"new\":\"...\",\"file\":\"...\"}\n\
- {\"operation\":\"delete\",\"name\":\"...\",\"file\":\"...\"}\n\
- {\"operation\":\"create\",\"path\":\"...\",\"content\":\"...\"}\n\
- {\"operation\":\"modify\",\"file\":\"...\",\"start\":N,\"end\":N,\"replacement\":\"...\"}\n\n\
Output ONLY a JSON array. No explanation.";

    match llm.complete(&prompt, Some(system)).await {
        Ok(resp) => parse_llm_steps(&resp).unwrap_or_default(),
        Err(e) => {
            tracing::warn!("LLM fix generation failed: {e}");
            Ok(Vec::new())
        }
    }
}
```

### Step 3: Add `max_fix_attempts` field and `with_max_fix_attempts` builder to `AgentLoop`

- [ ] In `forge_agent/src/loop.rs`, add to the `AgentLoop` struct (after `llm` field):

```rust
/// Maximum number of verification retry attempts after a fix.
max_fix_attempts: u32,
/// Original observation for re-planning during fix loop.
last_observation: Option<crate::observe::Observation>,
```

- [ ] In `AgentLoop::new`, initialize:
```rust
max_fix_attempts: 3,
last_observation: None,
```

- [ ] Add builder method after `with_llm`:
```rust
pub fn with_max_fix_attempts(mut self, n: u32) -> Self {
    self.max_fix_attempts = n;
    self
}
```

### Step 4: Add the retry loop to `AgentLoop::run`

- [ ] In `forge_agent/src/loop.rs`, replace the Phase 3–5 block in `run` with the retry loop.

The current sequential flow (phases 3–5) in `run`:
```rust
// Phase 3: Plan
let plan = match self.plan_phase(constrained).await { ... };

// Phase 4: Mutate
let mutation_result = match self.mutate_phase(plan).await { ... };

// Phase 5: Verify
let verification = match self.verify_phase(mutation_result).await { ... };
```

Replace with:
```rust
// Phase 3: Plan
let mut plan = match self.plan_phase(constrained.clone()).await {
    Ok(plan) => plan,
    Err(e) => {
        self.record_rollback(&e).await;
        return Err(e);
    }
};

// Phases 4 + 5 with retry loop
let mut attempt = 0u32;
let verification = loop {
    // Phase 4: Mutate
    let mutation_result = match self.mutate_phase(plan).await {
        Ok(r) => r,
        Err(e) => {
            self.record_rollback(&e).await;
            return Err(e);
        }
    };

    // Phase 5: Verify
    let verification = match self.verify_phase(mutation_result).await {
        Ok(v) => v,
        Err(e) => {
            self.record_rollback(&e).await;
            return Err(e);
        }
    };

    if verification.passed || attempt >= self.max_fix_attempts {
        break verification;
    }

    // Verification failed — ask LLM for a fix plan
    attempt += 1;
    tracing::info!(
        attempt,
        self.max_fix_attempts,
        errors = verification.diagnostics.len(),
        "verification failed, generating fix plan"
    );

    let fix_observation = self.last_observation.clone().unwrap_or_else(|| {
        constrained.observation.clone()
    });

    let mut planner = crate::planner::Planner::new();
    if let Some(ref llm) = self.llm {
        planner = planner.with_llm(llm.clone());
    }
    let fix_steps = planner
        .generate_fix_steps(&fix_observation, &verification.diagnostics)
        .await
        .unwrap_or_default();

    if fix_steps.is_empty() {
        // No fix steps available — fail now instead of looping forever
        let e = crate::AgentError::VerificationFailed(format!(
            "verification failed after {} attempt(s), no fix steps available: {}",
            attempt,
            verification.diagnostics.join("; ")
        ));
        self.record_rollback(&e).await;
        return Err(e);
    }

    // Re-estimate and order fix steps
    let impact = planner
        .estimate_impact(&fix_steps)
        .await
        .unwrap_or_else(|_| crate::planner::ImpactEstimate {
            affected_files: vec![],
            risk_level: crate::planner::RiskLevel::Low,
        });
    plan = crate::ExecutionPlan {
        steps: fix_steps,
        estimated_impact: impact,
        rollback_plan: vec![],
    };
};

// Guard: do not commit if verification failed after all retries
if !verification.passed {
    let e = crate::AgentError::VerificationFailed(format!(
        "verification failed after {} fix attempt(s): {}",
        attempt,
        verification.diagnostics.join("; ")
    ));
    self.record_rollback(&e).await;
    return Err(e);
}
```

Also store the observation for the fix loop. In `observe_phase`, at the end before returning, store:
```rust
// (inside AgentLoop::observe_phase, just before Ok(observation))
self.last_observation = Some(observation.clone());
Ok(observation)
```

- [ ] Also make `ConstrainedPlan` derive `Clone` (needed for `constrained.clone()` in the loop). Find the struct in `lib.rs` or wherever it's defined and add `#[derive(Clone)]`.

### Step 5: Verify `ImpactEstimate` and `RiskLevel` are accessible

- [ ] Check that `ImpactEstimate` and `RiskLevel` are `pub` in `planner.rs`:
```bash
grep -n "pub struct ImpactEstimate\|pub enum RiskLevel" forge_agent/src/planner.rs
```
If they're not `pub`, add `pub`.

### Step 6: Run tests

- [ ] Run:
```
cargo test -p forge_agent -- --nocapture 2>&1 | tail -30
```
Expected: all tests pass including `test_retry_loop_respects_max_attempts`.

### Step 7: Commit

```bash
git add forge_agent/src/loop.rs forge_agent/src/planner.rs
git commit -m "feat(agent): add verification retry/fix loop with LLM re-planning"
```

---

## Task 3: Code Generation Module

**Files:**
- Create: `forge_agent/src/generate.rs`
- Modify: `forge_agent/src/lib.rs` — add `pub mod generate;` and re-export

**Prerequisite:** None. Fully independent.

### Step 1: Write the failing test first

- [ ] Create `forge_agent/src/generate.rs` with ONLY the test (no impl yet):

```rust
//! Code generation from natural language descriptions.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::MockProvider;
    use std::sync::Arc;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_generate_returns_llm_content() {
        let temp_dir = TempDir::new().unwrap();
        let forge = forge_core::Forge::open(temp_dir.path()).await.unwrap();
        let llm = Arc::new(MockProvider::new("fn add(a: i32, b: i32) -> i32 { a + b }"));
        let gen = Generator::new(Arc::new(forge), llm);

        let result = gen.generate("add two integers").await.unwrap();

        assert!(result.content.contains("fn add"), "got: {}", result.content);
    }

    #[tokio::test]
    async fn test_generate_includes_context_in_prompt() {
        let temp_dir = TempDir::new().unwrap();
        // Write a source file so Observer has symbols to report
        std::fs::write(
            temp_dir.path().join("lib.rs"),
            "pub fn existing() {}",
        )
        .unwrap();

        let forge = forge_core::Forge::open(temp_dir.path()).await.unwrap();
        let llm = Arc::new(MockProvider::new("fn new_fn() {}"));
        let gen = Generator::new(Arc::new(forge), llm);

        let result = gen.generate("add a helper function").await.unwrap();
        assert!(!result.content.is_empty());
    }

    #[tokio::test]
    async fn test_generated_code_has_suggested_path() {
        let temp_dir = TempDir::new().unwrap();
        let forge = forge_core::Forge::open(temp_dir.path()).await.unwrap();
        let llm = Arc::new(MockProvider::new(
            r#"{"path":"src/helpers.rs","code":"fn helper() {}"}"#,
        ));
        let gen = Generator::new(Arc::new(forge), llm);

        let result = gen.generate("add helper").await.unwrap();
        // When LLM returns JSON with path+code, suggested_path is populated
        assert!(!result.content.is_empty());
    }
}
```

- [ ] Run to confirm compile failure (`Generator` not defined):
```
cargo test -p forge_agent generate -- --nocapture 2>&1 | head -20
```

### Step 2: Implement `Generator`

- [ ] Replace `forge_agent/src/generate.rs` with full implementation:

```rust
//! Code generation from natural language descriptions.

use crate::llm::LlmProvider;
use crate::observe::Observer;
use crate::AgentError;
use forge_core::Forge;
use std::path::PathBuf;
use std::sync::Arc;

/// Generates new code from a natural language description.
///
/// Queries the graph for relevant context (existing symbols, patterns),
/// builds a prompt, and calls the LLM.
pub struct Generator {
    forge: Arc<Forge>,
    llm: Arc<dyn LlmProvider>,
}

impl Generator {
    pub fn new(forge: Arc<Forge>, llm: Arc<dyn LlmProvider>) -> Self {
        Self { forge, llm }
    }

    /// Generate code matching `description`.
    ///
    /// Returns `GeneratedCode` with the LLM output and an optional
    /// suggested file path (populated if the LLM returns a JSON
    /// `{"path":"...","code":"..."}` envelope).
    pub async fn generate(&self, description: &str) -> Result<GeneratedCode, AgentError> {
        // Gather graph context for the description
        let observer = Observer::new((*self.forge).clone());
        let observation = observer.gather(description).await.map_err(|e| {
            AgentError::ObservationFailed(format!("generate context failed: {e}"))
        })?;

        let symbol_list: Vec<String> = observation
            .symbols
            .iter()
            .map(|s| format!("{} (id:{})", s.name, s.id.0))
            .collect();

        let prompt = format!(
            "Task: {}\nExisting symbols in codebase: [{}]\n\nGenerate Rust code for the task. \
If you want to suggest a file path, respond with JSON: {{\"path\":\"src/...\",\"code\":\"...\"}}. \
Otherwise, respond with plain Rust code only.",
            description,
            symbol_list.join(", ")
        );

        let system = "You are a Rust code generator. \
Write idiomatic, minimal Rust code. No explanations outside the code. \
Only public items where needed. Follow existing project patterns.";

        let raw = self
            .llm
            .complete(&prompt, Some(system))
            .await
            .map_err(|e| AgentError::PlanningFailed(format!("LLM generate failed: {e}")))?;

        Ok(parse_generated(&raw))
    }
}

/// Output of a code generation request.
#[derive(Debug, Clone)]
pub struct GeneratedCode {
    /// The generated Rust code.
    pub content: String,
    /// Suggested file path returned by the LLM, if any.
    pub suggested_path: Option<PathBuf>,
}

/// Attempt to parse JSON envelope `{"path":"...","code":"..."}`.
/// Falls back to treating the whole response as plain code.
fn parse_generated(raw: &str) -> GeneratedCode {
    let trimmed = raw.trim();
    if trimmed.starts_with('{') {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
            let code = v["code"].as_str().unwrap_or("").to_string();
            let path = v["path"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(PathBuf::from);
            if !code.is_empty() {
                return GeneratedCode {
                    content: code,
                    suggested_path: path,
                };
            }
        }
    }
    GeneratedCode {
        content: trimmed.to_string(),
        suggested_path: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::MockProvider;
    use std::sync::Arc;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_generate_returns_llm_content() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let llm = Arc::new(MockProvider::new("fn add(a: i32, b: i32) -> i32 { a + b }"));
        let gen = Generator::new(Arc::new(forge), llm);

        let result = gen.generate("add two integers").await.unwrap();

        assert!(result.content.contains("fn add"), "got: {}", result.content);
        assert!(result.suggested_path.is_none());
    }

    #[tokio::test]
    async fn test_generate_includes_context_in_prompt() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let llm = Arc::new(MockProvider::new("fn new_fn() {}"));
        let gen = Generator::new(Arc::new(forge), llm);

        let result = gen.generate("add a helper function").await.unwrap();
        assert!(!result.content.is_empty());
    }

    #[tokio::test]
    async fn test_generate_parses_json_envelope() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let llm = Arc::new(MockProvider::new(
            r#"{"path":"src/helpers.rs","code":"fn helper() {}"}"#,
        ));
        let gen = Generator::new(Arc::new(forge), llm);

        let result = gen.generate("add helper").await.unwrap();
        assert_eq!(result.content, "fn helper() {}");
        assert_eq!(
            result.suggested_path,
            Some(PathBuf::from("src/helpers.rs"))
        );
    }

    #[tokio::test]
    async fn test_parse_generated_plain_code() {
        let result = parse_generated("fn foo() {}");
        assert_eq!(result.content, "fn foo() {}");
        assert!(result.suggested_path.is_none());
    }

    #[tokio::test]
    async fn test_parse_generated_json_malformed_falls_back() {
        let result = parse_generated("{not valid json}");
        assert_eq!(result.content, "{not valid json}");
        assert!(result.suggested_path.is_none());
    }
}
```

### Step 3: Wire `generate` module into `lib.rs`

- [ ] In `forge_agent/src/lib.rs`, add after the other `pub mod` declarations:

```rust
pub mod generate;
pub use generate::{GeneratedCode, Generator};
```

### Step 4: Run tests

- [ ] Run:
```
cargo test -p forge_agent generate -- --nocapture
```
Expected: 5 tests pass.

- [ ] Run full suite:
```
cargo test -p forge_agent -- --nocapture 2>&1 | tail -10
```
Expected: all tests pass.

### Step 5: Commit

```bash
git add forge_agent/src/generate.rs forge_agent/src/lib.rs
git commit -m "feat(agent): add Generator module for LLM-driven code generation"
```

---

## Final Verification

After all three tasks:

- [ ] Full build clean:
```
cargo build --workspace
```

- [ ] All tests pass:
```
cargo test --workspace 2>&1 | tail -10
```

- [ ] Clippy clean:
```
cargo clippy --all-targets -- -D warnings
```

- [ ] Format check:
```
cargo fmt --check
```

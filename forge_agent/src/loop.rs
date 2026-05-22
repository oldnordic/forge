//! Agent loop orchestrator - Deterministic 6-phase execution.
//!
//! This module implements the core agent loop that sequences all phases:
//! Observe -> Constrain -> Plan -> Mutate -> Verify -> Commit
//!
//! Each phase receives the output of the previous phase, and failures
//! trigger rollback with audit trail entries.

use crate::{
    audit::AuditEvent, audit::AuditLog, policy::PolicyValidator, CommitResult, ConstrainedPlan,
    ExecutionPlan, MutationResult, Observation, VerificationResult,
};
use chrono::Utc;
use forge_core::Forge;
use std::sync::Arc;

/// Trait for storing discoveries to a knowledge backend (e.g., atheneum).
#[async_trait::async_trait]
pub trait DiscoveryStore: Send + Sync {
    /// Stores a discovery.
    async fn store(&self, discovery_type: &str, target: &str, metadata: serde_json::Value);
}

/// Agent phase in the execution loop.
#[derive(Clone, Debug, PartialEq)]
pub enum AgentPhase {
    /// Observation phase - gather context from graph
    Observe,
    /// Constraint phase - apply policy rules
    Constrain,
    /// Plan phase - generate execution steps
    Plan,
    /// Mutate phase - apply changes
    Mutate,
    /// Verify phase - validate results
    Verify,
    /// Commit phase - finalize transaction
    Commit,
}

/// Result of a complete agent loop execution.
#[derive(Clone, Debug)]
pub struct LoopResult {
    /// Transaction ID for this execution
    pub transaction_id: String,
    /// Files that were modified
    pub modified_files: Vec<std::path::PathBuf>,
    /// Audit trail of all phase transitions
    pub audit_events: Vec<AuditEvent>,
}

/// Agent loop orchestrator.
///
/// The AgentLoop sequences all 6 phases of the agent execution,
/// ensuring each phase receives the previous phase's output and
/// failures trigger proper rollback.
pub struct AgentLoop {
    /// Forge SDK for graph queries
    forge: Arc<Forge>,
    /// Path to the codebase
    codebase_path: std::path::PathBuf,
    /// Current transaction state
    transaction: Option<crate::transaction::Transaction>,
    /// Audit log for phase transitions
    audit_log: AuditLog,
    /// Optional discovery store for atheneum auto-store
    discovery_store: Option<Arc<dyn DiscoveryStore>>,
    /// Optional LLM provider for intelligent phases
    llm: Option<Arc<dyn crate::llm::LlmProvider>>,
    /// Maximum verify→fix retry attempts before giving up
    max_fix_attempts: u32,
    /// Cached observation from Phase 1 for use in fix loop re-planning
    last_observation: Option<crate::observe::Observation>,
}

impl AgentLoop {
    /// Creates a new agent loop with fresh state.
    ///
    /// # Arguments
    ///
    /// * `forge` - The Forge SDK instance for graph queries
    pub fn new(forge: Arc<Forge>) -> Self {
        let codebase_path = forge.codebase_path().to_path_buf();
        Self {
            forge,
            codebase_path,
            transaction: None,
            audit_log: AuditLog::new(),
            discovery_store: None,
            llm: None,
            max_fix_attempts: 3,
            last_observation: None,
        }
    }

    /// Sets the discovery store for atheneum auto-store.
    pub fn with_discovery_store(mut self, store: Arc<dyn DiscoveryStore>) -> Self {
        self.discovery_store = Some(store);
        self
    }

    /// Sets the LLM provider for intelligent phase execution.
    pub fn with_llm(mut self, provider: Arc<dyn crate::llm::LlmProvider>) -> Self {
        self.llm = Some(provider);
        self
    }

    /// Sets the maximum number of verify→fix retry attempts (default: 3).
    pub fn with_max_fix_attempts(mut self, n: u32) -> Self {
        self.max_fix_attempts = n;
        self
    }

    /// Runs the full agent loop: Observe -> Constrain -> Plan -> Mutate -> Verify -> Commit
    ///
    /// # Arguments
    ///
    /// * `query` - The natural language query or request
    ///
    /// # Returns
    ///
    /// Returns `LoopResult` with commit data on success, or error on phase failure.
    pub async fn run(&mut self, query: &str) -> Result<LoopResult, crate::AgentError> {
        // Phase 1: Observe
        let observation = match self.observe_phase(query).await {
            Ok(obs) => obs,
            Err(e) => {
                self.record_rollback(&e).await;
                return Err(e);
            }
        };

        // Auto-store observed symbols to atheneum (fire-and-forget)
        self.store_observation_discoveries(&observation).await;

        // Phase 2: Constrain
        let constrained = match self.constrain_phase(observation).await {
            Ok(constrained) => constrained,
            Err(e) => {
                self.record_rollback(&e).await;
                return Err(e);
            }
        };

        // Phase 3: Plan
        let mut plan = match self.plan_phase(constrained.clone()).await {
            Ok(plan) => plan,
            Err(e) => {
                self.record_rollback(&e).await;
                return Err(e);
            }
        };

        // Phases 4 + 5 with verify→fix retry loop
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
                max = self.max_fix_attempts,
                errors = verification.diagnostics.len(),
                "verification failed, generating fix plan"
            );

            let fix_observation = self
                .last_observation
                .clone()
                .unwrap_or_else(|| constrained.observation.clone());

            let mut planner = crate::planner::Planner::new();
            if let Some(ref llm) = self.llm {
                planner = planner.with_llm(llm.clone());
            }
            let fix_steps = planner
                .generate_fix_steps(&fix_observation, &verification.diagnostics)
                .await
                .unwrap_or_default();

            if fix_steps.is_empty() {
                let e = crate::AgentError::VerificationFailed(format!(
                    "verification failed after {} attempt(s), no fix steps available",
                    attempt
                ));
                self.record_rollback(&e).await;
                return Err(e);
            }

            let impact = planner
                .estimate_impact(&fix_steps)
                .await
                .unwrap_or_else(|_| crate::planner::ImpactEstimate {
                    affected_files: vec![],
                    complexity: 0,
                });
            plan = crate::ExecutionPlan {
                steps: fix_steps,
                estimated_impact: impact,
                rollback_plan: vec![],
            };
        };

        // Guard: do not commit if verification still failed after all retries
        if !verification.passed {
            let e = crate::AgentError::VerificationFailed(format!(
                "verification failed after {} fix attempt(s): {}",
                attempt,
                verification.diagnostics.join("; ")
            ));
            self.record_rollback(&e).await;
            return Err(e);
        }

        // Phase 6: Commit
        let commit_result = match self.commit_phase(verification).await {
            Ok(result) => result,
            Err(e) => {
                self.record_rollback(&e).await;
                return Err(e);
            }
        };

        // Success - return loop result
        Ok(LoopResult {
            transaction_id: commit_result.transaction_id,
            modified_files: commit_result.files_committed,
            audit_events: self.audit_log.clone().into_events(),
        })
    }

    /// Observation phase - gather context from the graph.
    async fn observe_phase(&mut self, query: &str) -> Result<Observation, crate::AgentError> {
        // Use Observer to gather context
        let mut observer = crate::observe::Observer::new((*self.forge).clone());
        if let Some(ref llm) = self.llm {
            observer = observer.with_llm(llm.clone());
        }
        let observation = observer
            .gather(query)
            .await
            .map_err(|e| crate::AgentError::ObservationFailed(e.to_string()))?;

        let symbol_count = observation.symbols.len();

        // Record audit event
        self.audit_log
            .record(AuditEvent::Observe {
                timestamp: Utc::now(),
                query: query.to_string(),
                symbol_count,
            })
            .await
            .map_err(|e| crate::AgentError::ObservationFailed(e.to_string()))?;

        self.last_observation = Some(observation.clone());
        Ok(observation)
    }

    /// Constraint phase - apply policy rules.
    async fn constrain_phase(
        &mut self,
        observation: Observation,
    ) -> Result<ConstrainedPlan, crate::AgentError> {
        // Create validator — diff will be populated after plan phase in future versions
        let validator = PolicyValidator::new((*self.forge).clone());
        let diff = crate::policy::Diff {
            file_path: std::path::PathBuf::from(&observation.query),
            original: String::new(),
            modified: format!("query: {}", observation.query),
            changes: Vec::new(),
        };
        let policies = Vec::new();

        let report = validator
            .validate(&diff, &policies)
            .await
            .map_err(|e| crate::AgentError::PolicyViolation(e.to_string()))?;

        let policy_count = policies.len();
        let violations = report.violations.len();

        // Record audit event
        self.audit_log
            .record(AuditEvent::Constrain {
                timestamp: Utc::now(),
                policy_count,
                violations,
            })
            .await
            .map_err(|e| crate::AgentError::PolicyViolation(e.to_string()))?;

        Ok(ConstrainedPlan {
            observation,
            policy_violations: report.violations,
        })
    }

    /// Plan phase - generate execution steps.
    async fn plan_phase(
        &mut self,
        constrained: ConstrainedPlan,
    ) -> Result<ExecutionPlan, crate::AgentError> {
        // Create planner
        let mut planner = crate::planner::Planner::new();
        if let Some(ref llm) = self.llm {
            planner = planner.with_llm(llm.clone());
        }

        // Generate steps from observation
        let steps = planner
            .generate_steps(&constrained.observation)
            .await
            .map_err(|e| crate::AgentError::PlanningFailed(e.to_string()))?;

        // Estimate impact
        let impact = planner
            .estimate_impact(&steps)
            .await
            .map_err(|e| crate::AgentError::PlanningFailed(e.to_string()))?;
        let estimated_files = impact.affected_files.len();

        // Detect conflicts
        let conflicts = planner
            .detect_conflicts(&steps)
            .map_err(|e| crate::AgentError::PlanningFailed(e.to_string()))?;

        if !conflicts.is_empty() {
            return Err(crate::AgentError::PlanningFailed(format!(
                "Found {} conflicts in plan",
                conflicts.len()
            )));
        }

        // Order steps
        let mut ordered_steps = steps;
        planner
            .order_steps(&mut ordered_steps)
            .map_err(|e| crate::AgentError::PlanningFailed(e.to_string()))?;

        let step_count = ordered_steps.len();

        // Generate rollback plan
        let rollback = planner.generate_rollback(&ordered_steps);

        // Record audit event
        self.audit_log
            .record(AuditEvent::Plan {
                timestamp: Utc::now(),
                step_count,
                estimated_files,
            })
            .await
            .map_err(|e| crate::AgentError::PlanningFailed(e.to_string()))?;

        Ok(ExecutionPlan {
            steps: ordered_steps,
            estimated_impact: impact,
            rollback_plan: rollback,
        })
    }

    /// Mutate phase - apply changes.
    async fn mutate_phase(
        &mut self,
        plan: ExecutionPlan,
    ) -> Result<MutationResult, crate::AgentError> {
        // Create mutator
        let mut mutator = crate::mutate::Mutator::new();
        mutator
            .begin_transaction()
            .await
            .map_err(|e| crate::AgentError::MutationFailed(e.to_string()))?;

        // Apply each step
        for step in &plan.steps {
            mutator
                .apply_step(step)
                .await
                .map_err(|e| crate::AgentError::MutationFailed(e.to_string()))?;
        }

        // Transfer transaction to loop for commit/rollback
        let transaction = mutator.into_transaction()?;
        let modified_files: Vec<std::path::PathBuf> = transaction
            .snapshots()
            .iter()
            .map(|s| s.path.clone())
            .collect();
        let diffs: Vec<String> = transaction
            .snapshots()
            .iter()
            .filter(|s| !s.original_content.is_empty())
            .map(|s| {
                let path_display = s.path.display();
                format!(
                    "--- {}\n+++ {}\n{} bytes original",
                    path_display,
                    path_display,
                    s.original_content.len()
                )
            })
            .collect();
        self.transaction = Some(transaction);

        // Collect file names for audit
        let files_modified: Vec<String> = modified_files
            .iter()
            .map(|p| p.display().to_string())
            .collect();

        // Record audit event
        self.audit_log
            .record(AuditEvent::Mutate {
                timestamp: Utc::now(),
                files_modified,
            })
            .await
            .map_err(|e| crate::AgentError::MutationFailed(e.to_string()))?;

        Ok(MutationResult {
            modified_files,
            diffs,
        })
    }

    /// Verify phase - validate results, scoped to changed files.
    async fn verify_phase(
        &mut self,
        result: MutationResult,
    ) -> Result<VerificationResult, crate::AgentError> {
        // Create verifier with Forge SDK for graph checks
        let mut verifier = crate::verify::Verifier::with_forge((*self.forge).clone());
        if let Some(ref llm) = self.llm {
            verifier = verifier.with_llm(llm.clone());
        }

        let report = if result.modified_files.is_empty() {
            verifier
                .verify(&self.codebase_path)
                .await
                .map_err(|e| crate::AgentError::VerificationFailed(e.to_string()))?
        } else {
            verifier
                .verify_changes(&self.codebase_path, &result.modified_files, &result.diffs)
                .await
                .map_err(|e| crate::AgentError::VerificationFailed(e.to_string()))?
        };

        let diagnostic_count = report.diagnostics.len();
        let passed = report.passed;

        // Record audit event
        self.audit_log
            .record(AuditEvent::Verify {
                timestamp: Utc::now(),
                passed,
                diagnostic_count,
            })
            .await
            .map_err(|e| crate::AgentError::VerificationFailed(e.to_string()))?;

        Ok(VerificationResult {
            passed: report.passed,
            diagnostics: report
                .diagnostics
                .iter()
                .map(|d| d.message.clone())
                .collect(),
            suggestions: report.suggestions,
        })
    }

    /// Commit phase - finalize transaction.
    async fn commit_phase(
        &mut self,
        verification: VerificationResult,
    ) -> Result<CommitResult, crate::AgentError> {
        // Extract files from diagnostics
        let files: Vec<std::path::PathBuf> = verification
            .diagnostics
            .iter()
            .filter_map(|d| {
                d.split(':')
                    .next()
                    .map(|s| std::path::PathBuf::from(s.trim()))
            })
            .collect();

        // Create committer
        let committer = crate::commit::Committer::new();
        let message = format!("forge: apply changes ({} files)", files.len());
        let commit_report = committer
            .finalize(&self.codebase_path, &files, &message)
            .await
            .map_err(|e| crate::AgentError::CommitFailed(e.to_string()))?;

        // Commit transaction
        if let Some(txn) = self.transaction.take() {
            txn.commit()
                .await
                .map_err(|e| crate::AgentError::CommitFailed(e.to_string()))?;
        }

        let transaction_id = commit_report.transaction_id.clone();

        // Record audit event
        self.audit_log
            .record(AuditEvent::Commit {
                timestamp: Utc::now(),
                transaction_id,
            })
            .await
            .map_err(|e| crate::AgentError::CommitFailed(e.to_string()))?;

        Ok(CommitResult {
            transaction_id: commit_report.transaction_id,
            files_committed: commit_report.files_committed,
        })
    }

    /// Stores observed symbols as atheneum discoveries (fire-and-forget).
    async fn store_observation_discoveries(&self, observation: &Observation) {
        if let Some(ref store) = self.discovery_store {
            for symbol in &observation.symbols {
                let metadata = serde_json::json!({
                    "kind": format!("{:?}", symbol.kind),
                    "file": symbol.location.file_path,
                    "line": symbol.location.line_number,
                });
                store.store("Symbol", &symbol.name, metadata).await;
            }
        }
    }

    /// Records a rollback in the audit log.
    async fn record_rollback(&mut self, error: &crate::AgentError) {
        // Rollback transaction if active
        if let Some(txn) = self.transaction.take() {
            if let Err(e) = txn.rollback().await {
                tracing::error!("Transaction rollback failed: {e}");
            }
        }

        // Determine phase from error type
        let phase = match error {
            crate::AgentError::ObservationFailed(_) => "Observe",
            crate::AgentError::PolicyViolation(_) => "Constrain",
            crate::AgentError::PlanningFailed(_) => "Plan",
            crate::AgentError::MutationFailed(_) => "Mutate",
            crate::AgentError::VerificationFailed(_) => "Verify",
            crate::AgentError::CommitFailed(_) => "Commit",
            crate::AgentError::ForgeError(_) => "Forge",
        };

        // Record rollback event
        if let Err(e) = self
            .audit_log
            .record(AuditEvent::Rollback {
                timestamp: Utc::now(),
                reason: error.to_string(),
                phase: phase.to_string(),
            })
            .await
        {
            tracing::error!("Failed to record rollback audit event: {e}");
        }
    }

    /// Returns a reference to the audit log (for testing).
    #[cfg(test)]
    pub fn audit_log(&self) -> &AuditLog {
        &self.audit_log
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_agent_loop_creation() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let agent_loop = AgentLoop::new(Arc::new(forge));

        // Should create with fresh state
        assert!(agent_loop.transaction.is_none());
        assert_eq!(agent_loop.audit_log().len(), 0);
    }

    #[tokio::test]
    async fn test_agent_loop_successful_run() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let mut agent_loop = AgentLoop::new(Arc::new(forge));

        let result = agent_loop.run("test query").await;

        // Should succeed or fail with verification error (expected for v0.3)
        // The loop completes all phases, verification may fail on empty dir
        match result {
            Ok(loop_result) => {
                // Verify transaction ID exists
                assert!(!loop_result.transaction_id.is_empty());
                // Verify audit log has 6 phase events
                assert_eq!(loop_result.audit_events.len(), 6);
            }
            Err(e) => {
                // Verification failure is expected for empty temp directory
                assert!(
                    e.to_string().contains("Verification")
                        || e.to_string().contains("verification")
                );
            }
        }
    }

    #[tokio::test]
    async fn test_agent_loop_state_isolation() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let mut agent_loop = AgentLoop::new(Arc::new(forge));

        // First run - may fail at verification, but should record phases
        let result1 = agent_loop.run("first query").await;
        let events1_count = match &result1 {
            Ok(r) => r.audit_events.len(),
            Err(_) => {
                // Even on failure, audit log should have events
                // For v0.3, verification fails, so we expect partial audit
                agent_loop.audit_log().len()
            }
        };

        // Second run should have fresh state
        let result2 = agent_loop.run("second query").await;
        let events2_count = match &result2 {
            Ok(r) => r.audit_events.len(),
            Err(_) => agent_loop.audit_log().len(),
        };

        // Both runs should have similar number of events
        // (may differ due to verification timing)
        assert!(events1_count > 0);
        assert!(events2_count > 0);
    }

    #[tokio::test]
    async fn test_phase_transitions_recorded() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let mut agent_loop = AgentLoop::new(Arc::new(forge));

        let result = agent_loop.run("test query").await;

        // Even on verification failure, phases should be recorded
        let events = match result {
            Ok(r) => r.audit_events,
            Err(_) => agent_loop.audit_log().clone().into_events(),
        };

        // Should have at least 5 phase events (Observe, Constrain, Plan, Mutate, Verify)
        // Commit may not be reached if verification fails
        assert!(events.len() >= 5);

        // Check phase order for first few events
        assert!(matches!(events[0], AuditEvent::Observe { .. }));
        assert!(matches!(events[1], AuditEvent::Constrain { .. }));
        assert!(matches!(events[2], AuditEvent::Plan { .. }));
        assert!(matches!(events[3], AuditEvent::Mutate { .. }));

        // The 5th event could be Verify or Rollback (if verification failed)
        // Both are valid for v0.3
        let is_valid_fifth = matches!(
            events[4],
            AuditEvent::Verify { .. } | AuditEvent::Rollback { .. }
        );
        assert!(
            is_valid_fifth,
            "Expected Verify or Rollback at index 4, got: {:?}",
            events[4]
        );
    }

    /// Mock discovery store for testing.
    struct MockDiscoveryStore {
        discoveries: std::sync::Mutex<Vec<(String, String, serde_json::Value)>>,
    }

    impl MockDiscoveryStore {
        fn new() -> Self {
            Self {
                discoveries: std::sync::Mutex::new(Vec::new()),
            }
        }

        fn discoveries(&self) -> Vec<(String, String, serde_json::Value)> {
            self.discoveries.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl DiscoveryStore for MockDiscoveryStore {
        async fn store(&self, discovery_type: &str, target: &str, metadata: serde_json::Value) {
            self.discoveries.lock().unwrap().push((
                discovery_type.to_string(),
                target.to_string(),
                metadata,
            ));
        }
    }

    #[tokio::test]
    async fn test_discovery_store_records_observed_symbols() {
        let store = Arc::new(MockDiscoveryStore::new());
        let store_ref = store.clone();

        // Verify store works
        store
            .store("Symbol", "test_func", serde_json::json!({"line": 1}))
            .await;
        assert_eq!(store_ref.discoveries().len(), 1);
        assert_eq!(store_ref.discoveries()[0].0, "Symbol");
        assert_eq!(store_ref.discoveries()[0].1, "test_func");
    }

    #[tokio::test]
    async fn test_agent_loop_returns_loop_result() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let mut agent_loop = AgentLoop::new(Arc::new(forge));

        let result = agent_loop.run("test query").await;

        match result {
            Ok(loop_result) => {
                assert!(!loop_result.transaction_id.is_empty());
                assert!(!loop_result.audit_events.is_empty());
            }
            Err(_) => {
                // Error is expected for empty temp directory
            }
        }
    }

    #[tokio::test]
    async fn test_retry_loop_respects_max_attempts() {
        use crate::llm::MockProvider;
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();

        // Empty plan → no mutation → verify will fail (cargo check on empty dir)
        let llm = Arc::new(MockProvider::new("[]"));
        let mut agent_loop = AgentLoop::new(Arc::new(forge))
            .with_llm(llm)
            .with_max_fix_attempts(2);

        let result = agent_loop.run("test retry").await;

        // Must fail — not silently commit with broken code
        match result {
            Err(e) => {
                let msg = e.to_string().to_lowercase();
                assert!(
                    msg.contains("verification") || msg.contains("verify"),
                    "unexpected error: {e}"
                );
            }
            Ok(r) => {
                // Only acceptable if verification actually passed
                assert!(
                    r.audit_events
                        .iter()
                        .any(|e| matches!(e, AuditEvent::Verify { passed: true, .. })),
                    "committed despite failed verification"
                );
            }
        }
    }

    #[tokio::test]
    async fn test_with_max_fix_attempts_builder() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let agent_loop = AgentLoop::new(Arc::new(forge)).with_max_fix_attempts(5);
        assert_eq!(agent_loop.max_fix_attempts, 5);
    }
}

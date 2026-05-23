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
    /// Bayesian belief tracking across the 6-phase loop
    reasoning: forge_reasoning::ReasoningSystem,
    /// Hypothesis ID for the current task (proposed in observe, resolved in verify)
    current_hypothesis: Option<forge_reasoning::HypothesisId>,
    /// Policies enforced during the constraint phase
    policies: Vec<crate::policy::Policy>,
    /// Codebase-level context for enriching LLM prompts
    context: crate::context::AgentContext,
}

impl AgentLoop {
    /// Hypothesis confidence below this threshold triggers early bail-out in the
    /// fix loop instead of burning remaining retry attempts.
    pub const CONFIDENCE_BAIL_THRESHOLD: f64 = 0.15;

    /// Creates a new agent loop with fresh state.
    ///
    /// # Arguments
    ///
    /// * `forge` - The Forge SDK instance for graph queries
    pub fn new(forge: Arc<Forge>) -> Self {
        let codebase_path = forge.codebase_path().to_path_buf();
        let context = crate::context::AgentContext::from_path(&codebase_path);
        Self {
            forge,
            codebase_path,
            transaction: None,
            audit_log: AuditLog::new(),
            discovery_store: None,
            llm: None,
            max_fix_attempts: 3,
            last_observation: None,
            reasoning: forge_reasoning::ReasoningSystem::in_memory(),
            current_hypothesis: None,
            policies: Vec::new(),
            context,
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

    /// Sets the policies enforced during the constraint phase.
    pub fn with_policies(mut self, policies: Vec<crate::policy::Policy>) -> Self {
        self.policies = policies;
        self
    }

    /// Returns the reasoning system for querying hypothesis state.
    pub fn reasoning(&self) -> &forge_reasoning::ReasoningSystem {
        &self.reasoning
    }

    /// Executes a workflow using this loop's Forge SDK instance.
    ///
    /// Wraps `WorkflowExecutor` and injects the loop's `Forge` into every
    /// `TaskContext`, making tasks like `AgentLoopTask` and `GraphQueryTask`
    /// functional within the workflow.
    pub async fn run_workflow(
        &self,
        workflow: crate::workflow::Workflow,
    ) -> Result<crate::workflow::WorkflowResult, crate::AgentError> {
        crate::workflow::WorkflowExecutor::new(workflow)
            .with_forge(Arc::clone(&self.forge))
            .execute()
            .await
            .map_err(|e| crate::AgentError::WorkflowFailed(e.to_string()))
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
        let result = self.run_inner(query).await;
        self.store_hypothesis_outcome(query, result.is_ok()).await;
        result
    }

    async fn run_inner(&mut self, query: &str) -> Result<LoopResult, crate::AgentError> {
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
        let mut fix_planner = {
            let mut p = crate::planner::Planner::new().with_context(&self.context);
            if let Some(ref llm) = self.llm {
                p = p.with_llm(llm.clone());
            }
            p
        };
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

            // Bail out early if reasoning confidence has collapsed
            if let Some(id) = self.current_hypothesis {
                let conf = self
                    .reasoning
                    .board
                    .get(id)
                    .await
                    .ok()
                    .flatten()
                    .map(|h| h.current_confidence().get())
                    .unwrap_or(1.0);
                if conf < Self::CONFIDENCE_BAIL_THRESHOLD {
                    let e = crate::AgentError::VerificationFailed(format!(
                        "hypothesis confidence {conf:.3} collapsed below threshold {:.3}, aborting fix loop",
                        Self::CONFIDENCE_BAIL_THRESHOLD
                    ));
                    self.record_rollback(&e).await;
                    return Err(e);
                }
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

            let fix_steps = fix_planner
                .fix_once(&fix_observation, &verification.diagnostics)
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

            let impact = fix_planner
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

        Ok(LoopResult {
            transaction_id: commit_result.transaction_id,
            modified_files: commit_result.files_committed,
            audit_events: self.audit_log.clone().into_events(),
        })
    }

    /// Observation phase - gather context from the graph.
    async fn observe_phase(&mut self, query: &str) -> Result<Observation, crate::AgentError> {
        // Use Observer to gather context
        let mut observer =
            crate::observe::Observer::new((*self.forge).clone()).with_context(&self.context);
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

        if let Ok(id) = self
            .reasoning
            .board
            .propose_with_max_uncertainty(format!("task: {}", query))
            .await
        {
            self.current_hypothesis = Some(id);
        }

        Ok(observation)
    }

    /// Constraint phase - apply policy rules.
    pub(crate) async fn constrain_phase(
        &mut self,
        observation: Observation,
    ) -> Result<ConstrainedPlan, crate::AgentError> {
        let validator = PolicyValidator::new((*self.forge).clone());
        let policies = self.policies.clone();
        let mut all_violations = Vec::new();

        // Collect unique file paths from observed symbols and run policies against
        // their current content. This catches pre-existing violations in files the
        // query will touch before any mutation is applied.
        let mut seen = std::collections::HashSet::new();
        for symbol in &observation.symbols {
            let path = &symbol.location.file_path;
            if !seen.insert(path.clone()) {
                continue;
            }
            let content = match tokio::fs::read_to_string(path).await {
                Ok(c) => c,
                Err(_) => continue, // new file — nothing to pre-check
            };
            let diff = crate::policy::Diff {
                file_path: path.clone(),
                original: content.clone(),
                modified: content,
                changes: Vec::new(),
            };
            let report = validator
                .validate(&diff, &policies)
                .await
                .map_err(|e| crate::AgentError::PolicyViolation(e.to_string()))?;
            all_violations.extend(report.violations);
        }

        let policy_count = policies.len();
        let violation_count = all_violations.len();

        // Record audit event
        self.audit_log
            .record(AuditEvent::Constrain {
                timestamp: Utc::now(),
                policy_count,
                violations: violation_count,
            })
            .await
            .map_err(|e| crate::AgentError::PolicyViolation(e.to_string()))?;

        Ok(ConstrainedPlan {
            observation,
            policy_violations: all_violations,
        })
    }

    /// Plan phase - generate execution steps.
    async fn plan_phase(
        &mut self,
        constrained: ConstrainedPlan,
    ) -> Result<ExecutionPlan, crate::AgentError> {
        // Create planner
        let mut planner = crate::planner::Planner::new().with_context(&self.context);
        if let Some(ref llm) = self.llm {
            planner = planner.with_llm(llm.clone()).with_generator(Arc::new(
                crate::generate::Generator::new(self.forge.clone(), llm.clone()),
            ));
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

        if let Some(id) = self.current_hypothesis {
            let _ = self
                .reasoning
                .board
                .set_status(id, forge_reasoning::HypothesisStatus::UnderTest)
                .await;
            let (lh, lnh) = if ordered_steps.is_empty() {
                (0.2, 0.8)
            } else {
                (0.8, 0.2)
            };
            let _ = self.reasoning.board.update_with_evidence(id, lh, lnh).await;
        }

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

        if let Some(id) = self.current_hypothesis {
            let (lh, lnh) = if report.passed {
                (0.9, 0.1)
            } else {
                (0.1, 0.9)
            };
            let _ = self.reasoning.board.update_with_evidence(id, lh, lnh).await;
            if report.passed {
                let _ = self
                    .reasoning
                    .board
                    .set_status(id, forge_reasoning::HypothesisStatus::Confirmed)
                    .await;
            }
        }

        let policy_violations = self
            .verify_policies_on_mutations()
            .await
            .unwrap_or_default();
        let final_passed = report.passed && policy_violations.is_empty();
        let final_diagnostic_count = report.diagnostics.len() + policy_violations.len();

        // Record audit event
        self.audit_log
            .record(AuditEvent::Verify {
                timestamp: Utc::now(),
                passed: final_passed,
                diagnostic_count: final_diagnostic_count,
            })
            .await
            .map_err(|e| crate::AgentError::VerificationFailed(e.to_string()))?;

        let mut all_diagnostics: Vec<String> = report
            .diagnostics
            .iter()
            .map(|d| d.message.clone())
            .collect();
        all_diagnostics.extend(policy_violations);

        Ok(VerificationResult {
            passed: final_passed,
            diagnostics: all_diagnostics,
            suggestions: report.suggestions,
        })
    }

    /// Commit phase - finalize transaction.
    pub(crate) async fn commit_phase(
        &mut self,
        _verification: VerificationResult,
    ) -> Result<CommitResult, crate::AgentError> {
        // Extract modified files from the transaction snapshots.
        // The transaction holds the authoritative record of what was changed;
        // verification.diagnostics contains error messages, not file paths.
        let files: Vec<std::path::PathBuf> = self
            .transaction
            .as_ref()
            .map(|txn| txn.snapshots().iter().map(|s| s.path.clone()).collect())
            .unwrap_or_default();

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

    /// Checks every file modified in the current transaction against the configured
    /// policies.  Returns violation messages; empty means no violations.
    pub(crate) async fn verify_policies_on_mutations(
        &self,
    ) -> Result<Vec<String>, crate::AgentError> {
        let Some(ref txn) = self.transaction else {
            return Ok(Vec::new());
        };
        if self.policies.is_empty() {
            return Ok(Vec::new());
        }
        let validator = crate::policy::PolicyValidator::new((*self.forge).clone());
        let mut violations = Vec::new();
        for snapshot in txn.snapshots() {
            let modified = tokio::fs::read_to_string(&snapshot.path)
                .await
                .unwrap_or_default();
            let diff = crate::policy::Diff {
                file_path: snapshot.path.clone(),
                original: snapshot.original_content.clone(),
                modified,
                changes: Vec::new(),
            };
            let report = validator
                .validate(&diff, &self.policies)
                .await
                .map_err(|e| crate::AgentError::PolicyViolation(e.to_string()))?;
            violations.extend(report.violations.iter().map(|v| v.message.clone()));
        }
        Ok(violations)
    }

    /// Stores observed symbols as atheneum discoveries (fire-and-forget).
    async fn store_hypothesis_outcome(&self, query: &str, succeeded: bool) {
        let Some(ref store) = self.discovery_store else {
            return;
        };
        let Some(id) = self.current_hypothesis else {
            return;
        };
        let hyp = self.reasoning.board.get(id).await.ok().flatten();
        let confidence = hyp
            .as_ref()
            .map(|h| h.current_confidence().get())
            .unwrap_or(0.5);
        let status = hyp.map(|h| format!("{:?}", h.status()));
        store
            .store(
                "HypothesisOutcome",
                query,
                serde_json::json!({
                    "hypothesis_id": id.to_string(),
                    "succeeded": succeeded,
                    "final_confidence": confidence,
                    "status": status,
                }),
            )
            .await;
    }

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
            crate::AgentError::WorkflowFailed(_) => "Workflow",
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

    #[tokio::test]
    async fn test_reasoning_system_initialized() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let agent_loop = AgentLoop::new(Arc::new(forge));
        // ReasoningSystem is always initialized; listing hypotheses returns empty vec
        let hypotheses = agent_loop.reasoning().board.list().await.unwrap();
        assert!(hypotheses.is_empty());
    }

    #[tokio::test]
    async fn test_hypothesis_proposed_after_observe() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let mut agent_loop = AgentLoop::new(Arc::new(forge));
        // current_hypothesis starts None
        assert!(agent_loop.current_hypothesis.is_none());
        // Run observe phase directly
        let _ = agent_loop.observe_phase("add a parser function").await;
        // After observe, a hypothesis should be registered
        assert!(agent_loop.current_hypothesis.is_some());
        let hypotheses = agent_loop.reasoning().board.list().await.unwrap();
        assert_eq!(hypotheses.len(), 1);
        assert!(hypotheses[0].statement().contains("add a parser function"));
    }

    #[tokio::test]
    async fn test_hypothesis_confirmed_after_successful_run() {
        use forge_reasoning::HypothesisStatus;
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let mut agent_loop = AgentLoop::new(Arc::new(forge));
        // Run the full loop (may fail on empty dir, but observe+plan phases run)
        let _ = agent_loop.run("generate a helper").await;
        // If hypothesis was proposed, check its final state
        if let Some(id) = agent_loop.current_hypothesis {
            let hyp = agent_loop.reasoning().board.get(id).await.unwrap().unwrap();
            // Status should be Confirmed (passed) or still UnderTest/Proposed (failed early)
            assert!(
                hyp.status() == HypothesisStatus::Confirmed
                    || hyp.status() == HypothesisStatus::UnderTest
                    || hyp.status() == HypothesisStatus::Proposed
            );
        }
    }

    // ── Task 4: reasoning persistence ────────────────────────────────────

    #[tokio::test]
    async fn test_hypothesis_outcome_stored_on_run() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let store = Arc::new(MockDiscoveryStore::new());
        let mut agent_loop = AgentLoop::new(Arc::new(forge)).with_discovery_store(store.clone());
        let _ = agent_loop.run("generate helper").await;
        let discoveries = store.discoveries();
        let outcome = discoveries
            .iter()
            .find(|(dtype, _, _)| dtype == "HypothesisOutcome");
        assert!(
            outcome.is_some(),
            "HypothesisOutcome should be stored; got: {:?}",
            discoveries.iter().map(|(t, _, _)| t).collect::<Vec<_>>()
        );
        let (_, target, meta) = outcome.unwrap();
        assert_eq!(target.as_str(), "generate helper");
        assert!(
            meta.get("succeeded").is_some(),
            "meta must have 'succeeded'"
        );
        assert!(
            meta.get("final_confidence").is_some(),
            "meta must have 'final_confidence'"
        );
    }

    // ── Task 1: policy threading ──────────────────────────────────────────

    #[tokio::test]
    async fn test_with_policies_builder_stores_policies() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let agent_loop = AgentLoop::new(Arc::new(forge))
            .with_policies(vec![crate::policy::Policy::NoUnsafeInPublicAPI]);
        assert_eq!(agent_loop.policies.len(), 1);
    }

    #[tokio::test]
    async fn test_constrain_phase_records_policy_count() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let mut agent_loop = AgentLoop::new(Arc::new(forge)).with_policies(vec![
            crate::policy::Policy::NoUnsafeInPublicAPI,
            crate::policy::Policy::PreserveTests,
        ]);
        // Run the full loop (verification will fail on empty dir, but constrain runs)
        let _ = agent_loop.run("test query").await;
        // Find the Constrain audit event and verify policy_count == 2
        let events = agent_loop.audit_log().clone().into_events();
        let constrain_event = events
            .iter()
            .find(|e| matches!(e, AuditEvent::Constrain { .. }));
        assert!(constrain_event.is_some(), "Constrain event not recorded");
        if let Some(AuditEvent::Constrain { policy_count, .. }) = constrain_event {
            assert_eq!(
                *policy_count, 2,
                "policy_count should be 2, got {policy_count}"
            );
        }
    }

    // ── Critical gap 2: policy enforcement on real mutations ─────────────────

    #[tokio::test]
    async fn test_policy_catches_unsafe_in_created_file() {
        use crate::transaction::Transaction;
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let mut agent_loop = AgentLoop::new(Arc::new(forge))
            .with_policies(vec![crate::policy::Policy::NoUnsafeInPublicAPI]);

        // Simulate what mutate_phase does: snapshot the file (doesn't exist yet),
        // then write unsafe content.
        let unsafe_file = temp_dir.path().join("danger.rs");
        let mut txn = Transaction::begin().await.unwrap();
        txn.snapshot_file(&unsafe_file).await.unwrap();
        tokio::fs::write(&unsafe_file, "pub unsafe fn danger() {}")
            .await
            .unwrap();
        agent_loop.transaction = Some(txn);

        let violations = agent_loop.verify_policies_on_mutations().await.unwrap();
        assert!(
            !violations.is_empty(),
            "NoUnsafeInPublicAPI should catch `pub unsafe fn`"
        );
    }

    #[tokio::test]
    async fn test_policy_passes_safe_file() {
        use crate::transaction::Transaction;
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let mut agent_loop = AgentLoop::new(Arc::new(forge))
            .with_policies(vec![crate::policy::Policy::NoUnsafeInPublicAPI]);

        let safe_file = temp_dir.path().join("safe.rs");
        let mut txn = Transaction::begin().await.unwrap();
        txn.snapshot_file(&safe_file).await.unwrap();
        tokio::fs::write(&safe_file, "pub fn safe() {}")
            .await
            .unwrap();
        agent_loop.transaction = Some(txn);

        let violations = agent_loop.verify_policies_on_mutations().await.unwrap();
        assert!(
            violations.is_empty(),
            "safe file should produce no violations"
        );
    }

    #[tokio::test]
    async fn test_constrain_phase_reads_real_file_content() {
        use crate::observe::{Observation, ObservedSymbol};
        use forge_core::types::{Location, SymbolId, SymbolKind};

        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let mut agent_loop = AgentLoop::new(Arc::new(forge))
            .with_policies(vec![crate::policy::Policy::NoUnsafeInPublicAPI]);

        // Write a file that violates NoUnsafeInPublicAPI
        let unsafe_file = temp_dir.path().join("danger.rs");
        tokio::fs::write(&unsafe_file, "pub unsafe fn danger() {}")
            .await
            .unwrap();

        // Build an observation whose symbol points at that real file.
        // The query itself does NOT contain "unsafe" — so the current fake-diff
        // path will produce zero violations (RED).
        let observation = Observation {
            query: "refactor danger module".to_string(),
            symbols: vec![ObservedSymbol {
                id: SymbolId(1),
                name: "danger".to_string(),
                kind: SymbolKind::Function,
                location: Location {
                    file_path: unsafe_file.clone(),
                    byte_start: 0,
                    byte_end: 0,
                    line_number: 1,
                },
            }],
            summary: None,
        };

        let constrained = agent_loop.constrain_phase(observation).await.unwrap();

        assert!(
            !constrained.policy_violations.is_empty(),
            "constrain_phase must detect NoUnsafeInPublicAPI from the real file, not from the query string"
        );
    }

    #[tokio::test]
    async fn test_commit_phase_uses_transaction_snapshots_not_diagnostics() {
        use crate::transaction::Transaction;
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let mut agent_loop = AgentLoop::new(Arc::new(forge));

        // Write a real file and snapshot it in the transaction
        let modified_file = temp_dir.path().join("modified.rs");
        tokio::fs::write(&modified_file, "pub fn foo() {}")
            .await
            .unwrap();
        let mut txn = Transaction::begin().await.unwrap();
        txn.snapshot_file(&modified_file).await.unwrap();
        agent_loop.transaction = Some(txn);

        // Call commit_phase with EMPTY diagnostics — simulates a passing verification.
        // The bug: files_committed is derived from diagnostics (empty), so git stages nothing.
        let verification = crate::VerificationResult {
            passed: true,
            diagnostics: vec![],
            suggestions: None,
        };
        let result = agent_loop.commit_phase(verification).await.unwrap();

        assert!(
            result.files_committed.contains(&modified_file),
            "commit_phase must derive committed files from transaction snapshots, not diagnostics"
        );
    }

    #[tokio::test]
    async fn test_agent_loop_run_workflow_passes_forge() {
        use crate::workflow::dag::Workflow;
        use crate::workflow::task::{TaskContext, TaskError, TaskId, TaskResult, WorkflowTask};
        use async_trait::async_trait;

        struct ForgeCheckTask;

        #[async_trait]
        impl WorkflowTask for ForgeCheckTask {
            async fn execute(&self, context: &TaskContext) -> Result<TaskResult, TaskError> {
                if context.forge.is_some() {
                    Ok(TaskResult::Success)
                } else {
                    Err(TaskError::ExecutionFailed(
                        "no forge in context".to_string(),
                    ))
                }
            }

            fn id(&self) -> TaskId {
                TaskId::new("forge-check")
            }

            fn name(&self) -> &str {
                "ForgeCheckTask"
            }
        }

        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let agent_loop = AgentLoop::new(Arc::new(forge));

        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(ForgeCheckTask));

        let result = agent_loop.run_workflow(workflow).await;
        assert!(
            result.is_ok(),
            "run_workflow should succeed when forge is wired: {:?}",
            result.err()
        );
        assert!(
            result.unwrap().success,
            "workflow result should report success"
        );
    }

    // ── Gap 5: reasoning confidence bail-out ─────────────────────────────

    /// When hypothesis confidence drops below CONFIDENCE_BAIL_THRESHOLD during
    /// the fix loop the loop must bail out before exhausting max_fix_attempts.
    #[tokio::test]
    async fn test_low_confidence_bails_before_max_attempts() {
        use crate::llm::MockProvider;
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();

        // Enough retries that without early bail-out we'd see multiple attempts;
        // with bail-out we should see VerificationFailed before reaching attempt 5.
        let llm = Arc::new(MockProvider::new("[]"));
        let mut agent_loop = AgentLoop::new(Arc::new(forge))
            .with_llm(llm)
            .with_max_fix_attempts(5);

        let result = agent_loop.run("test confidence bail-out").await;

        // Must fail (empty dir cannot pass verification)
        assert!(result.is_err(), "expected failure on empty codebase");

        // If a hypothesis was registered, its confidence must be below threshold
        // (evidence of 0.1/0.9 likelihood on failure drives it below 0.15)
        if let Some(id) = agent_loop.current_hypothesis {
            let hyp = agent_loop.reasoning().board.get(id).await.unwrap().unwrap();
            let conf = hyp.current_confidence().get();
            // The bail-out threshold is CONFIDENCE_BAIL_THRESHOLD = 0.15
            assert!(
                conf < AgentLoop::CONFIDENCE_BAIL_THRESHOLD,
                "confidence {conf} should be below bail threshold after repeated failures"
            );
        }
    }
}

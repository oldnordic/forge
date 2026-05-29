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

mod phases;
#[cfg(test)]
mod tests;
mod types;

pub use types::{AgentLoopCheckpoint, AgentPhase, DiscoveryStore, LoopResult};

pub struct AgentLoop {
    forge: Arc<Forge>,
    codebase_path: std::path::PathBuf,
    transaction: Option<crate::transaction::Transaction>,
    audit_log: AuditLog,
    discovery_store: Option<Arc<dyn DiscoveryStore>>,
    llm: Option<Arc<dyn crate::llm::LlmProvider>>,
    max_fix_attempts: u32,
    last_observation: Option<crate::observe::Observation>,
    reasoning: forge_reasoning::ReasoningSystem,
    current_hypothesis: Option<forge_reasoning::HypothesisId>,
    policies: Vec<crate::policy::Policy>,
    context: crate::context::AgentContext,
    gap_hints: Vec<String>,
    checkpoints: Vec<AgentLoopCheckpoint>,
    #[cfg(feature = "envoy")]
    session: Option<std::sync::Arc<crate::evidence::ForgeSession>>,
}

impl AgentLoop {
    pub const CONFIDENCE_BAIL_THRESHOLD: f64 = 0.15;

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
            gap_hints: Vec::new(),
            checkpoints: Vec::new(),
            #[cfg(feature = "envoy")]
            session: None,
        }
    }

    pub fn with_discovery_store(mut self, store: Arc<dyn DiscoveryStore>) -> Self {
        self.discovery_store = Some(store);
        self
    }

    pub fn with_llm(mut self, provider: Arc<dyn crate::llm::LlmProvider>) -> Self {
        self.llm = Some(provider);
        self
    }

    pub fn with_max_fix_attempts(mut self, n: u32) -> Self {
        self.max_fix_attempts = n;
        self
    }

    pub fn with_policies(mut self, policies: Vec<crate::policy::Policy>) -> Self {
        self.policies = policies;
        self
    }

    #[cfg(feature = "envoy")]
    pub fn with_session(mut self, session: std::sync::Arc<crate::evidence::ForgeSession>) -> Self {
        self.session = Some(session);
        self
    }

    pub fn reasoning(&self) -> &forge_reasoning::ReasoningSystem {
        &self.reasoning
    }

    pub fn gap_hints(&self) -> &[String] {
        &self.gap_hints
    }

    pub fn checkpoints(&self) -> &[AgentLoopCheckpoint] {
        &self.checkpoints
    }

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

    pub async fn run(&mut self, query: &str) -> Result<LoopResult, crate::AgentError> {
        let result = self.run_inner(query).await;
        self.store_hypothesis_outcome(query, result.is_ok()).await;
        result
    }

    async fn run_inner(&mut self, query: &str) -> Result<LoopResult, crate::AgentError> {
        let observation = match self.observe_phase(query).await {
            Ok(obs) => obs,
            Err(e) => {
                self.record_rollback(&e).await;
                return Err(e);
            }
        };

        self.store_observation_discoveries(&observation).await;
        self.save_phase_checkpoint("observe");

        #[cfg(feature = "envoy")]
        if let Some(ref session) = self.session {
            for sym in &observation.symbols {
                session.record_tool_call(crate::evidence::ToolCallEvidence {
                    tool_name: "magellan_find".into(),
                    input_hash: crate::evidence::sha256_hex(sym.name.as_bytes()),
                    input_summary: format!("symbol: {}", sym.name),
                    exit_status: "success".into(),
                    tool_category: crate::evidence::ToolCategory::GroundedQuery,
                    ..Default::default()
                });
            }
        }

        let constrained = match self.constrain_phase(observation).await {
            Ok(constrained) => constrained,
            Err(e) => {
                self.record_rollback(&e).await;
                return Err(e);
            }
        };
        self.save_phase_checkpoint("constrain");

        let mut plan = match self.plan_phase(constrained.clone()).await {
            Ok(plan) => plan,
            Err(e) => {
                self.record_rollback(&e).await;
                return Err(e);
            }
        };
        self.save_phase_checkpoint("plan");

        #[cfg(feature = "envoy")]
        if let Some(ref session) = self.session {
            session.record_prompt(crate::evidence::PromptRecord {
                role: "planner".into(),
                input_hash: crate::evidence::sha256_hex(query.as_bytes()),
                ..Default::default()
            });
        }

        let mut attempt = 0u32;
        let mut fix_planner = {
            let mut p = crate::planner::Planner::new().with_context(&self.context);
            if let Some(ref llm) = self.llm {
                p = p.with_llm(llm.clone());
            }
            p
        };
        let verification = loop {
            let mutation_result = match self.mutate_phase(plan).await {
                Ok(r) => r,
                Err(e) => {
                    self.record_rollback(&e).await;
                    return Err(e);
                }
            };
            self.save_phase_checkpoint("mutate");

            #[cfg(feature = "envoy")]
            if let Some(ref session) = self.session {
                if let Some(ref tx) = self.transaction {
                    for snap in tx.snapshots() {
                        let path_str = snap.path.to_string_lossy().into_owned();
                        session.record_file_write(crate::evidence::FileWriteRecord {
                            file_path: path_str.clone(),
                            file_id: crate::evidence::sha256_hex(path_str.as_bytes()),
                            after_hash: crate::evidence::sha256_hex(
                                format!("mutated:{}", path_str).as_bytes(),
                            ),
                            write_type: "edit".into(),
                            ..Default::default()
                        });
                    }
                }
            }

            let verification = match self.verify_phase(mutation_result).await {
                Ok(v) => v,
                Err(e) => {
                    self.record_rollback(&e).await;
                    return Err(e);
                }
            };
            self.save_phase_checkpoint("verify");

            if verification.passed || attempt >= self.max_fix_attempts {
                break verification;
            }

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

        if !verification.passed {
            let e = crate::AgentError::VerificationFailed(format!(
                "verification failed after {} fix attempt(s): {}",
                attempt,
                verification.diagnostics.join("; ")
            ));
            self.record_rollback(&e).await;
            return Err(e);
        }

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
}

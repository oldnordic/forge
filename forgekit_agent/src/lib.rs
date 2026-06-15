//! ForgeKit agent layer - Deterministic AI loop.
//!
//! This crate provides a deterministic agent loop for AI-driven code operations:
//!
//! - Observation: Gather context from the graph
//! - Constraint: Apply policy rules
//! - Planning: Generate execution steps
//! - Mutation: Apply changes
//! - Verification: Validate results
//! - Commit: Finalize transaction
//!
//! # Status
//!
//! All six phases (observe, constrain, plan, mutate, verify, commit) are
//! implemented and tested. The crate also provides a ReAct tool-calling loop,
//! a workflow/DAG engine with rollback, and multi-agent orchestration.

use std::path::PathBuf;

pub(crate) mod agent_loop;
pub(crate) mod audit;
pub(crate) mod commit;
pub(crate) mod llm;
pub(crate) mod mutate;
pub mod observe;
pub(crate) mod planner;
pub(crate) mod policy;
pub(crate) mod verify;
pub mod workflow;
#[cfg(feature = "llm-anthropic")]
pub use llm::AnthropicProvider;
#[cfg(feature = "llm-ollama")]
pub use llm::OllamaProvider;
#[cfg(feature = "llm-openai")]
pub use llm::OpenAiProvider;
pub use llm::{LlmConfig, LlmProvider};

pub mod chat;

pub mod prelude;

#[cfg(feature = "envoy")]
pub mod envoy;

#[cfg(feature = "envoy")]
pub mod evidence;

pub(crate) mod context;
pub(crate) mod generate;

pub use generate::{GeneratedCode, Generator};

/// Error types for agent operations.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum AgentError {
    /// Observation phase failed
    #[error("Observation failed: {0}")]
    ObservationFailed(String),

    /// Planning phase failed
    #[error("Planning failed: {0}")]
    PlanningFailed(String),

    /// Mutation phase failed
    #[error("Mutation failed: {0}")]
    MutationFailed(String),

    /// Verification phase failed
    #[error("Verification failed: {0}")]
    VerificationFailed(String),

    /// Commit phase failed
    #[error("Commit failed: {0}")]
    CommitFailed(String),

    /// Policy constraint violated
    #[error("Policy violation: {0}")]
    PolicyViolation(String),

    /// Error from Forge SDK
    #[error("Forge error: {0}")]
    ForgeError(#[from] forgekit_core::ForgeError),

    /// Workflow execution failed
    #[error("Workflow failed: {0}")]
    WorkflowFailed(String),

    /// ReAct agent loop failed
    #[error("ReAct agent failed: {0}")]
    ReActFailed(String),
}

impl AgentError {
    pub fn phase_label(&self) -> &'static str {
        match self { // nosemgrep: llm-giant-match — exhaustive enum→label dispatch, the idiomatic pattern
            AgentError::ObservationFailed(_) => "Observe",
            AgentError::PolicyViolation(_) => "Constrain",
            AgentError::PlanningFailed(_) => "Plan",
            AgentError::MutationFailed(_) => "Mutate",
            AgentError::VerificationFailed(_) => "Verify",
            AgentError::CommitFailed(_) => "Commit",
            AgentError::ForgeError(_) => "Forge",
            AgentError::WorkflowFailed(_) => "Workflow",
            AgentError::ReActFailed(_) => "ReAct",
        }
    }
}

/// Result type for agent operations.
pub type Result<T> = std::result::Result<T, AgentError>;

/// A handle to a spawned agent task. Await to get the result.
pub struct AgentTask {
    handle: tokio::task::JoinHandle<std::result::Result<String, String>>,
}

impl std::fmt::Debug for AgentTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentTask").finish_non_exhaustive()
    }
}

impl AgentTask {
    pub async fn await_result(self) -> std::result::Result<String, AgentError> {
        self.handle
            .await
            .map_err(|e| AgentError::ReActFailed(format!("spawned task failed: {e}")))?
            .map_err(AgentError::ReActFailed)
    }
}

impl std::future::IntoFuture for AgentTask {
    type Output = std::result::Result<String, AgentError>;
    type IntoFuture = std::pin::Pin<Box<dyn std::future::Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.await_result())
    }
}

// Re-export policy module
pub use policy::{Policy, PolicyReport, PolicyValidator, PolicyViolation};

// Re-export observation types
pub use observe::Observation;

// Re-export loop types
pub use agent_loop::{AgentLoop, AgentPhase, LoopResult};

// Re-export audit types
pub use audit::{AuditEvent, AuditLog};

// Re-export workflow types
pub use workflow::{
    Dependency, Gate, GateAction, GateLanguage, GateResult, GateRunner, TaskContext, TaskError,
    TaskId, TaskResult, ValidationReport, Workflow, WorkflowError, WorkflowExecutor,
    WorkflowResult, WorkflowTask, WorkflowValidator,
};

/// Result of applying policy constraints.
#[derive(Clone, Debug)]
pub struct ConstrainedPlan {
    /// The original observation
    pub observation: Observation,
    /// Any policy violations detected
    pub policy_violations: Vec<policy::PolicyViolation>,
}

/// Execution plan for the mutation phase.
#[derive(Clone, Debug)]
pub struct ExecutionPlan {
    /// Steps to execute
    pub steps: Vec<planner::PlanStep>,
    /// Estimated impact
    pub estimated_impact: planner::ImpactEstimate,
    /// Rollback plan
    pub rollback_plan: Vec<planner::RollbackStep>,
}

/// Result of the mutation phase.
#[derive(Clone, Debug)]
pub struct MutationResult {
    /// Files that were modified
    pub modified_files: Vec<PathBuf>,
    /// Diffs of changes made
    pub diffs: Vec<String>,
}

/// Result of the verification phase.
#[derive(Clone, Debug)]
pub struct VerificationResult {
    /// Whether verification passed
    pub passed: bool,
    /// Any diagnostics or errors
    pub diagnostics: Vec<String>,
    /// LLM-generated fix suggestions
    pub suggestions: Option<String>,
}

/// Result of the commit phase.
#[derive(Clone, Debug)]
pub struct CommitResult {
    /// Transaction ID for the commit
    pub transaction_id: String,
    /// Files that were committed
    pub files_committed: Vec<PathBuf>,
    /// Whether `git commit` actually ran (false if git was unavailable)
    pub git_committed: bool,
}

// Agent implementation (split from lib.rs for 1K LOC modularization)
mod agent;
pub use agent::Agent;

mod builder;
pub use builder::{agent_builder, AgentBuilder, NeedsProvider, Ready};

pub(crate) mod agent_config;
pub(crate) mod orchestrate;
pub use agent_config::AgentConfig;
pub use orchestrate::{OrchestrateResult, Orchestrator};

pub(crate) mod transaction;

pub use transaction::{FileSnapshot, Transaction, TransactionState};

pub(crate) mod runtime_integration;

// Re-export runtime types for convenience
pub use forgekit_runtime::{ForgeRuntime, RuntimeConfig, RuntimeStats};

#[cfg(test)]
mod orchestrate_tests;

#[cfg(test)]
mod tests;

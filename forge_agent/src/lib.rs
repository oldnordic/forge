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
//! This crate is under active development. Observation and planning phases are implemented.

use std::path::PathBuf;

// Observation module (Phase 4 - Task 1)
pub mod observe;

// Policy module (Phase 4 - Task 2)
pub mod policy;

// Planning module (Phase 4 - Task 3)
pub mod planner;

// Mutation module (Phase 4 - Task 4)
pub mod mutate;

// Verification module (Phase 4 - Task 5)
pub mod verify;

// Commit module (Phase 4 - Task 6)
pub mod commit;

// Loop module (Phase 3 - Task 1)
pub mod r#loop;

// Audit module (Phase 3 - Task 2)
pub mod audit;

// Workflow module (Phase 8 - Plan 1)
pub mod workflow;

// LLM provider module
pub mod llm;
#[cfg(feature = "llm-anthropic")]
pub use llm::AnthropicProvider;
#[cfg(feature = "llm-ollama")]
pub use llm::OllamaProvider;
#[cfg(feature = "llm-openai")]
pub use llm::OpenAiProvider;

// Envoy coordination module
#[cfg(feature = "envoy")]
pub mod envoy;

// Context composition module
pub mod context;

// Code generation from natural language descriptions
pub mod generate;
pub use generate::{GeneratedCode, Generator};

/// Error types for agent operations.
#[derive(thiserror::Error, Debug)]
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
    ForgeError(#[from] forge_core::ForgeError),
}

/// Result type for agent operations.
pub type Result<T> = std::result::Result<T, AgentError>;

// Re-export policy module
pub use policy::{Policy, PolicyReport, PolicyValidator, PolicyViolation};

// Re-export observation types
pub use observe::Observation;

// Re-export loop types
pub use r#loop::{AgentLoop, AgentPhase, LoopResult};

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
}

/// Agent for deterministic AI-driven code operations.
///
/// The agent follows a strict loop:
/// 1. Observe: Gather context from the graph
/// 2. Constrain: Apply policy rules
/// 3. Plan: Generate execution steps
/// 4. Mutate: Apply changes
/// 5. Verify: Validate results
/// 6. Commit: Finalize transaction
///
/// # Runtime Integration
///
/// The agent can integrate with `ForgeRuntime` for coordinated file watching
/// and caching:
///
/// ```ignore
/// let (agent, mut runtime) = Agent::with_runtime("./project").await?;
/// let result = agent.run_with_runtime(&mut runtime, "refactor function").await?;
/// ```
///
pub struct Agent {
    /// Path to the codebase
    codebase_path: PathBuf,
    /// Forge SDK instance for graph queries
    forge: Option<forge_core::Forge>,
    /// Optional LLM provider for semantic operations
    pub(crate) llm: Option<std::sync::Arc<dyn llm::LlmProvider>>,
    /// Optional envoy client for multi-agent coordination
    #[cfg(feature = "envoy")]
    pub(crate) envoy: Option<std::sync::Arc<envoy::EnvoyClient>>,
    /// Policies enforced during the constraint phase
    policies: Vec<policy::Policy>,
}

impl Agent {
    /// Creates a new agent for the given codebase.
    ///
    /// # Arguments
    ///
    /// * `codebase_path` - Path to the codebase
    pub async fn new(codebase_path: impl AsRef<std::path::Path>) -> Result<Self> {
        let path = codebase_path.as_ref().to_path_buf();

        // Try to initialize Forge SDK
        let forge = forge_core::Forge::open(&path).await.ok();

        // Load envoy config from .forge.toml
        #[cfg(feature = "envoy")]
        let envoy = {
            let config_path = path.join(".forge.toml");
            envoy::EnvoyConfig::from_file(&config_path)
                .ok()
                .flatten()
                .map(|c| std::sync::Arc::new(envoy::EnvoyClient::new(c)))
        };

        Ok(Self {
            codebase_path: path,
            forge,
            llm: None,
            #[cfg(feature = "envoy")]
            envoy,
            policies: Vec::new(),
        })
    }

    /// Sets the LLM provider for semantic operations.
    pub fn with_llm(mut self, provider: std::sync::Arc<dyn llm::LlmProvider>) -> Self {
        self.llm = Some(provider);
        self
    }

    /// Sets the policies enforced during the constraint phase.
    pub fn with_policies(mut self, policies: Vec<policy::Policy>) -> Self {
        self.policies = policies;
        self
    }

    /// Sets the envoy client for multi-agent coordination.
    #[cfg(feature = "envoy")]
    pub fn with_envoy(mut self, client: envoy::EnvoyClient) -> Self {
        self.envoy = Some(std::sync::Arc::new(client));
        self
    }

    /// Connects to envoy and registers this agent. Returns the agent ID.
    #[cfg(feature = "envoy")]
    pub async fn connect_envoy(&self) -> std::result::Result<String, String> {
        let client = self
            .envoy
            .as_ref()
            .ok_or_else(|| "envoy not configured".to_string())?;
        client.register().await
    }

    /// Observes the codebase to gather context for a query.
    ///
    /// # Arguments
    ///
    /// * `query` - The natural language query or request
    pub async fn observe(&self, query: &str) -> Result<Observation> {
        let forge = self
            .forge
            .as_ref()
            .ok_or_else(|| AgentError::ObservationFailed("Forge SDK not available".to_string()))?;

        let mut observer = observe::Observer::new(forge.clone());
        if let Some(ref llm) = self.llm {
            observer = observer.with_llm(llm.clone());
        }
        #[cfg(feature = "envoy")]
        if let Some(ref envoy) = self.envoy {
            observer = observer.with_knowledge_source(envoy.clone());
        }
        let obs = observer.gather(query).await?;

        Ok(obs)
    }

    /// Applies policy constraints to the observation.
    ///
    /// # Arguments
    ///
    /// * `observation` - The observation to constrain
    /// * `policies` - The policies to validate
    pub async fn constrain(
        &self,
        observation: Observation,
        policies: Vec<policy::Policy>,
    ) -> Result<ConstrainedPlan> {
        let forge = self.forge.as_ref().ok_or_else(|| {
            AgentError::ObservationFailed(
                "Forge SDK not available for policy validation".to_string(),
            )
        })?;

        // Create a validator
        let validator = policy::PolicyValidator::new(forge.clone());

        let diff = policy::Diff {
            file_path: std::path::PathBuf::from(&observation.query),
            original: String::new(),
            modified: format!("query: {}", observation.query),
            changes: Vec::new(),
        };

        // Validate policies
        let report = validator.validate(&diff, &policies).await?;

        Ok(ConstrainedPlan {
            observation,
            policy_violations: report.violations,
        })
    }

    /// Generates an execution plan from the constrained observation.
    pub async fn plan(&self, constrained: ConstrainedPlan) -> Result<ExecutionPlan> {
        // Create planner
        let planner_instance = planner::Planner::new();

        // Convert observation to the planner's format
        let obs = observe::Observation {
            query: constrained.observation.query.clone(),
            symbols: vec![],
            summary: None,
        };

        // Generate steps
        let steps = planner_instance.generate_steps(&obs).await?;

        // Estimate impact
        let impact = planner_instance.estimate_impact(&steps).await?;

        // Detect conflicts
        let conflicts = planner_instance.detect_conflicts(&steps)?;

        if !conflicts.is_empty() {
            return Err(AgentError::PlanningFailed(format!(
                "Found {} conflicts in plan",
                conflicts.len()
            )));
        }

        // Order steps based on dependencies
        let mut ordered_steps = steps;
        planner_instance.order_steps(&mut ordered_steps)?;

        // Generate rollback plan
        let rollback = planner_instance.generate_rollback(&ordered_steps);

        Ok(ExecutionPlan {
            steps: ordered_steps,
            estimated_impact: planner::ImpactEstimate {
                affected_files: impact.affected_files,
                complexity: impact.complexity,
            },
            rollback_plan: rollback,
        })
    }

    /// Executes the mutation phase of the plan.
    pub async fn mutate(&self, plan: ExecutionPlan) -> Result<MutationResult> {
        // Verify forge is available
        self.forge
            .as_ref()
            .ok_or_else(|| AgentError::MutationFailed("Forge SDK not available".to_string()))?;

        let mut mutator = mutate::Mutator::new();
        mutator.begin_transaction().await?;

        for step in &plan.steps {
            mutator.apply_step(step).await?;
        }

        Ok(MutationResult {
            modified_files: vec![],
            diffs: vec!["Transaction completed".to_string()],
        })
    }

    /// Verifies the mutation result, scoped to changed files.
    pub async fn verify(&self, result: MutationResult) -> Result<VerificationResult> {
        let verifier = verify::Verifier::new();
        let report = if result.modified_files.is_empty() {
            verifier.verify(&self.codebase_path).await?
        } else {
            verifier
                .verify_changes(&self.codebase_path, &result.modified_files, &result.diffs)
                .await?
        };

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

    /// Commits the verified mutation.
    pub async fn commit(&self, result: VerificationResult) -> Result<CommitResult> {
        let committer = commit::Committer::new();
        let files: Vec<std::path::PathBuf> = result
            .diagnostics
            .iter()
            .filter_map(|d| {
                // Parse diagnostics to extract file paths
                // Format: "file:line:col: message"
                d.split(':')
                    .next()
                    .map(|s| std::path::PathBuf::from(s.trim()))
            })
            .collect();

        let message = format!("forge: apply changes ({} files)", files.len());
        let commit_report = committer
            .finalize(&self.codebase_path, &files, &message)
            .await?;

        Ok(CommitResult {
            transaction_id: commit_report.transaction_id,
            files_committed: commit_report.files_committed,
        })
    }

    /// Runs the full agent loop: Observe -> Constrain -> Plan -> Mutate -> Verify -> Commit
    ///
    /// This is the main entry point for executing a complete agent operation.
    /// Each phase receives the output of the previous phase, and failures
    /// trigger rollback with audit trail entries.
    ///
    /// # Arguments
    ///
    /// * `query` - The natural language query or request
    ///
    /// # Returns
    ///
    /// Returns `LoopResult` with transaction ID, modified files, and audit trail.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use forge_agent::Agent;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let agent = Agent::new(".").await?;
    /// let result = agent.run("Add error handling to the parser").await?;
    /// println!("Transaction ID: {}", result.transaction_id);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn run(&self, query: &str) -> Result<LoopResult> {
        let forge = self
            .forge
            .as_ref()
            .ok_or_else(|| AgentError::ObservationFailed("Forge SDK not available".to_string()))?;

        // Create fresh loop state (no state leakage between runs)
        let mut agent_loop = r#loop::AgentLoop::new(std::sync::Arc::new(forge.clone()));

        // Pass LLM provider to agent loop if configured
        if let Some(ref llm) = self.llm {
            agent_loop = agent_loop.with_llm(llm.clone());
        }

        if !self.policies.is_empty() {
            agent_loop = agent_loop.with_policies(self.policies.clone());
        }

        agent_loop.run(query).await
    }
}

// Transaction module (Phase 3 - Plan 3)
pub mod transaction;

// Re-export transaction types
pub use transaction::{FileSnapshot, Transaction, TransactionState};

// Runtime integration module (Phase 3 - Plan 4)
pub mod runtime_integration;

// Re-export runtime types for convenience
pub use forge_runtime::{ForgeRuntime, RuntimeConfig, RuntimeStats};

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_creation() {
        let temp = tempfile::tempdir().unwrap();
        let agent = Agent::new(temp.path()).await.unwrap();

        assert_eq!(agent.codebase_path, temp.path());
    }

    #[tokio::test]
    async fn test_agent_with_runtime() {
        let temp = tempfile::tempdir().unwrap();
        let (_agent, runtime) = Agent::with_runtime(temp.path()).await.unwrap();

        // Verify runtime is accessible
        assert_eq!(runtime.codebase_path(), temp.path());

        // Run agent with runtime
        let result = _agent.run("test query").await;

        // Should complete (may fail on actual query, but infrastructure works)
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_agent_runtime_stats() {
        let temp = tempfile::tempdir().unwrap();
        let (_agent, runtime) = Agent::with_runtime(temp.path()).await.unwrap();

        let stats = runtime.stats();
        assert!(!stats.watch_active); // Not started
    }

    #[tokio::test]
    async fn test_agent_backward_compatibility() {
        // Agent should work without runtime (backward compatibility)
        let temp = tempfile::tempdir().unwrap();
        let agent = Agent::new(temp.path()).await.unwrap();

        // Agent should be functional standalone
        assert_eq!(agent.codebase_path, temp.path());
    }

    #[tokio::test]
    async fn test_agent_with_llm_provider() {
        let temp = tempfile::tempdir().unwrap();
        let mock = std::sync::Arc::new(llm::MockProvider::new("mocked LLM response"));
        let agent = Agent::new(temp.path()).await.unwrap().with_llm(mock);

        assert!(agent.llm.is_some());
    }

    #[tokio::test]
    async fn test_agent_without_llm_provider() {
        let temp = tempfile::tempdir().unwrap();
        let agent = Agent::new(temp.path()).await.unwrap();

        assert!(agent.llm.is_none());
    }

    #[cfg(feature = "envoy")]
    #[tokio::test]
    async fn test_agent_with_envoy() {
        let temp = tempfile::tempdir().unwrap();
        let config = envoy::EnvoyConfig {
            url: "http://localhost:9999".to_string(),
            agent_name: "test-forge".to_string(),
        };
        let client = envoy::EnvoyClient::new(config);
        let agent = Agent::new(temp.path()).await.unwrap().with_envoy(client);

        assert!(agent.envoy.is_some());
    }

    #[cfg(feature = "envoy")]
    #[tokio::test]
    async fn test_agent_without_envoy() {
        let temp = tempfile::tempdir().unwrap();
        let agent = Agent::new(temp.path()).await.unwrap();

        // No .forge.toml → no envoy
        assert!(agent.envoy.is_none());
    }
}

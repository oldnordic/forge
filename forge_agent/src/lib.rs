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
pub use policy::{Policy, PolicyValidator, PolicyReport, PolicyViolation};

// Re-export observation types
pub use observe::Observation;

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
pub struct Agent {
    /// Path to the codebase
    #[allow(dead_code)]
    codebase_path: PathBuf,
    /// Forge SDK instance for graph queries
    forge: Option<forge_core::Forge>,
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

        Ok(Self {
            codebase_path: path,
            forge,
        })
    }

    /// Observes the codebase to gather context for a query.
    ///
    /// # Arguments
    ///
    /// * `query` - The natural language query or request
    pub async fn observe(&self, query: &str) -> Result<Observation> {
        let forge = self.forge.as_ref()
            .ok_or_else(|| AgentError::ObservationFailed(
                "Forge SDK not available".to_string()
            ))?;

        let observer = observe::Observer::new(forge.clone());
        let obs = observer.gather(query).await?;

        // Return the observation directly - it's already the correct type
        Ok(obs)
    }

    /// Applies policy constraints to the observation.
    ///
    /// # Arguments
    ///
    /// * `observation` - The observation to constrain
    /// * `policies` - The policies to validate
    pub async fn constrain(&self, observation: Observation, policies: Vec<policy::Policy>) -> Result<ConstrainedPlan> {
        let forge = self.forge.as_ref()
            .ok_or_else(|| AgentError::ObservationFailed(
                "Forge SDK not available for policy validation".to_string()
            ))?;

        // Create a validator
        let validator = policy::PolicyValidator::new(forge.clone());

        // For observation, create a placeholder diff
        // In production, this would be the actual planned diff
        let diff = policy::Diff {
            file_path: std::path::PathBuf::from(""),
            original: String::new(),
            modified: String::new(),
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
        let forge = self.forge.as_ref()
            .ok_or_else(|| AgentError::PlanningFailed(
                "Forge SDK not available for planning".to_string()
            ))?;

        // Create planner
        let planner_instance = planner::Planner::new(forge.clone());

        // Convert observation to the planner's format
        let obs = observe::Observation {
            query: constrained.observation.query.clone(),
            symbols: vec![],
            references: vec![],
            cfg_data: vec![],
        };

        // Generate steps
        let steps = planner_instance.generate_steps(&obs).await?;

        // Estimate impact
        let impact = planner_instance.estimate_impact(&steps).await?;

        // Detect conflicts
        let conflicts = planner_instance.detect_conflicts(&steps)?;

        if !conflicts.is_empty() {
            return Err(AgentError::PlanningFailed(
                format!("Found {} conflicts in plan", conflicts.len())
            ));
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
        let forge = self.forge.as_ref()
            .ok_or_else(|| AgentError::MutationFailed(
                "Forge SDK not available".to_string()
            ))?;

        let mutator = mutate::Mutator::new(forge.clone());
        mutator.begin_transaction().await?;

        for step in &plan.steps {
            mutator.apply_step(step).await?;
        }

        Ok(MutationResult {
            modified_files: vec![],
            diffs: vec!["Transaction completed".to_string()],
        })
    }

    /// Verifies the mutation result.
    pub async fn verify(&self, _result: MutationResult) -> Result<VerificationResult> {
        let forge = self.forge.as_ref()
            .ok_or_else(|| AgentError::VerificationFailed(
                "Forge SDK not available".to_string()
            ))?;

        let verifier = verify::Verifier::new(forge.clone());
        let report = verifier.verify(&self.codebase_path).await?;

        Ok(VerificationResult {
            passed: report.passed,
            diagnostics: report.diagnostics.iter()
                .map(|d| d.message.clone())
                .collect(),
        })
    }

    /// Commits the verified mutation.
    pub async fn commit(&self, result: VerificationResult) -> Result<CommitResult> {
        let forge = self.forge.as_ref()
            .ok_or_else(|| AgentError::CommitFailed(
                "Forge SDK not available".to_string()
            ))?;

        let committer = commit::Committer::new(forge.clone());
        let commit_report = committer.finalize(&self.codebase_path, &result.diagnostics).await?;

        Ok(CommitResult {
            transaction_id: commit_report.transaction_id,
            files_committed: commit_report.files_committed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_creation() {
        let temp = tempfile::tempdir().unwrap();
        let agent = Agent::new(temp.path()).await.unwrap();

        assert_eq!(agent.codebase_path, temp.path());
    }
}

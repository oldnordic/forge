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
//! This crate is under active development. Observation phase is implemented.

use std::path::PathBuf;

// Observation module (Phase 4 - Task 1)
pub mod observe;

// Policy module (Phase 4 - Task 2)
pub mod policy;

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

        Ok(Observation {
            relevant_symbols: obs.symbols.iter()
                .map(|s| format!("{} at {:?}", s.name, s.location))
                .collect(),
            references: obs.references.iter()
                .map(|r| format!("{:?} -> {:?}", r.from, r.to))
                .collect(),
            cfg_data: obs.cfg_data.iter()
                .map(|c| format!("symbol {:?}: {} paths, complexity {}", c.symbol_id, c.path_count, c.complexity))
                .collect(),
        })
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
    pub async fn plan(&self, _constrained: ConstrainedPlan) -> Result<ExecutionPlan> {
        // TODO: Implement planning
        Err(AgentError::PlanningFailed(
            "Planning not yet implemented".to_string()
        ))
    }

    /// Executes the mutation phase of the plan.
    pub async fn mutate(&self, _plan: ExecutionPlan) -> Result<MutationResult> {
        // TODO: Implement mutation via edit module
        Err(AgentError::MutationFailed(
            "Mutation not yet implemented".to_string()
        ))
    }

    /// Verifies the mutation result.
    pub async fn verify(&self, _result: MutationResult) -> Result<VerificationResult> {
        // TODO: Implement verification
        Err(AgentError::VerificationFailed(
            "Verification not yet implemented".to_string()
        ))
    }

    /// Commits the verified mutation.
    pub async fn commit(&self, _result: VerificationResult) -> Result<CommitResult> {
        // TODO: Implement commit
        Err(AgentError::CommitFailed(
            "Commit not yet implemented".to_string()
        ))
    }
}

/// Result of the observation phase.
#[derive(Clone, Debug)]
pub struct Observation {
    /// Symbols relevant to the query
    pub relevant_symbols: Vec<String>,
    /// References found
    pub references: Vec<String>,
    /// CFG data for relevant functions
    pub cfg_data: Vec<String>,
}

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
    pub steps: Vec<PlanStep>,
    /// Estimated impact
    pub estimated_impact: ImpactEstimate,
}

/// A single step in the execution plan.
#[derive(Clone, Debug)]
pub struct PlanStep {
    /// Step description
    pub description: String,
    /// Operation to perform
    pub operation: PlanOperation,
}

/// Operation to perform in a plan step.
#[derive(Clone, Debug)]
pub enum PlanOperation {
    /// Rename a symbol
    Rename { old: String, new: String },
    /// Delete a symbol
    Delete { name: String },
    /// Create new code
    Create { path: String, content: String },
}

/// Estimated impact of a plan.
#[derive(Clone, Debug)]
pub struct ImpactEstimate {
    /// Files to be modified
    pub affected_files: Vec<PathBuf>,
    /// Estimated complexity
    pub complexity: usize,
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

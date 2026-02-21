//! Confidence propagation through dependency graphs
//!
//! This module provides algorithms for propagating confidence changes through
//! belief dependency graphs using BFS traversal with depth-based decay.
//! Cycles are detected and normalized to consistent confidence values.

use petgraph::algo::tarjan_scc;
use std::collections::{HashMap, VecDeque};
use indexmap::IndexSet;

use crate::belief::BeliefGraph;
use crate::hypothesis::{
    confidence::{Confidence, ConfidenceError},
    types::HypothesisId,
    HypothesisBoard,
};
use crate::errors::Result as ReasoningResult;

/// Single confidence change during cascade propagation
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ConfidenceChange {
    pub hypothesis_id: HypothesisId,
    pub hypothesis_name: String,
    pub old_confidence: Confidence,
    pub new_confidence: Confidence,
    pub delta: f64,  // new - old
    pub depth: usize,  // Distance from start node
    pub propagation_path: Vec<HypothesisId>,  // Path from start to this node
}

/// Configuration for confidence propagation
#[derive(Clone, Debug)]
pub struct PropagationConfig {
    /// Decay factor per dependency level (default: 0.95)
    pub decay_factor: f64,
    /// Minimum confidence floor to prevent underflow (default: 0.1)
    pub min_confidence: f64,
    /// Maximum cascade size safety limit (default: 10,000)
    pub max_cascade_size: usize,
}

impl Default for PropagationConfig {
    fn default() -> Self {
        Self {
            decay_factor: 0.95,
            min_confidence: 0.1,
            max_cascade_size: 10_000,
        }
    }
}

/// Errors that can occur during cascade propagation
#[derive(Debug, thiserror::Error)]
pub enum CascadeError {
    #[error("Cascade too large: {size} hypotheses affected, limit is {limit}")]
    CascadeTooLarge { size: usize, limit: usize },

    #[error("Hypothesis not found: {0}")]
    HypothesisNotFound(HypothesisId),

    #[error("Confidence validation error: {0}")]
    ConfidenceError(#[from] ConfidenceError),

    #[error("Cycle detected and normalization failed")]
    CycleNormalizationFailed,

    #[error("Graph error: {0}")]
    GraphError(String),
}

/// Result of computing a confidence cascade
#[derive(Clone, Debug)]
pub struct PropagationResult {
    pub changes: Vec<ConfidenceChange>,
    pub cycles_detected: bool,
    pub normalized_cycles: usize,
    pub total_affected: usize,
    pub max_depth: usize,
}

// Placeholder function declarations - will be implemented in Task 2
pub async fn compute_cascade(
    _start: HypothesisId,
    _new_confidence: Confidence,
    _board: &HypothesisBoard,
    _graph: &BeliefGraph,
    _config: &PropagationConfig,
) -> std::result::Result<PropagationResult, CascadeError> {
    // TODO: Implement in Task 2
    Err(CascadeError::GraphError("compute_cascade not yet implemented".to_string()))
}

pub fn normalize_cycles(
    _changes: &mut [ConfidenceChange],
    _graph: &BeliefGraph,
) -> std::result::Result<usize, CascadeError> {
    // TODO: Implement in Task 2
    Err(CascadeError::CycleNormalizationFailed)
}

pub async fn propagate_confidence(
    _result: PropagationResult,
    _board: &std::sync::Arc<HypothesisBoard>,
) -> ReasoningResult<()> {
    // TODO: Implement in Task 2
    Err(crate::errors::ReasoningError::InvalidState(
        "propagate_confidence not yet implemented".to_string()
    ))
}

pub async fn impact_radius(
    _start: HypothesisId,
    _graph: &BeliefGraph,
) -> ReasoningResult<usize> {
    // TODO: Implement in Task 2
    Err(crate::errors::ReasoningError::InvalidState(
        "impact_radius not yet implemented".to_string()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_propagation_config_default() {
        let config = PropagationConfig::default();
        assert_eq!(config.decay_factor, 0.95);
        assert_eq!(config.min_confidence, 0.1);
        assert_eq!(config.max_cascade_size, 10_000);
    }

    #[test]
    fn test_cascade_error_display() {
        let err = CascadeError::CascadeTooLarge { size: 100, limit: 50 };
        assert!(err.to_string().contains("100"));
        assert!(err.to_string().contains("50"));
    }
}

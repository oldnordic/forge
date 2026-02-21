//! Hypothesis management with Bayesian confidence tracking
//!
//! This module provides tools for proposing, tracking, and updating hypotheses
//! during debugging. LLMs can maintain explicit belief states with proper
//! Bayesian updates as evidence accumulates.

pub mod confidence;
pub mod types;
pub mod evidence;
pub mod storage;

// Public exports
pub use confidence::{Confidence, ConfidenceError};
pub use types::{Hypothesis, HypothesisId, HypothesisStatus};
pub use storage::{HypothesisStorage, InMemoryHypothesisStorage};

use std::sync::Arc;
use crate::errors::Result;

/// Main API for hypothesis management
pub struct HypothesisBoard {
    storage: Arc<dyn HypothesisStorage>,
}

impl HypothesisBoard {
    pub fn new(storage: Arc<dyn HypothesisStorage>) -> Self {
        Self { storage }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(InMemoryHypothesisStorage::new()))
    }

    /// Propose a new hypothesis with explicit prior
    pub async fn propose(
        &self,
        statement: impl Into<String>,
        prior: Confidence,
    ) -> Result<HypothesisId> {
        let hypothesis = Hypothesis::new(statement, prior);
        self.storage.create_hypothesis(&hypothesis).await
    }

    /// Propose with maximum uncertainty (0.5) convenience method
    pub async fn propose_with_max_uncertainty(
        &self,
        statement: impl Into<String>,
    ) -> Result<HypothesisId> {
        self.propose(statement, Confidence::max_uncertainty()).await
    }

    /// Update hypothesis confidence using Bayes formula
    pub async fn update_with_evidence(
        &self,
        id: HypothesisId,
        likelihood_h: f64,
        likelihood_not_h: f64,
    ) -> Result<Confidence> {
        let hypothesis = self.storage.get_hypothesis(id).await?
            .ok_or_else(|| crate::errors::ReasoningError::NotFound(
                format!("Hypothesis {} not found", id)
            ))?;

        let current = hypothesis.current_confidence();
        let posterior = current.update_with_evidence(likelihood_h, likelihood_not_h)
            .map_err(|e| crate::errors::ReasoningError::InvalidState(e.to_string()))?;

        self.storage.update_confidence(id, posterior).await?;
        Ok(posterior)
    }

    /// Update hypothesis status
    pub async fn set_status(
        &self,
        id: HypothesisId,
        status: HypothesisStatus,
    ) -> Result<()> {
        self.storage.set_status(id, status).await
    }

    /// Get a hypothesis by ID
    pub async fn get(&self, id: HypothesisId) -> Result<Option<Hypothesis>> {
        self.storage.get_hypothesis(id).await
    }

    /// List all hypotheses
    pub async fn list(&self) -> Result<Vec<Hypothesis>> {
        self.storage.list_hypotheses().await
    }

    /// Delete a hypothesis
    pub async fn delete(&self, id: HypothesisId) -> Result<bool> {
        self.storage.delete_hypothesis(id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_propose_hypothesis() {
        let board = HypothesisBoard::in_memory();
        let prior = Confidence::new(0.5).unwrap();
        let id = board.propose("Test hypothesis", prior).await.unwrap();
        assert!(board.get(id).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_confidence_rejects_nan() {
        assert!(Confidence::new(f64::NAN).is_err());
    }

    #[tokio::test]
    async fn test_confidence_rejects_out_of_bounds() {
        assert!(Confidence::new(1.5).is_err());
        assert!(Confidence::new(-0.1).is_err());
    }

    #[tokio::test]
    async fn test_bayes_update() {
        let board = HypothesisBoard::in_memory();
        let prior = Confidence::new(0.5).unwrap();
        let id = board.propose("Test", prior).await.unwrap();

        // Supporting evidence: P(E|H) = 0.9, P(E|¬H) = 0.1
        let posterior = board.update_with_evidence(id, 0.9, 0.1).await.unwrap();

        // Posterior should be higher than prior (0.5 -> ~0.9)
        assert!(posterior.get() > 0.8);

        let h = board.get(id).await.unwrap().unwrap();
        assert_eq!(h.posterior(), posterior);
    }

    #[tokio::test]
    async fn test_status_transitions() {
        let board = HypothesisBoard::in_memory();
        let prior = Confidence::new(0.5).unwrap();
        let id = board.propose("Test", prior).await.unwrap();

        // Valid: Proposed -> UnderTest
        board.set_status(id, HypothesisStatus::UnderTest).await.unwrap();

        // Valid: UnderTest -> Confirmed
        board.set_status(id, HypothesisStatus::Confirmed).await.unwrap();

        // Invalid: Confirmed -> Proposed (should fail)
        assert!(board.set_status(id, HypothesisStatus::Proposed).await.is_err());
    }
}

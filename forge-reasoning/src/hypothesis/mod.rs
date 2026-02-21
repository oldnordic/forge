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
pub use types::{Hypothesis, HypothesisId, HypothesisStatus, HypothesisState};
pub use storage::{HypothesisStorage, InMemoryHypothesisStorage};
pub use evidence::{Evidence, EvidenceId, EvidenceType, EvidenceMetadata, strength_to_likelihood};

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

    /// Attach evidence to a hypothesis and update confidence
    ///
    /// This is the primary method for evidence attachment. It:
    /// 1. Stores the evidence
    /// 2. Converts strength to likelihood ratio
    /// 3. Updates hypothesis confidence using Bayes formula
    pub async fn attach_evidence(
        &self,
        hypothesis_id: HypothesisId,
        evidence_type: EvidenceType,
        strength: f64,
        metadata: EvidenceMetadata,
    ) -> Result<(EvidenceId, Confidence)> {
        // Create evidence
        let evidence = Evidence::new(hypothesis_id, evidence_type, strength, metadata);
        let evidence_id = self.storage.attach_evidence(&evidence).await?;

        // Convert strength to likelihood ratio
        let (likelihood_h, likelihood_not_h) =
            strength_to_likelihood(evidence.strength(), evidence_type);

        // Update hypothesis confidence
        let posterior = self.update_with_evidence(
            hypothesis_id,
            likelihood_h,
            likelihood_not_h,
        ).await?;

        Ok((evidence_id, posterior))
    }

    /// Get evidence by ID
    pub async fn get_evidence(&self, id: EvidenceId) -> Result<Option<Evidence>> {
        self.storage.get_evidence(id).await
    }

    /// List all evidence for a hypothesis
    pub async fn list_evidence(&self, hypothesis_id: HypothesisId) -> Result<Vec<Evidence>> {
        self.storage.list_evidence_for_hypothesis(hypothesis_id).await
    }

    /// Trace supporting evidence for a hypothesis
    pub async fn list_supporting_evidence(&self, hypothesis_id: HypothesisId) -> Result<Vec<Evidence>> {
        let all = self.list_evidence(hypothesis_id).await?;
        Ok(all.into_iter().filter(|e| e.is_supporting()).collect())
    }

    /// Trace refuting evidence for a hypothesis
    pub async fn list_refuting_evidence(&self, hypothesis_id: HypothesisId) -> Result<Vec<Evidence>> {
        let all = self.list_evidence(hypothesis_id).await?;
        Ok(all.into_iter().filter(|e| e.is_refuting()).collect())
    }

    /// Delete evidence
    pub async fn delete_evidence(&self, id: EvidenceId) -> Result<bool> {
        self.storage.delete_evidence(id).await
    }

    /// Query hypothesis state at a past checkpoint time
    ///
    /// This enables time-travel queries: "What did I believe at checkpoint X?"
    pub async fn state_at(
        &self,
        checkpoint_service: &crate::service::CheckpointService,
        checkpoint_id: crate::checkpoint::CheckpointId,
    ) -> Result<Option<crate::hypothesis::types::HypothesisState>> {
        // Query checkpoint service for hypothesis state at given checkpoint
        checkpoint_service.get_hypothesis_state(checkpoint_id).await
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

    #[tokio::test]
    async fn test_attach_evidence_updates_confidence() {
        let board = HypothesisBoard::in_memory();
        let prior = Confidence::new(0.5).unwrap();
        let id = board.propose("Test hypothesis", prior).await.unwrap();

        // Attach supporting evidence
        let metadata = EvidenceMetadata::Observation {
            description: "Observed behavior supports hypothesis".to_string(),
            source_path: None,
        };

        let (evidence_id, posterior) = board.attach_evidence(
            id,
            EvidenceType::Observation,
            0.5,  // Max supporting strength for Observation
            metadata,
        ).await.unwrap();

        // Posterior should be higher than prior
        assert!(posterior.get() > 0.5);

        // Evidence should be retrievable
        let evidence = board.get_evidence(evidence_id).await.unwrap().unwrap();
        assert_eq!(evidence.hypothesis_id(), id);
    }

    #[tokio::test]
    async fn test_list_evidence_by_hypothesis() {
        let board = HypothesisBoard::in_memory();
        let prior = Confidence::new(0.5).unwrap();
        let id = board.propose("Test", prior).await.unwrap();

        // Attach multiple evidence
        for i in 0..3 {
            let metadata = EvidenceMetadata::Observation {
                description: format!("Observation {}", i),
                source_path: None,
            };
            board.attach_evidence(id, EvidenceType::Observation, 0.3, metadata).await.unwrap();
        }

        let evidence_list = board.list_evidence(id).await.unwrap();
        assert_eq!(evidence_list.len(), 3);
    }

    #[tokio::test]
    async fn test_supporting_vs_refuting_evidence() {
        let board = HypothesisBoard::in_memory();
        let prior = Confidence::new(0.5).unwrap();
        let id = board.propose("Test", prior).await.unwrap();

        // Supporting evidence
        let metadata_support = EvidenceMetadata::Observation {
            description: "Supporting".to_string(),
            source_path: None,
        };
        board.attach_evidence(id, EvidenceType::Observation, 0.4, metadata_support).await.unwrap();

        // Refuting evidence
        let metadata_refute = EvidenceMetadata::Observation {
            description: "Refuting".to_string(),
            source_path: None,
        };
        board.attach_evidence(id, EvidenceType::Observation, -0.4, metadata_refute).await.unwrap();

        let supporting = board.list_supporting_evidence(id).await.unwrap();
        let refuting = board.list_refuting_evidence(id).await.unwrap();

        assert_eq!(supporting.len(), 1);
        assert_eq!(refuting.len(), 1);
    }

    #[tokio::test]
    async fn test_evidence_strength_clamping_by_type() {
        let board = HypothesisBoard::in_memory();
        let prior = Confidence::new(0.5).unwrap();
        let id = board.propose("Test", prior).await.unwrap();

        let metadata = EvidenceMetadata::Observation {
            description: "Test".to_string(),
            source_path: None,
        };

        // Try to attach evidence with strength 1.0 (beyond Observation's max of 0.5)
        let (evidence_id, _) = board.attach_evidence(
            id,
            EvidenceType::Observation,
            1.0,  // Will be clamped to 0.5
            metadata,
        ).await.unwrap();

        let evidence = board.get_evidence(evidence_id).await.unwrap().unwrap();
        assert_eq!(evidence.strength(), 0.5); // Clamped
    }
}

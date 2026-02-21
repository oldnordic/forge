//! Core hypothesis types
//!
//! Defines the hypothesis data model including:
//! - HypothesisId: Unique identifier (UUID v4)
//! - HypothesisStatus: Lifecycle state machine
//! - Hypothesis: Main data structure with confidence tracking
//! - HypothesisState: Checkpoint snapshot of all hypotheses

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::hypothesis::confidence::Confidence;

/// Unique identifier for a hypothesis
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HypothesisId(pub Uuid);

impl HypothesisId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for HypothesisId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for HypothesisId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Lifecycle status of a hypothesis
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HypothesisStatus {
    /// Initial state, hypothesis proposed but not yet investigated
    Proposed,
    /// Currently being tested or investigated
    UnderTest,
    /// Evidence supports the hypothesis
    Confirmed,
    /// Evidence contradicts the hypothesis
    Rejected,
}

impl HypothesisStatus {
    /// Valid status transitions
    ///
    /// Returns true if transition from self to next is valid
    pub fn can_transition_to(&self, next: &HypothesisStatus) -> bool {
        match (self, next) {
            (HypothesisStatus::Proposed, HypothesisStatus::UnderTest) => true,
            (HypothesisStatus::UnderTest, HypothesisStatus::Confirmed) => true,
            (HypothesisStatus::UnderTest, HypothesisStatus::Rejected) => true,
            (HypothesisStatus::Proposed, HypothesisStatus::Rejected) => true,
            _ => false,
        }
    }
}

/// A hypothesis with Bayesian confidence tracking
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hypothesis {
    pub id: HypothesisId,
    pub statement: String,
    pub prior: Confidence,
    pub posterior: Confidence,
    pub status: HypothesisStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Hypothesis {
    /// Create a new hypothesis with explicit prior confidence
    pub fn new(statement: impl Into<String>, prior: Confidence) -> Self {
        let id = HypothesisId::new();
        let now = Utc::now();
        Self {
            id,
            statement: statement.into(),
            prior,
            posterior: prior, // Initially equal
            status: HypothesisStatus::Proposed,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn id(&self) -> HypothesisId {
        self.id
    }

    pub fn statement(&self) -> &str {
        &self.statement
    }

    pub fn prior(&self) -> Confidence {
        self.prior
    }

    pub fn posterior(&self) -> Confidence {
        self.posterior
    }

    pub fn status(&self) -> HypothesisStatus {
        self.status
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }

    /// Update posterior based on evidence
    pub fn update_posterior(&mut self, new_posterior: Confidence) -> Result<(), String> {
        self.posterior = new_posterior;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Transition to a new status
    pub fn set_status(&mut self, new_status: HypothesisStatus) -> Result<(), String> {
        if !self.status.can_transition_to(&new_status) {
            return Err(format!(
                "Invalid status transition: {:?} -> {:?}",
                self.status, new_status
            ));
        }
        self.status = new_status;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Get current confidence (posterior, or prior if no updates yet)
    pub fn current_confidence(&self) -> Confidence {
        self.posterior
    }
}

/// Snapshot of all hypotheses and their state at a point in time
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HypothesisState {
    /// All hypotheses and their current state
    pub hypotheses: Vec<Hypothesis>,
    /// Dependency edges between hypotheses (from_id, to_id)
    pub dependencies: Vec<(HypothesisId, HypothesisId)>,
    /// Snapshot timestamp
    pub captured_at: DateTime<Utc>,
    /// Global sequence for checkpoint ordering
    pub sequence: u64,
}

impl HypothesisState {
    pub fn new(
        hypotheses: Vec<Hypothesis>,
        dependencies: Vec<(HypothesisId, HypothesisId)>,
        sequence: u64,
    ) -> Self {
        Self {
            hypotheses,
            dependencies,
            captured_at: Utc::now(),
            sequence,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hypothesis_id_new() {
        let id = HypothesisId::new();
        // UUID should be valid (non-nil)
        assert_ne!(id.0, Uuid::nil());
    }

    #[test]
    fn test_hypothesis_id_default() {
        let id = HypothesisId::default();
        assert_ne!(id.0, Uuid::nil());
    }

    #[test]
    fn test_hypothesis_id_display() {
        let id = HypothesisId::new();
        let s = format!("{}", id);
        // UUID string format
        assert!(s.len() > 0);
    }

    #[test]
    fn test_status_valid_transitions() {
        assert!(HypothesisStatus::Proposed.can_transition_to(&HypothesisStatus::UnderTest));
        assert!(HypothesisStatus::UnderTest.can_transition_to(&HypothesisStatus::Confirmed));
        assert!(HypothesisStatus::UnderTest.can_transition_to(&HypothesisStatus::Rejected));
        assert!(HypothesisStatus::Proposed.can_transition_to(&HypothesisStatus::Rejected));
    }

    #[test]
    fn test_status_invalid_transitions() {
        // Cannot go backwards
        assert!(!HypothesisStatus::Confirmed.can_transition_to(&HypothesisStatus::UnderTest));
        assert!(!HypothesisStatus::Rejected.can_transition_to(&HypothesisStatus::Proposed));
        // Cannot skip states
        assert!(!HypothesisStatus::Proposed.can_transition_to(&HypothesisStatus::Confirmed));
    }

    #[test]
    fn test_hypothesis_new() {
        let prior = Confidence::new(0.7).unwrap();
        let h = Hypothesis::new("Test hypothesis", prior);

        assert_eq!(h.statement(), "Test hypothesis");
        assert_eq!(h.prior(), prior);
        assert_eq!(h.posterior(), prior);
        assert_eq!(h.status(), HypothesisStatus::Proposed);
    }

    #[test]
    fn test_hypothesis_update_posterior() {
        let prior = Confidence::new(0.5).unwrap();
        let mut h = Hypothesis::new("Test", prior);

        let new_posterior = Confidence::new(0.8).unwrap();
        h.update_posterior(new_posterior).unwrap();

        assert_eq!(h.posterior(), new_posterior);
        assert!(h.updated_at > h.created_at);
    }

    #[test]
    fn test_hypothesis_set_status_valid() {
        let prior = Confidence::new(0.5).unwrap();
        let mut h = Hypothesis::new("Test", prior);

        h.set_status(HypothesisStatus::UnderTest).unwrap();
        assert_eq!(h.status(), HypothesisStatus::UnderTest);
    }

    #[test]
    fn test_hypothesis_set_status_invalid() {
        let prior = Confidence::new(0.5).unwrap();
        let mut h = Hypothesis::new("Test", prior);

        // Try to skip from Proposed to Confirmed
        let result = h.set_status(HypothesisStatus::Confirmed);
        assert!(result.is_err());
    }

    #[test]
    fn test_current_confidence() {
        let prior = Confidence::new(0.5).unwrap();
        let h = Hypothesis::new("Test", prior);
        assert_eq!(h.current_confidence(), prior);
    }
}

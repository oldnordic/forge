//! Evidence attachment system for hypotheses
//!
//! Provides types for attaching evidence to hypotheses with four evidence types:
//! Observation, Experiment, Reference, and Deduction. Each type has a specific
//! strength range that maps to likelihood ratios in Bayesian updates.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use super::types::HypothesisId;

/// Unique identifier for evidence
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EvidenceId(pub Uuid);

impl EvidenceId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for EvidenceId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for EvidenceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Type of evidence with type-specific strength ranges
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EvidenceType {
    /// Direct observation (strength range: ±0.5)
    Observation,
    /// Controlled experiment (strength range: ±1.0)
    Experiment,
    /// External reference (strength range: ±0.3)
    Reference,
    /// Logical deduction from premises (strength range: ±0.7)
    Deduction,
}

impl EvidenceType {
    /// Maximum strength for this evidence type
    pub fn max_strength(&self) -> f64 {
        match self {
            Self::Observation => 0.5,
            Self::Experiment => 1.0,
            Self::Reference => 0.3,
            Self::Deduction => 0.7,
        }
    }

    /// Clamp strength to valid range for this type
    pub fn clamp_strength(&self, strength: f64) -> f64 {
        let max = self.max_strength();
        strength.clamp(-max, max)
    }
}

/// Type-specific metadata for evidence
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EvidenceMetadata {
    Observation {
        description: String,
        source_path: Option<PathBuf>,
    },
    Experiment {
        name: String,
        test_command: String,
        output: String,
        passed: bool,
    },
    Reference {
        citation: String,
        url: Option<String>,
        author: Option<String>,
    },
    Deduction {
        premises: Vec<HypothesisId>,
        reasoning: String,
    },
}

/// Evidence attached to a hypothesis
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Evidence {
    pub id: EvidenceId,
    pub evidence_type: EvidenceType,
    pub hypothesis_id: HypothesisId,
    pub strength: f64,
    pub metadata: EvidenceMetadata,
    pub created_at: DateTime<Utc>,
}

impl Evidence {
    pub fn new(
        hypothesis_id: HypothesisId,
        evidence_type: EvidenceType,
        strength: f64,
        metadata: EvidenceMetadata,
    ) -> Self {
        let clamped_strength = evidence_type.clamp_strength(strength);

        Self {
            id: EvidenceId::new(),
            evidence_type,
            hypothesis_id,
            strength: clamped_strength,
            metadata,
            created_at: Utc::now(),
        }
    }

    pub fn id(&self) -> EvidenceId {
        self.id
    }

    pub fn hypothesis_id(&self) -> HypothesisId {
        self.hypothesis_id
    }

    pub fn strength(&self) -> f64 {
        self.strength
    }

    pub fn evidence_type(&self) -> EvidenceType {
        self.evidence_type
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Check if evidence is supporting (positive strength)
    pub fn is_supporting(&self) -> bool {
        self.strength > 0.0
    }

    /// Check if evidence is refuting (negative strength)
    pub fn is_refuting(&self) -> bool {
        self.strength < 0.0
    }
}

/// Convert evidence strength to likelihood ratio for Bayes update
pub fn strength_to_likelihood(strength: f64, evidence_type: EvidenceType) -> (f64, f64) {
    let max_strength = evidence_type.max_strength();
    let clamped = strength.clamp(-max_strength, max_strength);

    // Base probability (no evidence = 0.5 for both)
    const BASE: f64 = 0.5;

    // Scale adjustment: strength moves probability away from base
    let adjustment = (clamped / max_strength) * 0.4; // Max 0.4 adjustment

    if clamped >= 0.0 {
        // Supporting: P(E|H) > P(E|¬H)
        (BASE + adjustment, BASE - adjustment)
    } else {
        // Refuting: P(E|H) < P(E|¬H) - adjustment is negative
        (BASE + adjustment, BASE - adjustment)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strength_ranges() {
        assert_eq!(EvidenceType::Observation.max_strength(), 0.5);
        assert_eq!(EvidenceType::Experiment.max_strength(), 1.0);
        assert_eq!(EvidenceType::Reference.max_strength(), 0.3);
        assert_eq!(EvidenceType::Deduction.max_strength(), 0.7);
    }

    #[test]
    fn test_strength_clamping() {
        let evidence_type = EvidenceType::Observation;
        assert_eq!(evidence_type.clamp_strength(1.0), 0.5); // Clamped down
        assert_eq!(evidence_type.clamp_strength(-1.0), -0.5); // Clamped up
        assert_eq!(evidence_type.clamp_strength(0.3), 0.3); // Within range
    }

    #[test]
    fn test_strength_to_likelihood_supporting() {
        // Strong supporting evidence
        let (p_e_given_h, p_e_given_not_h) = strength_to_likelihood(0.9, EvidenceType::Experiment);
        assert!(p_e_given_h > p_e_given_not_h);
        assert!(p_e_given_h > 0.5);
        assert!(p_e_given_not_h < 0.5);
    }

    #[test]
    fn test_strength_to_likelihood_refuting() {
        // Refuting evidence
        let (p_e_given_h, p_e_given_not_h) = strength_to_likelihood(-0.5, EvidenceType::Experiment);
        assert!(p_e_given_h < p_e_given_not_h);
        assert!(p_e_given_h < 0.5);
        assert!(p_e_given_not_h > 0.5);
    }

    #[test]
    fn test_evidence_creation_clamps_strength() {
        let hypothesis_id = HypothesisId::new();
        let metadata = EvidenceMetadata::Observation {
            description: "Test".to_string(),
            source_path: None,
        };

        // Try to create evidence with strength 1.0 for Observation (max is 0.5)
        let evidence = Evidence::new(
            hypothesis_id,
            EvidenceType::Observation,
            1.0,
            metadata,
        );

        assert_eq!(evidence.strength, 0.5); // Clamped to max
    }

    #[test]
    fn test_evidence_id_generation() {
        let id1 = EvidenceId::new();
        let id2 = EvidenceId::new();
        assert_ne!(id1, id2); // UUIDs should be unique
    }

    #[test]
    fn test_evidence_supporting_refuting() {
        let hypothesis_id = HypothesisId::new();

        let supporting = Evidence::new(
            hypothesis_id,
            EvidenceType::Observation,
            0.3,
            EvidenceMetadata::Observation {
                description: "Supporting".to_string(),
                source_path: None,
            },
        );
        assert!(supporting.is_supporting());
        assert!(!supporting.is_refuting());

        let refuting = Evidence::new(
            hypothesis_id,
            EvidenceType::Observation,
            -0.3,
            EvidenceMetadata::Observation {
                description: "Refuting".to_string(),
                source_path: None,
            },
        );
        assert!(!refuting.is_supporting());
        assert!(refuting.is_refuting());
    }

    #[test]
    fn test_strength_to_likelihood_max_experiment() {
        // Maximum supporting strength for Experiment
        let (p_e_given_h, p_e_given_not_h) = strength_to_likelihood(1.0, EvidenceType::Experiment);
        assert!((p_e_given_h - 0.9).abs() < 1e-10); // BASE + 0.4
        assert!((p_e_given_not_h - 0.1).abs() < 1e-10); // BASE - 0.4
    }

    #[test]
    fn test_strength_to_likelihood_zero_strength() {
        // Zero strength should return equal probabilities
        let (p_e_given_h, p_e_given_not_h) = strength_to_likelihood(0.0, EvidenceType::Observation);
        assert_eq!(p_e_given_h, 0.5);
        assert_eq!(p_e_given_not_h, 0.5);
    }
}

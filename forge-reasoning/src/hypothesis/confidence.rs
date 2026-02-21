//! Bounded confidence value with Bayesian update support
//!
//! Confidence is a newtype wrapper around f64 that enforces bounds [0.0, 1.0]
//! and rejects NaN values. This provides type-safe probability values for
//! hypothesis tracking.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Bounded confidence value [0.0, 1.0]
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Confidence(f64);

#[derive(Error, Debug)]
pub enum ConfidenceError {
    #[error("Confidence value cannot be NaN")]
    NaN,

    #[error("Confidence out of bounds: {value} (must be {min} to {max})")]
    OutOfBounds { value: f64, min: f64, max: f64 },

    #[error("Evidence probability is zero, cannot compute Bayes update")]
    ZeroEvidenceProbability,
}

impl Confidence {
    const MIN: f64 = 0.0;
    const MAX: f64 = 1.0;

    /// Create a new confidence value with bounds validation
    ///
    /// # Errors
    /// - Returns `ConfidenceError::NaN` if value is NaN
    /// - Returns `ConfidenceError::OutOfBounds` if value < 0.0 or > 1.0
    pub fn new(value: f64) -> Result<Self, ConfidenceError> {
        if value.is_nan() {
            return Err(ConfidenceError::NaN);
        }
        if value < Self::MIN || value > Self::MAX {
            return Err(ConfidenceError::OutOfBounds {
                value,
                min: Self::MIN,
                max: Self::MAX,
            });
        }
        Ok(Self(value))
    }

    /// Get the underlying f64 value
    pub fn get(self) -> f64 {
        self.0
    }

    /// Update confidence using Bayes theorem
    ///
    /// P(H|E) = P(E|H) * P(H) / P(E)
    ///
    /// # Arguments
    /// - `likelihood_h`: P(E|H) - probability of evidence given hypothesis is true
    /// - `likelihood_not_h`: P(E|¬H) - probability of evidence given hypothesis is false
    ///
    /// # Returns
    /// New confidence value (posterior) based on Bayes formula
    ///
    /// # Errors
    /// - Returns error if the resulting posterior is invalid (NaN or out of bounds)
    pub fn update_with_evidence(
        self,
        likelihood_h: f64,  // P(E|H)
        likelihood_not_h: f64,  // P(E|¬H)
    ) -> Result<Self, ConfidenceError> {
        let prior = self.0;

        // P(E) = P(E|H) * P(H) + P(E|¬H) * P(¬H)
        let p_e = (likelihood_h * prior) + (likelihood_not_h * (1.0 - prior));

        // Guard against division by zero
        const MIN_PROB: f64 = 1e-10;
        let p_e = p_e.max(MIN_PROB);

        // P(H|E) = P(E|H) * P(H) / P(E)
        let posterior = (likelihood_h * prior) / p_e;

        Self::new(posterior)
    }

    /// Maximum uncertainty confidence (0.5)
    pub fn max_uncertainty() -> Self {
        Self(0.5)
    }
}

impl TryFrom<f64> for Confidence {
    type Error = ConfidenceError;
    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl Default for Confidence {
    fn default() -> Self {
        Self::max_uncertainty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confidence_valid() {
        assert!(Confidence::new(0.0).is_ok());
        assert!(Confidence::new(0.5).is_ok());
        assert!(Confidence::new(1.0).is_ok());
    }

    #[test]
    fn test_confidence_rejects_nan() {
        assert!(matches!(
            Confidence::new(f64::NAN),
            Err(ConfidenceError::NaN)
        ));
    }

    #[test]
    fn test_confidence_rejects_out_of_bounds() {
        assert!(Confidence::new(-0.1).is_err());
        assert!(Confidence::new(1.1).is_err());
        assert!(Confidence::new(2.0).is_err());
    }

    #[test]
    fn test_confidence_get() {
        let c = Confidence::new(0.75).unwrap();
        assert_eq!(c.get(), 0.75);
    }

    #[test]
    fn test_bayes_update_supporting_evidence() {
        // Prior: 0.5 (max uncertainty)
        // Evidence: P(E|H) = 0.9, P(E|¬H) = 0.1
        // Expected: Posterior > 0.8
        let prior = Confidence::new(0.5).unwrap();
        let posterior = prior.update_with_evidence(0.9, 0.1).unwrap();
        assert!(posterior.get() > 0.8);
    }

    #[test]
    fn test_bayes_update_contradictory_evidence() {
        // Prior: 0.5
        // Evidence: P(E|H) = 0.1, P(E|¬H) = 0.9
        // Expected: Posterior < 0.2
        let prior = Confidence::new(0.5).unwrap();
        let posterior = prior.update_with_evidence(0.1, 0.9).unwrap();
        assert!(posterior.get() < 0.2);
    }

    #[test]
    fn test_bayes_update_neutral_evidence() {
        // Prior: 0.5
        // Evidence: P(E|H) = 0.5, P(E|¬H) = 0.5
        // Expected: Posterior ≈ 0.5 (no change)
        let prior = Confidence::new(0.5).unwrap();
        let posterior = prior.update_with_evidence(0.5, 0.5).unwrap();
        assert!((posterior.get() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_max_uncertainty() {
        let c = Confidence::max_uncertainty();
        assert_eq!(c.get(), 0.5);
    }

    #[test]
    fn test_default() {
        let c = Confidence::default();
        assert_eq!(c.get(), 0.5);
    }

    #[test]
    fn test_try_from_f64() {
        assert!(Confidence::try_from(0.5).is_ok());
        assert!(Confidence::try_from(f64::NAN).is_err());
        assert!(Confidence::try_from(1.5).is_err());
    }
}

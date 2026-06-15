//! Checkpoint validation logic.
//!
//! Extracted from `mod.rs` (SPLIT-20). Confidence-based validation gate that
//! decides whether a checkpoint passes, warns, or fails (with rollback).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::workflow::task::TaskId;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationStatus {
    Passed,
    Warning,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RollbackRecommendation {
    ToPreviousCheckpoint,
    SpecificTask(TaskId),
    FullRollback,
    None,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidationResult {
    pub confidence: f64,
    pub status: ValidationStatus,
    pub message: String,
    pub rollback_recommendation: Option<RollbackRecommendation>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct ValidationCheckpoint {
    pub min_confidence: f64,
    pub warning_threshold: f64,
    pub rollback_on_failure: bool,
}

impl Default for ValidationCheckpoint {
    fn default() -> Self {
        Self {
            min_confidence: 0.7,
            warning_threshold: 0.85,
            rollback_on_failure: true,
        }
    }
}

pub fn extract_confidence(result: &crate::workflow::task::TaskResult) -> f64 {
    match result {
        crate::workflow::task::TaskResult::Success => 1.0,
        crate::workflow::task::TaskResult::Skipped => 0.5,
        crate::workflow::task::TaskResult::Failed(_) => 0.0,
        crate::workflow::task::TaskResult::WithCompensation { result, .. } => {
            extract_confidence(result)
        }
    }
}

pub fn validate_checkpoint(
    task_result: &crate::workflow::task::TaskResult,
    config: &ValidationCheckpoint,
) -> ValidationResult {
    let confidence = extract_confidence(task_result);

    let status = if confidence >= config.warning_threshold {
        ValidationStatus::Passed
    } else if confidence >= config.min_confidence {
        ValidationStatus::Warning
    } else {
        ValidationStatus::Failed
    };

    let percentage = (confidence * 100.0) as u32;
    let message = format!("Confidence: {}% (status: {:?})", percentage, status);

    let rollback_recommendation =
        if matches!(status, ValidationStatus::Failed) && config.rollback_on_failure {
            Some(RollbackRecommendation::FullRollback)
        } else {
            None
        };

    ValidationResult {
        confidence,
        status,
        message,
        rollback_recommendation,
        timestamp: Utc::now(),
    }
}

pub fn can_proceed(validation: &ValidationResult) -> bool {
    !matches!(validation.status, ValidationStatus::Failed)
}

pub fn requires_rollback(validation: &ValidationResult) -> bool {
    matches!(validation.status, ValidationStatus::Failed)
        && validation.rollback_recommendation.is_some()
}

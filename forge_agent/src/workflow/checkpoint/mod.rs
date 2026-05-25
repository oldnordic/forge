mod service;

pub use service::{CheckpointSummary, WorkflowCheckpointService};

use crate::workflow::dag::Workflow;
use crate::workflow::executor::WorkflowExecutor;
use crate::workflow::task::TaskId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use uuid::Uuid;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CheckpointId(pub Uuid);

impl CheckpointId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for CheckpointId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for CheckpointId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowCheckpoint {
    pub id: CheckpointId,
    pub workflow_id: String,
    pub sequence: u64,
    pub timestamp: DateTime<Utc>,
    pub completed_tasks: Vec<TaskId>,
    pub failed_tasks: Vec<TaskId>,
    pub current_position: usize,
    pub total_tasks: usize,
    pub checksum: String,
    pub task_ids_checksum: String,
}

impl WorkflowCheckpoint {
    pub fn from_executor(
        workflow_id: impl Into<String>,
        sequence: u64,
        executor: &WorkflowExecutor,
        position: usize,
    ) -> Self {
        let completed = executor.completed_task_ids();
        let failed = executor.failed_task_ids();

        let task_ids = executor.task_ids();
        let task_ids_checksum = compute_task_ids_checksum(&task_ids);

        let mut checkpoint = Self {
            id: CheckpointId::new(),
            workflow_id: workflow_id.into(),
            sequence,
            timestamp: Utc::now(),
            completed_tasks: completed.clone(),
            failed_tasks: failed.clone(),
            current_position: position,
            total_tasks: executor.task_count(),
            checksum: String::new(),
            task_ids_checksum,
        };

        checkpoint.checksum = checkpoint.compute_checksum();
        checkpoint
    }

    fn compute_checksum(&self) -> String {
        let data_for_hash = CheckpointDataForHash {
            id: self.id,
            workflow_id: &self.workflow_id,
            sequence: self.sequence,
            timestamp: self.timestamp,
            completed_tasks: &self.completed_tasks,
            failed_tasks: &self.failed_tasks,
            current_position: self.current_position,
            total_tasks: self.total_tasks,
            task_ids_checksum: &self.task_ids_checksum,
        };

        let json = serde_json::to_vec(&data_for_hash).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(&json);
        format!("{:x}", hasher.finalize())
    }

    pub fn validate(&self) -> Result<(), crate::workflow::WorkflowError> {
        let expected = self.compute_checksum();
        if self.checksum != expected {
            return Err(crate::workflow::WorkflowError::CheckpointCorrupted(
                format!(
                    "Checksum mismatch: expected {}, got {}",
                    expected, self.checksum
                ),
            ));
        }
        Ok(())
    }
}

#[derive(Serialize)]
struct CheckpointDataForHash<'a> {
    id: CheckpointId,
    workflow_id: &'a str,
    sequence: u64,
    timestamp: DateTime<Utc>,
    completed_tasks: &'a [TaskId],
    failed_tasks: &'a [TaskId],
    current_position: usize,
    total_tasks: usize,
    task_ids_checksum: &'a str,
}

fn compute_task_ids_checksum(task_ids: &[TaskId]) -> String {
    let mut sorted_ids: Vec<&TaskId> = task_ids.iter().collect();
    sorted_ids.sort_by_key(|id| id.as_str());

    let json = serde_json::to_vec(&sorted_ids).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(&json);
    format!("{:x}", hasher.finalize())
}

pub fn validate_workflow_consistency(
    workflow: &Workflow,
    checkpoint: &WorkflowCheckpoint,
) -> Result<(), crate::workflow::WorkflowError> {
    if workflow.task_count() != checkpoint.total_tasks {
        return Err(crate::workflow::WorkflowError::WorkflowChanged(format!(
            "Task count mismatch: checkpoint has {} tasks, current workflow has {} tasks",
            checkpoint.total_tasks,
            workflow.task_count()
        )));
    }

    let workflow_task_ids: HashSet<_> = workflow.task_ids().into_iter().collect();

    for task_id in &checkpoint.completed_tasks {
        if !workflow_task_ids.contains(task_id) {
            return Err(crate::workflow::WorkflowError::WorkflowChanged(format!(
                "Completed task from checkpoint not found in workflow: {}",
                task_id
            )));
        }
    }

    for task_id in &checkpoint.failed_tasks {
        if !workflow_task_ids.contains(task_id) {
            return Err(crate::workflow::WorkflowError::WorkflowChanged(format!(
                "Failed task from checkpoint not found in workflow: {}",
                task_id
            )));
        }
    }

    if checkpoint.current_position >= checkpoint.total_tasks {
        return Err(crate::workflow::WorkflowError::CheckpointCorrupted(
            format!(
                "Invalid checkpoint position: {} exceeds total tasks {}",
                checkpoint.current_position, checkpoint.total_tasks
            ),
        ));
    }

    let current_task_ids = workflow.task_ids();
    let current_checksum = compute_task_ids_checksum(&current_task_ids);
    if current_checksum != checkpoint.task_ids_checksum {
        return Err(crate::workflow::WorkflowError::WorkflowChanged(format!(
            "Workflow structure changed: task IDs checksum mismatch. Expected: {}, Got: {}",
            checkpoint.task_ids_checksum, current_checksum
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests;

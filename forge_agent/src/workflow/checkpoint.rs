//! Workflow state checkpointing with integrity validation.
//!
//! Provides incremental state snapshots after each task completion,
//! enabling workflow recovery from failures. Uses bincode serialization
//! for fast snapshots and SHA-256 checksums for integrity validation.

use crate::workflow::executor::WorkflowExecutor;
use crate::workflow::task::TaskId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use uuid::Uuid;

/// Unique identifier for a workflow checkpoint.
///
/// Wrapper type for forge_reasoning::CheckpointId to maintain
/// namespace separation between workflow and debugging checkpoints.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CheckpointId(pub Uuid);

impl CheckpointId {
    /// Creates a new checkpoint ID.
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

/// Snapshot of workflow execution state at a point in time.
///
/// Stores completed tasks, failed tasks, current execution position,
/// and includes SHA-256 checksum for integrity validation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowCheckpoint {
    /// Unique checkpoint identifier
    pub id: CheckpointId,
    /// Workflow this checkpoint belongs to
    pub workflow_id: String,
    /// Checkpoint sequence number (monotonically increasing)
    pub sequence: u64,
    /// Timestamp when checkpoint was created
    pub timestamp: DateTime<Utc>,
    /// Tasks that have completed successfully
    pub completed_tasks: Vec<TaskId>,
    /// Tasks that have failed
    pub failed_tasks: Vec<TaskId>,
    /// Current position in execution order
    pub current_position: usize,
    /// Total number of tasks in workflow
    pub total_tasks: usize,
    /// SHA-256 checksum for integrity verification
    pub checksum: String,
}

impl WorkflowCheckpoint {
    /// Creates a checkpoint from current executor state.
    ///
    /// Captures the current execution state including completed tasks,
    /// failed tasks, and current position. Computes SHA-256 checksum
    /// for integrity validation.
    ///
    /// # Arguments
    ///
    /// * `workflow_id` - Workflow identifier
    /// * `sequence` - Checkpoint sequence number
    /// * `executor` - Reference to workflow executor
    /// * `position` - Current position in execution order
    pub fn from_executor(
        workflow_id: impl Into<String>,
        sequence: u64,
        executor: &WorkflowExecutor,
        position: usize,
    ) -> Self {
        let completed = executor.completed_task_ids();
        let failed = executor.failed_task_ids();

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
        };

        // Compute checksum for integrity validation
        checkpoint.checksum = checkpoint.compute_checksum();
        checkpoint
    }

    /// Computes SHA-256 checksum of checkpoint data.
    ///
    /// Serializes checkpoint data (excluding checksum field) and
    /// computes SHA-256 hash for integrity validation.
    fn compute_checksum(&self) -> String {
        // Create a copy without checksum for serialization
        let data_for_hash = CheckpointDataForHash {
            id: self.id,
            workflow_id: &self.workflow_id,
            sequence: self.sequence,
            timestamp: self.timestamp,
            completed_tasks: &self.completed_tasks,
            failed_tasks: &self.failed_tasks,
            current_position: self.current_position,
            total_tasks: self.total_tasks,
        };

        let json = serde_json::to_vec(&data_for_hash).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(&json);
        format!("{:x}", hasher.finalize())
    }

    /// Validates the checkpoint's checksum.
    ///
    /// Verifies that the stored checksum matches the computed checksum
    /// of the checkpoint data. Returns an error if checksums don't match.
    ///
    /// # Returns
    ///
    /// - `Ok(())` if checksum is valid
    /// - `Err(WorkflowError)` if checksum mismatch detected
    pub fn validate(&self) -> Result<(), crate::workflow::WorkflowError> {
        let expected = self.compute_checksum();
        if self.checksum != expected {
            return Err(crate::workflow::WorkflowError::CheckpointCorrupted(
                format!("Checksum mismatch: expected {}, got {}", expected, self.checksum),
            ));
        }
        Ok(())
    }
}

/// Helper struct for computing checksum (excludes checksum field).
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
}

/// Summary of a checkpoint (for listing).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckpointSummary {
    /// Checkpoint identifier
    pub id: CheckpointId,
    /// Checkpoint sequence number
    pub sequence: u64,
    /// Timestamp when checkpoint was created
    pub timestamp: DateTime<Utc>,
    /// Number of completed tasks at checkpoint time
    pub completed_count: usize,
    /// Current execution position
    pub current_position: usize,
    /// Total number of tasks
    pub total_tasks: usize,
}

impl CheckpointSummary {
    /// Creates a checkpoint summary from a workflow checkpoint.
    pub fn from_checkpoint(checkpoint: &WorkflowCheckpoint) -> Self {
        Self {
            id: checkpoint.id,
            sequence: checkpoint.sequence,
            timestamp: checkpoint.timestamp,
            completed_count: checkpoint.completed_tasks.len(),
            current_position: checkpoint.current_position,
            total_tasks: checkpoint.total_tasks,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::dag::Workflow;
    use crate::workflow::task::{TaskContext, TaskError, TaskResult, WorkflowTask};
    use async_trait::async_trait;

    // Mock task for testing
    struct MockTask {
        id: TaskId,
        name: String,
    }

    impl MockTask {
        fn new(id: impl Into<TaskId>, name: &str) -> Self {
            Self {
                id: id.into(),
                name: name.to_string(),
            }
        }
    }

    #[async_trait]
    impl WorkflowTask for MockTask {
        async fn execute(&self, _context: &TaskContext) -> Result<TaskResult, TaskError> {
            Ok(TaskResult::Success)
        }

        fn id(&self) -> TaskId {
            self.id.clone()
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn dependencies(&self) -> Vec<TaskId> {
            Vec::new()
        }
    }

    #[test]
    fn test_checkpoint_id_generation() {
        let id1 = CheckpointId::new();
        let id2 = CheckpointId::new();

        // Each ID should be unique
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_checkpoint_id_display() {
        let id = CheckpointId::new();
        let display = format!("{}", id);

        // Should format as UUID
        assert!(display.len() > 0);
    }

    #[test]
    fn test_checkpoint_from_executor() {
        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));
        workflow.add_task(Box::new(MockTask::new("task-2", "Task 2")));
        workflow.add_task(Box::new(MockTask::new("task-3", "Task 3")));

        let executor = WorkflowExecutor::new(workflow);

        let checkpoint = WorkflowCheckpoint::from_executor("workflow-1", 0, &executor, 0);

        assert_eq!(checkpoint.workflow_id, "workflow-1");
        assert_eq!(checkpoint.sequence, 0);
        assert_eq!(checkpoint.current_position, 0);
        assert_eq!(checkpoint.total_tasks, 3);
        assert_eq!(checkpoint.completed_tasks.len(), 0);
        assert_eq!(checkpoint.failed_tasks.len(), 0);
        assert!(!checkpoint.checksum.is_empty());
    }

    #[test]
    fn test_checkpoint_checksum_computation() {
        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));

        let executor = WorkflowExecutor::new(workflow);

        let checkpoint = WorkflowCheckpoint::from_executor("workflow-1", 0, &executor, 0);

        // Checksum should be non-empty and valid hex
        assert!(!checkpoint.checksum.is_empty());
        assert!(checkpoint.checksum.len() == 64); // SHA-256 produces 64 hex characters
        assert!(checkpoint.checksum.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_checkpoint_validation() {
        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));

        let executor = WorkflowExecutor::new(workflow);

        let checkpoint = WorkflowCheckpoint::from_executor("workflow-1", 0, &executor, 0);

        // Fresh checkpoint should validate
        assert!(checkpoint.validate().is_ok());
    }

    #[test]
    fn test_checkpoint_validation_fails_on_corruption() {
        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));

        let executor = WorkflowExecutor::new(workflow);

        let mut checkpoint = WorkflowCheckpoint::from_executor("workflow-1", 0, &executor, 0);

        // Corrupt the checksum
        checkpoint.checksum = "corrupted".to_string();

        // Validation should fail
        assert!(checkpoint.validate().is_err());
    }

    #[test]
    fn test_checkpoint_serialization() {
        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));

        let executor = WorkflowExecutor::new(workflow);

        let checkpoint = WorkflowCheckpoint::from_executor("workflow-1", 0, &executor, 0);

        // Serialize with JSON
        let serialized = serde_json::to_string(&checkpoint);
        assert!(serialized.is_ok());

        // Deserialize back
        let deserialized: Result<WorkflowCheckpoint, _> = serde_json::from_str(&serialized.unwrap());
        assert!(deserialized.is_ok());

        let restored = deserialized.unwrap();
        assert_eq!(restored.id, checkpoint.id);
        assert_eq!(restored.workflow_id, checkpoint.workflow_id);
        assert_eq!(restored.sequence, checkpoint.sequence);
        assert_eq!(restored.checksum, checkpoint.checksum);
    }

    #[test]
    fn test_checkpoint_summary_from_checkpoint() {
        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));
        workflow.add_task(Box::new(MockTask::new("task-2", "Task 2")));

        let executor = WorkflowExecutor::new(workflow);

        let checkpoint = WorkflowCheckpoint::from_executor("workflow-1", 0, &executor, 0);

        let summary = CheckpointSummary::from_checkpoint(&checkpoint);

        assert_eq!(summary.id, checkpoint.id);
        assert_eq!(summary.sequence, checkpoint.sequence);
        assert_eq!(summary.completed_count, 0);
        assert_eq!(summary.current_position, 0);
        assert_eq!(summary.total_tasks, 2);
    }
}

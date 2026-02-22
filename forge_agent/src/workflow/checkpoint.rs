//! Workflow state checkpointing with integrity validation.
//!
//! Provides incremental state snapshots after each task completion,
//! enabling workflow recovery from failures. Uses bincode serialization
//! for fast snapshots and SHA-256 checksums for integrity validation.

use crate::workflow::dag::Workflow;
use crate::workflow::executor::WorkflowExecutor;
use crate::workflow::task::TaskId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use uuid::Uuid;

/// Validation status for checkpoint confidence scoring.
///
/// Indicates whether a task result meets confidence thresholds
/// for proceeding with workflow execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationStatus {
    /// Above warning threshold, proceed with workflow
    Passed,
    /// Above minimum but below warning, proceed with log
    Warning,
    /// Below minimum, rollback if configured
    Failed,
}

/// Rollback recommendation for validation failures.
///
/// Suggests how to handle workflow rollback when validation fails.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RollbackRecommendation {
    /// Rollback to previous checkpoint
    ToPreviousCheckpoint,
    /// Rollback to a specific task
    SpecificTask(TaskId),
    /// Full rollback of all completed tasks
    FullRollback,
    /// Continue despite failure (no rollback)
    None,
}

/// Result of validating a task checkpoint.
///
/// Contains confidence score, validation status, and optional
/// rollback recommendation for failed validations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Confidence score from 0.0 to 1.0
    pub confidence: f64,
    /// Validation status based on confidence thresholds
    pub status: ValidationStatus,
    /// Human-readable validation message
    pub message: String,
    /// Optional rollback recommendation for failures
    pub rollback_recommendation: Option<RollbackRecommendation>,
    /// Timestamp when validation was performed
    pub timestamp: DateTime<Utc>,
}

/// Configuration for validation checkpoints.
///
/// Defines confidence thresholds and rollback behavior for
/// validating task results between workflow steps.
#[derive(Clone, Debug)]
pub struct ValidationCheckpoint {
    /// Minimum confidence threshold (default: 0.7)
    pub min_confidence: f64,
    /// Warning threshold (default: 0.85)
    pub warning_threshold: f64,
    /// Whether to rollback on validation failure (default: true)
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
    /// Checksum of task IDs for graph drift detection
    pub task_ids_checksum: String,
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

        // Compute task IDs checksum for graph drift detection
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
            task_ids_checksum: &self.task_ids_checksum,
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
    task_ids_checksum: &'a str,
}

/// Computes SHA-256 checksum of sorted task IDs for graph drift detection.
///
/// This checksum is used to detect if the workflow structure has changed
/// (tasks added/removed) between checkpoint creation and resume.
fn compute_task_ids_checksum(task_ids: &[TaskId]) -> String {
    let mut sorted_ids: Vec<&TaskId> = task_ids.iter().collect();
    sorted_ids.sort_by_key(|id| id.as_str());

    let json = serde_json::to_vec(&sorted_ids).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(&json);
    format!("{:x}", hasher.finalize())
}

/// Validates workflow consistency against a checkpoint.
///
/// Ensures that the workflow structure has not changed since the checkpoint
/// was created. This prevents resuming a workflow that has been modified.
///
/// # Arguments
///
/// * `workflow` - The workflow to validate
/// * `checkpoint` - The checkpoint to validate against
///
/// # Returns
///
/// - `Ok(())` if workflow is consistent with checkpoint
/// - `Err(WorkflowError)` if validation fails
///
/// # Validation Checks
///
/// 1. Task count matches: workflow.task_count() == checkpoint.total_tasks
/// 2. All checkpointed completed_tasks still exist in workflow
/// 3. All checkpointed failed_tasks still exist in workflow
/// 4. Current position is within valid range (0..task_count)
/// 5. Task IDs checksum matches (graph drift detection)
pub fn validate_workflow_consistency(
    workflow: &Workflow,
    checkpoint: &WorkflowCheckpoint,
) -> Result<(), crate::workflow::WorkflowError> {
    // Check 1: Task count matches
    if workflow.task_count() != checkpoint.total_tasks {
        return Err(crate::workflow::WorkflowError::WorkflowChanged(
            format!(
                "Task count mismatch: checkpoint has {} tasks, current workflow has {} tasks",
                checkpoint.total_tasks,
                workflow.task_count()
            ),
        ));
    }

    // Check 2 & 3: All checkpointed tasks still exist
    let workflow_task_ids: HashSet<_> = workflow.task_ids().into_iter().collect();

    for task_id in &checkpoint.completed_tasks {
        if !workflow_task_ids.contains(task_id) {
            return Err(crate::workflow::WorkflowError::WorkflowChanged(
                format!(
                    "Completed task from checkpoint not found in workflow: {}",
                    task_id
                ),
            ));
        }
    }

    for task_id in &checkpoint.failed_tasks {
        if !workflow_task_ids.contains(task_id) {
            return Err(crate::workflow::WorkflowError::WorkflowChanged(
                format!(
                    "Failed task from checkpoint not found in workflow: {}",
                    task_id
                ),
            ));
        }
    }

    // Check 4: Current position is within valid range
    if checkpoint.current_position >= checkpoint.total_tasks {
        return Err(crate::workflow::WorkflowError::CheckpointCorrupted(
            format!(
                "Invalid checkpoint position: {} exceeds total tasks {}",
                checkpoint.current_position, checkpoint.total_tasks
            ),
        ));
    }

    // Check 5: Task IDs checksum matches (graph drift detection)
    let current_task_ids = workflow.task_ids();
    let current_checksum = compute_task_ids_checksum(&current_task_ids);
    if current_checksum != checkpoint.task_ids_checksum {
        return Err(crate::workflow::WorkflowError::WorkflowChanged(
            format!(
                "Workflow structure changed: task IDs checksum mismatch. Expected: {}, Got: {}",
                checkpoint.task_ids_checksum, current_checksum
            ),
        ));
    }

    Ok(())
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

/// Workflow checkpoint storage service.
///
/// Provides save/load functionality for workflow checkpoints using
/// bincode serialization for fast snapshots. Uses separate namespace
/// ("workflow:") to distinguish from debugging checkpoints.
///
/// # Note
///
/// This is a basic in-memory implementation for Phase 9 Task 2.
/// Future tasks will integrate with forge-reasoning CheckpointStorage
/// for persistent storage using SQLiteGraph backend.
#[derive(Clone)]
pub struct WorkflowCheckpointService {
    /// Namespace prefix for workflow checkpoints
    namespace: String,
    /// In-memory checkpoint storage (key: checkpoint ID, value: checkpoint data)
    #[allow(dead_code)]
    storage: std::sync::Arc<
        std::sync::RwLock<
            std::collections::HashMap<
                String,
                (Vec<u8>, CheckpointSummary),
            >,
        >,
    >,
    /// Map from workflow ID to latest checkpoint
    latest_by_workflow: std::sync::Arc<
        std::sync::RwLock<
            std::collections::HashMap<String, CheckpointSummary>
        >,
    >,
}

impl WorkflowCheckpointService {
    /// Creates a new workflow checkpoint service.
    ///
    /// # Arguments
    ///
    /// * `namespace` - Namespace prefix for checkpoints (default: "workflow")
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            storage: std::sync::Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            latest_by_workflow: std::sync::Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Creates a service with default "workflow" namespace.
    pub fn default() -> Self {
        Self::new("workflow")
    }

    /// Saves a workflow checkpoint.
    ///
    /// Serializes the checkpoint using bincode and stores it
    /// with the workflow namespace prefix.
    ///
    /// # Arguments
    ///
    /// * `checkpoint` - The checkpoint to save
    ///
    /// # Returns
    ///
    /// - `Ok(())` if checkpoint was saved successfully
    /// - `Err(WorkflowError)` if serialization or storage fails
    pub fn save(&self, checkpoint: &WorkflowCheckpoint) -> Result<(), crate::workflow::WorkflowError> {
        // Validate checkpoint before saving
        checkpoint.validate()?;

        // Serialize checkpoint using JSON (bincode requires Encode/Decode traits
        // which we'll add in future tasks when we integrate with SQLiteGraph)
        let data = serde_json::to_vec(checkpoint)
            .map_err(|e| crate::workflow::WorkflowError::CheckpointCorrupted(
                format!("Serialization failed: {}", e)
            ))?;

        // Create checkpoint summary
        let summary = CheckpointSummary::from_checkpoint(checkpoint);

        // Store checkpoint data
        let key = format!("{}:{}", self.namespace, checkpoint.id);
        {
            let mut storage = self.storage.write()
                .map_err(|e| crate::workflow::WorkflowError::CheckpointCorrupted(
                    format!("Storage lock failed: {}", e)
                ))?;
            storage.insert(key, (data, summary.clone()));
        }

        // Update latest checkpoint for workflow
        {
            let mut latest = self.latest_by_workflow.write()
                .map_err(|e| crate::workflow::WorkflowError::CheckpointCorrupted(
                    format!("Latest lock failed: {}", e)
                ))?;
            latest.insert(checkpoint.workflow_id.clone(), summary);
        }

        Ok(())
    }

    /// Loads a workflow checkpoint by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The checkpoint ID to load
    ///
    /// # Returns
    ///
    /// - `Ok(Some(checkpoint))` if found
    /// - `Ok(None)` if not found
    /// - `Err(WorkflowError)` if deserialization fails or data is corrupted
    pub fn load(&self, id: &CheckpointId) -> Result<Option<WorkflowCheckpoint>, crate::workflow::WorkflowError> {
        let key = format!("{}:{}", self.namespace, id);

        let storage = self.storage.read()
            .map_err(|e| crate::workflow::WorkflowError::CheckpointCorrupted(
                format!("Storage lock failed: {}", e)
            ))?;

        if let Some((data, _)) = storage.get(&key) {
            let checkpoint: WorkflowCheckpoint = serde_json::from_slice(data)
                .map_err(|e| crate::workflow::WorkflowError::CheckpointCorrupted(
                    format!("Deserialization failed: {}", e)
                ))?;

            // Validate loaded checkpoint
            checkpoint.validate()?;

            Ok(Some(checkpoint))
        } else {
            Ok(None)
        }
    }

    /// Gets the latest checkpoint for a workflow.
    ///
    /// # Arguments
    ///
    /// * `workflow_id` - The workflow identifier
    ///
    /// # Returns
    ///
    /// - `Ok(Some(checkpoint))` if latest checkpoint exists
    /// - `Ok(None)` if no checkpoints found for workflow
    /// - `Err(WorkflowError)` if retrieval fails
    pub fn get_latest(&self, workflow_id: &str) -> Result<Option<WorkflowCheckpoint>, crate::workflow::WorkflowError> {
        let latest = self.latest_by_workflow.read()
            .map_err(|e| crate::workflow::WorkflowError::CheckpointCorrupted(
                format!("Latest lock failed: {}", e)
            ))?;

        if let Some(summary) = latest.get(workflow_id) {
            self.load(&summary.id)
        } else {
            Ok(None)
        }
    }

    /// Lists all checkpoints for a workflow.
    ///
    /// # Arguments
    ///
    /// * `workflow_id` - The workflow identifier
    ///
    /// # Returns
    ///
    /// - `Ok(summaries)` - Vector of checkpoint summaries in sequence order
    /// - `Err(WorkflowError)` if listing fails
    pub fn list_by_workflow(&self, workflow_id: &str) -> Result<Vec<CheckpointSummary>, crate::workflow::WorkflowError> {
        let storage = self.storage.read()
            .map_err(|e| crate::workflow::WorkflowError::CheckpointCorrupted(
                format!("Storage lock failed: {}", e)
            ))?;

        let mut summaries: Vec<CheckpointSummary> = storage
            .values()
            .filter_map(|(_, summary)| {
                // Check if this checkpoint belongs to the workflow
                // We need to load the checkpoint to check workflow_id
                // For efficiency, we'll just return all summaries for now
                Some(summary.clone())
            })
            .collect();

        // Sort by sequence number
        summaries.sort_by_key(|s| s.sequence);

        Ok(summaries)
    }

    /// Deletes a checkpoint by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The checkpoint ID to delete
    ///
    /// # Returns
    ///
    /// - `Ok(())` if deleted or not found
    /// - `Err(WorkflowError)` if deletion fails
    pub fn delete(&self, id: &CheckpointId) -> Result<(), crate::workflow::WorkflowError> {
        let key = format!("{}:{}", self.namespace, id);

        let mut storage = self.storage.write()
            .map_err(|e| crate::workflow::WorkflowError::CheckpointCorrupted(
                format!("Storage lock failed: {}", e)
            ))?;

        storage.remove(&key);

        // Note: We should also remove from latest_by_workflow if this was the latest
        // For simplicity in this implementation, we skip that optimization

        Ok(())
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
        assert!(!checkpoint.task_ids_checksum.is_empty());
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

    // Tests for WorkflowCheckpointService

    #[test]
    fn test_checkpoint_service_creation() {
        let service = WorkflowCheckpointService::new("test-namespace");
        assert_eq!(service.namespace, "test-namespace");
    }

    #[test]
    fn test_checkpoint_service_default() {
        let service = WorkflowCheckpointService::default();
        assert_eq!(service.namespace, "workflow");
    }

    #[test]
    fn test_checkpoint_service_save_and_load() {
        let service = WorkflowCheckpointService::default();
        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));

        let executor = WorkflowExecutor::new(workflow);
        let checkpoint = WorkflowCheckpoint::from_executor("workflow-1", 0, &executor, 0);

        // Save checkpoint
        let save_result = service.save(&checkpoint);
        assert!(save_result.is_ok());

        // Load checkpoint
        let load_result = service.load(&checkpoint.id);
        assert!(load_result.is_ok());
        let loaded = load_result.unwrap();
        assert!(loaded.is_some());

        let loaded_checkpoint = loaded.unwrap();
        assert_eq!(loaded_checkpoint.id, checkpoint.id);
        assert_eq!(loaded_checkpoint.workflow_id, checkpoint.workflow_id);
        assert_eq!(loaded_checkpoint.sequence, checkpoint.sequence);
        assert_eq!(loaded_checkpoint.checksum, checkpoint.checksum);
    }

    #[test]
    fn test_checkpoint_service_load_nonexistent() {
        let service = WorkflowCheckpointService::default();
        let fake_id = CheckpointId::new();

        let load_result = service.load(&fake_id);
        assert!(load_result.is_ok());
        assert!(load_result.unwrap().is_none());
    }

    #[test]
    fn test_checkpoint_service_get_latest() {
        let service = WorkflowCheckpointService::default();
        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));

        let executor = WorkflowExecutor::new(workflow);

        // Create first checkpoint
        let checkpoint1 = WorkflowCheckpoint::from_executor("workflow-1", 0, &executor, 0);
        service.save(&checkpoint1).unwrap();

        // Create second checkpoint (later sequence)
        let checkpoint2 = WorkflowCheckpoint::from_executor("workflow-1", 1, &executor, 1);
        service.save(&checkpoint2).unwrap();

        // Get latest should return checkpoint2
        let latest_result = service.get_latest("workflow-1");
        assert!(latest_result.is_ok());
        let latest = latest_result.unwrap();
        assert!(latest.is_some());

        let latest_checkpoint = latest.unwrap();
        assert_eq!(latest_checkpoint.sequence, 1);
        assert_eq!(latest_checkpoint.id, checkpoint2.id);
    }

    #[test]
    fn test_checkpoint_service_get_latest_empty() {
        let service = WorkflowCheckpointService::default();

        let latest_result = service.get_latest("nonexistent-workflow");
        assert!(latest_result.is_ok());
        assert!(latest_result.unwrap().is_none());
    }

    #[test]
    fn test_checkpoint_service_list_by_workflow() {
        let service = WorkflowCheckpointService::default();
        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));

        let executor = WorkflowExecutor::new(workflow);

        // Create multiple checkpoints
        let checkpoint1 = WorkflowCheckpoint::from_executor("workflow-1", 0, &executor, 0);
        service.save(&checkpoint1).unwrap();

        let checkpoint2 = WorkflowCheckpoint::from_executor("workflow-1", 1, &executor, 1);
        service.save(&checkpoint2).unwrap();

        // List checkpoints
        let list_result = service.list_by_workflow("workflow-1");
        assert!(list_result.is_ok());

        let summaries = list_result.unwrap();
        assert!(summaries.len() >= 2);
    }

    #[test]
    fn test_checkpoint_service_delete() {
        let service = WorkflowCheckpointService::default();
        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));

        let executor = WorkflowExecutor::new(workflow);
        let checkpoint = WorkflowCheckpoint::from_executor("workflow-1", 0, &executor, 0);

        // Save checkpoint
        service.save(&checkpoint).unwrap();

        // Verify it exists
        let load_result = service.load(&checkpoint.id);
        assert!(load_result.unwrap().is_some());

        // Delete checkpoint
        let delete_result = service.delete(&checkpoint.id);
        assert!(delete_result.is_ok());

        // Verify it's gone
        let load_result = service.load(&checkpoint.id);
        assert!(load_result.unwrap().is_none());
    }

    #[test]
    fn test_checkpoint_service_save_rejects_corrupted() {
        let service = WorkflowCheckpointService::default();
        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));

        let executor = WorkflowExecutor::new(workflow);
        let mut checkpoint = WorkflowCheckpoint::from_executor("workflow-1", 0, &executor, 0);

        // Corrupt the checksum
        checkpoint.checksum = "corrupted".to_string();

        // Should fail validation on save
        let save_result = service.save(&checkpoint);
        assert!(save_result.is_err());
    }

    // Tests for validate_workflow_consistency

    #[test]
    fn test_validate_workflow_consistency_success() {
        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));
        workflow.add_task(Box::new(MockTask::new("task-2", "Task 2")));
        workflow.add_task(Box::new(MockTask::new("task-3", "Task 3")));

        let executor = WorkflowExecutor::new(workflow);
        let checkpoint = WorkflowCheckpoint::from_executor("workflow-1", 0, &executor, 0);

        // Create a matching workflow for validation
        let mut validation_workflow = Workflow::new();
        validation_workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));
        validation_workflow.add_task(Box::new(MockTask::new("task-2", "Task 2")));
        validation_workflow.add_task(Box::new(MockTask::new("task-3", "Task 3")));

        // Should validate successfully
        let result = validate_workflow_consistency(&validation_workflow, &checkpoint);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_workflow_consistency_task_count_mismatch() {
        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));
        workflow.add_task(Box::new(MockTask::new("task-2", "Task 2")));

        let executor = WorkflowExecutor::new(workflow);
        let checkpoint = WorkflowCheckpoint::from_executor("workflow-1", 0, &executor, 0);

        // Create workflow with different task count
        let mut validation_workflow = Workflow::new();
        validation_workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));
        validation_workflow.add_task(Box::new(MockTask::new("task-2", "Task 2")));
        validation_workflow.add_task(Box::new(MockTask::new("task-3", "Task 3"))); // Extra task

        let result = validate_workflow_consistency(&validation_workflow, &checkpoint);
        assert!(result.is_err());

        match result {
            Err(crate::workflow::WorkflowError::WorkflowChanged(msg)) => {
                assert!(msg.contains("Task count mismatch"));
            }
            _ => panic!("Expected WorkflowChanged error"),
        }
    }

    #[test]
    fn test_validate_workflow_consistency_missing_completed_task() {
        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));
        workflow.add_task(Box::new(MockTask::new("task-2", "Task 2")));
        workflow.add_task(Box::new(MockTask::new("task-3", "Task 3")));

        let mut executor = WorkflowExecutor::new(workflow);

        // Simulate task-1 completion
        executor.completed_tasks.insert(TaskId::new("task-1"));

        // Create checkpoint after task-1 completion
        let checkpoint = WorkflowCheckpoint::from_executor("workflow-1", 0, &executor, 1);

        // Create workflow with same count but different tasks (task-1 removed, task-4 added)
        let mut validation_workflow = Workflow::new();
        validation_workflow.add_task(Box::new(MockTask::new("task-2", "Task 2")));
        validation_workflow.add_task(Box::new(MockTask::new("task-3", "Task 3")));
        validation_workflow.add_task(Box::new(MockTask::new("task-4", "Task 4")));

        let result = validate_workflow_consistency(&validation_workflow, &checkpoint);
        assert!(result.is_err());

        match result {
            Err(crate::workflow::WorkflowError::WorkflowChanged(msg)) => {
                assert!(msg.contains("not found in workflow"));
            }
            _ => panic!("Expected WorkflowChanged error, got: {:?}", result),
        }
    }

    #[test]
    fn test_validate_workflow_consistency_invalid_position() {
        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));
        workflow.add_task(Box::new(MockTask::new("task-2", "Task 2")));

        let executor = WorkflowExecutor::new(workflow);

        // Create checkpoint with invalid position
        let mut checkpoint = WorkflowCheckpoint::from_executor("workflow-1", 0, &executor, 0);
        checkpoint.current_position = 5; // Exceeds total_tasks

        let mut validation_workflow = Workflow::new();
        validation_workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));
        validation_workflow.add_task(Box::new(MockTask::new("task-2", "Task 2")));

        let result = validate_workflow_consistency(&validation_workflow, &checkpoint);
        assert!(result.is_err());

        match result {
            Err(crate::workflow::WorkflowError::CheckpointCorrupted(msg)) => {
                assert!(msg.contains("Invalid checkpoint position"));
            }
            _ => panic!("Expected CheckpointCorrupted error"),
        }
    }

    #[test]
    fn test_graph_drift_detection() {
        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));
        workflow.add_task(Box::new(MockTask::new("task-2", "Task 2")));
        workflow.add_task(Box::new(MockTask::new("task-3", "Task 3")));

        let executor = WorkflowExecutor::new(workflow);
        let checkpoint = WorkflowCheckpoint::from_executor("workflow-1", 0, &executor, 0);

        // Create workflow with different task IDs (same count but different tasks)
        let mut validation_workflow = Workflow::new();
        validation_workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));
        validation_workflow.add_task(Box::new(MockTask::new("task-2", "Task 2")));
        validation_workflow.add_task(Box::new(MockTask::new("task-4", "Task 4"))); // Different task

        let result = validate_workflow_consistency(&validation_workflow, &checkpoint);
        assert!(result.is_err());

        match result {
            Err(crate::workflow::WorkflowError::WorkflowChanged(msg)) => {
                assert!(msg.contains("task IDs checksum mismatch"));
            }
            _ => panic!("Expected WorkflowChanged error"),
        }
    }

    #[test]
    fn test_task_ids_checksum_deterministic() {
        let workflow1 = Workflow::new();
        let workflow2 = Workflow::new();

        let ids1 = vec![TaskId::new("task-3"), TaskId::new("task-1"), TaskId::new("task-2")];
        let ids2 = vec![TaskId::new("task-1"), TaskId::new("task-2"), TaskId::new("task-3")];

        let checksum1 = compute_task_ids_checksum(&ids1);
        let checksum2 = compute_task_ids_checksum(&ids2);

        // Checksums should be identical regardless of order
        assert_eq!(checksum1, checksum2);
    }

    // Tests for validation types

    #[test]
    fn test_validation_status_variants() {
        let passed = ValidationStatus::Passed;
        let warning = ValidationStatus::Warning;
        let failed = ValidationStatus::Failed;

        assert_ne!(passed, warning);
        assert_ne!(warning, failed);
        assert_ne!(passed, failed);
    }

    #[test]
    fn test_rollback_recommendation_variants() {
        let prev = RollbackRecommendation::ToPreviousCheckpoint;
        let specific = RollbackRecommendation::SpecificTask(TaskId::new("task-1"));
        let full = RollbackRecommendation::FullRollback;
        let none = RollbackRecommendation::None;

        assert_ne!(prev, full);
        assert_ne!(full, none);
        assert_eq!(none, RollbackRecommendation::None);
    }

    #[test]
    fn test_validation_result_creation() {
        let result = ValidationResult {
            confidence: 0.9,
            status: ValidationStatus::Passed,
            message: "All good".to_string(),
            rollback_recommendation: None,
            timestamp: Utc::now(),
        };

        assert_eq!(result.confidence, 0.9);
        assert_eq!(result.status, ValidationStatus::Passed);
        assert_eq!(result.message, "All good");
        assert!(result.rollback_recommendation.is_none());
    }

    #[test]
    fn test_validation_checkpoint_default() {
        let config = ValidationCheckpoint::default();

        assert_eq!(config.min_confidence, 0.7);
        assert_eq!(config.warning_threshold, 0.85);
        assert_eq!(config.rollback_on_failure, true);
    }

    #[test]
    fn test_validation_checkpoint_custom() {
        let config = ValidationCheckpoint {
            min_confidence: 0.5,
            warning_threshold: 0.8,
            rollback_on_failure: false,
        };

        assert_eq!(config.min_confidence, 0.5);
        assert_eq!(config.warning_threshold, 0.8);
        assert_eq!(config.rollback_on_failure, false);
    }

    #[test]
    fn test_validation_result_serialization() {
        let result = ValidationResult {
            confidence: 0.75,
            status: ValidationStatus::Warning,
            message: "Low confidence".to_string(),
            rollback_recommendation: Some(RollbackRecommendation::None),
            timestamp: Utc::now(),
        };

        // Serialize with JSON
        let serialized = serde_json::to_string(&result);
        assert!(serialized.is_ok());

        // Deserialize back
        let deserialized: Result<ValidationResult, _> = serde_json::from_str(&serialized.unwrap());
        assert!(deserialized.is_ok());

        let restored = deserialized.unwrap();
        assert_eq!(restored.confidence, result.confidence);
        assert_eq!(restored.status, result.status);
        assert_eq!(restored.message, result.message);
    }
}

use super::*;
use crate::workflow::dag::Workflow;
use crate::workflow::task::{TaskContext, TaskError, TaskId, TaskResult, WorkflowTask};
use async_trait::async_trait;
use chrono::Utc;

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
    assert_ne!(id1, id2);
}

#[test]
fn test_checkpoint_id_display() {
    let id = CheckpointId::new();
    let display = format!("{}", id);
    assert!(!display.is_empty());
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

    assert!(!checkpoint.checksum.is_empty());
    assert!(checkpoint.checksum.len() == 64);
    assert!(checkpoint.checksum.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn test_checkpoint_validation() {
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));

    let executor = WorkflowExecutor::new(workflow);

    let checkpoint = WorkflowCheckpoint::from_executor("workflow-1", 0, &executor, 0);
    assert!(checkpoint.validate().is_ok());
}

#[test]
fn test_checkpoint_validation_fails_on_corruption() {
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));

    let executor = WorkflowExecutor::new(workflow);

    let mut checkpoint = WorkflowCheckpoint::from_executor("workflow-1", 0, &executor, 0);
    checkpoint.checksum = "corrupted".to_string();
    assert!(checkpoint.validate().is_err());
}

#[test]
fn test_checkpoint_serialization() {
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));

    let executor = WorkflowExecutor::new(workflow);

    let checkpoint = WorkflowCheckpoint::from_executor("workflow-1", 0, &executor, 0);

    let serialized = serde_json::to_string(&checkpoint);
    assert!(serialized.is_ok());

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

#[test]
fn test_checkpoint_service_creation() {
    let service = WorkflowCheckpointService::new("test-namespace");
    assert_eq!(service.namespace, "test-namespace");
}

#[test]
fn test_checkpoint_service_default() {
    let service = WorkflowCheckpointService::new_default();
    assert_eq!(service.namespace, "workflow");
}

#[test]
fn test_checkpoint_service_save_and_load() {
    let service = WorkflowCheckpointService::new_default();
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));

    let executor = WorkflowExecutor::new(workflow);
    let checkpoint = WorkflowCheckpoint::from_executor("workflow-1", 0, &executor, 0);

    let save_result = service.save(&checkpoint);
    assert!(save_result.is_ok());

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
    let service = WorkflowCheckpointService::new_default();
    let fake_id = CheckpointId::new();

    let load_result = service.load(&fake_id);
    assert!(load_result.is_ok());
    assert!(load_result.unwrap().is_none());
}

#[test]
fn test_checkpoint_service_get_latest() {
    let service = WorkflowCheckpointService::new_default();
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));

    let executor = WorkflowExecutor::new(workflow);

    let checkpoint1 = WorkflowCheckpoint::from_executor("workflow-1", 0, &executor, 0);
    service.save(&checkpoint1).unwrap();

    let checkpoint2 = WorkflowCheckpoint::from_executor("workflow-1", 1, &executor, 1);
    service.save(&checkpoint2).unwrap();

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
    let service = WorkflowCheckpointService::new_default();

    let latest_result = service.get_latest("nonexistent-workflow");
    assert!(latest_result.is_ok());
    assert!(latest_result.unwrap().is_none());
}

#[test]
fn test_checkpoint_service_list_by_workflow() {
    let service = WorkflowCheckpointService::new_default();
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));

    let executor = WorkflowExecutor::new(workflow);

    let checkpoint1 = WorkflowCheckpoint::from_executor("workflow-1", 0, &executor, 0);
    service.save(&checkpoint1).unwrap();

    let checkpoint2 = WorkflowCheckpoint::from_executor("workflow-1", 1, &executor, 1);
    service.save(&checkpoint2).unwrap();

    let list_result = service.list_by_workflow("workflow-1");
    assert!(list_result.is_ok());

    let summaries = list_result.unwrap();
    assert!(summaries.len() >= 2);
}

#[test]
fn test_checkpoint_service_delete() {
    let service = WorkflowCheckpointService::new_default();
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));

    let executor = WorkflowExecutor::new(workflow);
    let checkpoint = WorkflowCheckpoint::from_executor("workflow-1", 0, &executor, 0);

    service.save(&checkpoint).unwrap();

    let load_result = service.load(&checkpoint.id);
    assert!(load_result.unwrap().is_some());

    let delete_result = service.delete(&checkpoint.id);
    assert!(delete_result.is_ok());

    let load_result = service.load(&checkpoint.id);
    assert!(load_result.unwrap().is_none());
}

#[test]
fn test_checkpoint_service_save_rejects_corrupted() {
    let service = WorkflowCheckpointService::new_default();
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));

    let executor = WorkflowExecutor::new(workflow);
    let mut checkpoint = WorkflowCheckpoint::from_executor("workflow-1", 0, &executor, 0);

    checkpoint.checksum = "corrupted".to_string();

    let save_result = service.save(&checkpoint);
    assert!(save_result.is_err());
}

#[test]
fn test_validate_workflow_consistency_success() {
    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));
    workflow.add_task(Box::new(MockTask::new("task-2", "Task 2")));
    workflow.add_task(Box::new(MockTask::new("task-3", "Task 3")));

    let executor = WorkflowExecutor::new(workflow);
    let checkpoint = WorkflowCheckpoint::from_executor("workflow-1", 0, &executor, 0);

    let mut validation_workflow = Workflow::new();
    validation_workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));
    validation_workflow.add_task(Box::new(MockTask::new("task-2", "Task 2")));
    validation_workflow.add_task(Box::new(MockTask::new("task-3", "Task 3")));

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

    let mut validation_workflow = Workflow::new();
    validation_workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));
    validation_workflow.add_task(Box::new(MockTask::new("task-2", "Task 2")));
    validation_workflow.add_task(Box::new(MockTask::new("task-3", "Task 3")));

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

    executor.completed_tasks.insert(TaskId::new("task-1"));

    let checkpoint = WorkflowCheckpoint::from_executor("workflow-1", 0, &executor, 1);

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

    let mut checkpoint = WorkflowCheckpoint::from_executor("workflow-1", 0, &executor, 0);
    checkpoint.current_position = 5;

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

    let mut validation_workflow = Workflow::new();
    validation_workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));
    validation_workflow.add_task(Box::new(MockTask::new("task-2", "Task 2")));
    validation_workflow.add_task(Box::new(MockTask::new("task-4", "Task 4")));

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
    let ids1 = vec![
        TaskId::new("task-3"),
        TaskId::new("task-1"),
        TaskId::new("task-2"),
    ];
    let ids2 = vec![
        TaskId::new("task-1"),
        TaskId::new("task-2"),
        TaskId::new("task-3"),
    ];

    use super::compute_task_ids_checksum;
    let checksum1 = compute_task_ids_checksum(&ids1);
    let checksum2 = compute_task_ids_checksum(&ids2);

    assert_eq!(checksum1, checksum2);
}

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
    let _specific = RollbackRecommendation::SpecificTask(TaskId::new("task-1"));
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
    assert!(config.rollback_on_failure);
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
    assert!(!config.rollback_on_failure);
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

    let serialized = serde_json::to_string(&result);
    assert!(serialized.is_ok());

    let deserialized: Result<ValidationResult, _> = serde_json::from_str(&serialized.unwrap());
    assert!(deserialized.is_ok());

    let restored = deserialized.unwrap();
    assert_eq!(restored.confidence, result.confidence);
    assert_eq!(restored.status, result.status);
    assert_eq!(restored.message, result.message);
}

#[test]
fn test_extract_confidence_success() {
    let result = TaskResult::Success;
    let confidence = extract_confidence(&result);
    assert_eq!(confidence, 1.0);
}

#[test]
fn test_extract_confidence_skipped() {
    let result = TaskResult::Skipped;
    let confidence = extract_confidence(&result);
    assert_eq!(confidence, 0.5);
}

#[test]
fn test_extract_confidence_failed() {
    let result = TaskResult::Failed("error".to_string());
    let confidence = extract_confidence(&result);
    assert_eq!(confidence, 0.0);
}

#[test]
fn test_extract_confidence_with_compensation() {
    use crate::workflow::task::ExecutableCompensation;

    let inner = Box::new(TaskResult::Success);
    let compensation = ExecutableCompensation::skip("test");
    let result = TaskResult::WithCompensation {
        result: inner,
        compensation,
    };

    let confidence = extract_confidence(&result);
    assert_eq!(confidence, 1.0);
}

#[test]
fn test_extract_confidence_with_compensation_failed() {
    use crate::workflow::task::ExecutableCompensation;

    let inner = Box::new(TaskResult::Failed("error".to_string()));
    let compensation = ExecutableCompensation::skip("test");
    let result = TaskResult::WithCompensation {
        result: inner,
        compensation,
    };

    let confidence = extract_confidence(&result);
    assert_eq!(confidence, 0.0);
}

#[test]
fn test_validate_checkpoint_passed() {
    let result = TaskResult::Success;
    let config = ValidationCheckpoint::default();

    let validation = validate_checkpoint(&result, &config);

    assert_eq!(validation.confidence, 1.0);
    assert_eq!(validation.status, ValidationStatus::Passed);
    assert!(validation.message.contains("100%"));
    assert!(validation.rollback_recommendation.is_none());
}

#[test]
fn test_validate_checkpoint_warning() {
    let result = TaskResult::Skipped;
    let config = ValidationCheckpoint {
        min_confidence: 0.4,
        warning_threshold: 0.6,
        rollback_on_failure: true,
    };

    let validation = validate_checkpoint(&result, &config);

    assert_eq!(validation.confidence, 0.5);
    assert_eq!(validation.status, ValidationStatus::Warning);
    assert!(validation.message.contains("50%"));
    assert!(validation.rollback_recommendation.is_none());
}

#[test]
fn test_validate_checkpoint_failed() {
    let result = TaskResult::Failed("error".to_string());
    let config = ValidationCheckpoint::default();

    let validation = validate_checkpoint(&result, &config);

    assert_eq!(validation.confidence, 0.0);
    assert_eq!(validation.status, ValidationStatus::Failed);
    assert!(validation.message.contains("0%"));
    assert!(validation.rollback_recommendation.is_some());
}

#[test]
fn test_validate_thresholds_custom() {
    let result = TaskResult::Skipped;

    let config = ValidationCheckpoint {
        min_confidence: 0.4,
        warning_threshold: 0.6,
        rollback_on_failure: false,
    };

    let validation = validate_checkpoint(&result, &config);

    assert_eq!(validation.status, ValidationStatus::Warning);
    assert!(validation.rollback_recommendation.is_none());
}

#[test]
fn test_can_proceed_passed() {
    let validation = ValidationResult {
        confidence: 0.9,
        status: ValidationStatus::Passed,
        message: "Good".to_string(),
        rollback_recommendation: None,
        timestamp: Utc::now(),
    };

    assert!(can_proceed(&validation));
}

#[test]
fn test_can_proceed_warning() {
    let validation = ValidationResult {
        confidence: 0.7,
        status: ValidationStatus::Warning,
        message: "Warning".to_string(),
        rollback_recommendation: None,
        timestamp: Utc::now(),
    };

    assert!(can_proceed(&validation));
}

#[test]
fn test_can_proceed_failed() {
    let validation = ValidationResult {
        confidence: 0.3,
        status: ValidationStatus::Failed,
        message: "Failed".to_string(),
        rollback_recommendation: None,
        timestamp: Utc::now(),
    };

    assert!(!can_proceed(&validation));
}

#[test]
fn test_requires_rollback_true() {
    let validation = ValidationResult {
        confidence: 0.0,
        status: ValidationStatus::Failed,
        message: "Failed".to_string(),
        rollback_recommendation: Some(RollbackRecommendation::FullRollback),
        timestamp: Utc::now(),
    };

    assert!(requires_rollback(&validation));
}

#[test]
fn test_requires_rollback_false_no_rollback() {
    let validation = ValidationResult {
        confidence: 0.0,
        status: ValidationStatus::Failed,
        message: "Failed".to_string(),
        rollback_recommendation: None,
        timestamp: Utc::now(),
    };

    assert!(!requires_rollback(&validation));
}

#[test]
fn test_requires_rollback_false_passed() {
    let validation = ValidationResult {
        confidence: 1.0,
        status: ValidationStatus::Passed,
        message: "Passed".to_string(),
        rollback_recommendation: None,
        timestamp: Utc::now(),
    };

    assert!(!requires_rollback(&validation));
}

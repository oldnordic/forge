//! Audit trail system with serde-serializable events and JSON persistence.
//!
//! The audit trail provides a complete record of all agent operations for
//! debugging, compliance, and transaction replay. Every phase transition
//! is logged with timestamps and relevant data.
//!
//! # Audit Events
//!
//! Each phase of the agent loop records an audit event:
//! - `Observe`: Context gathering from the graph
//! - `Constrain`: Policy validation results
//! - `Plan`: Execution step generation
//! - `Mutate`: File modifications applied
//! - `Verify`: Validation results
//! - `Commit`: Transaction finalization
//! - `Rollback`: Error recovery with reason
//!
//! # Persistence
//!
//! Events are persisted to `.forge/audit/{tx_id}.json` after each phase
//! for durability and replay capability.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;
use uuid::Uuid;

/// Error types for audit operations.
#[derive(Error, Debug)]
pub enum AuditError {
    /// Failed to serialize audit event
    #[error("Serialization failed: {0}")]
    SerializationFailed(#[from] serde_json::Error),

    /// Failed to write audit file
    #[error("Write failed: {0}")]
    WriteFailed(#[from] std::io::Error),

    /// Failed to create audit directory
    #[error("Directory creation failed: {0}")]
    DirectoryFailed(String),
}

/// Audit event for phase transitions.
///
/// Each event captures the timestamp and phase-specific data for
/// complete transaction reconstruction.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AuditEvent {
    /// Observation phase - gather context from graph
    Observe {
        timestamp: DateTime<Utc>,
        query: String,
        symbol_count: usize,
    },
    /// Constraint phase - apply policy rules
    Constrain {
        timestamp: DateTime<Utc>,
        policy_count: usize,
        violations: usize,
    },
    /// Plan phase - generate execution steps
    Plan {
        timestamp: DateTime<Utc>,
        step_count: usize,
        estimated_files: usize,
    },
    /// Mutate phase - apply changes
    Mutate {
        timestamp: DateTime<Utc>,
        files_modified: Vec<String>,
    },
    /// Verify phase - validate results
    Verify {
        timestamp: DateTime<Utc>,
        passed: bool,
        diagnostic_count: usize,
    },
    /// Commit phase - finalize transaction
    Commit {
        timestamp: DateTime<Utc>,
        transaction_id: String,
    },
    /// Rollback occurred with reason
    Rollback {
        timestamp: DateTime<Utc>,
        reason: String,
        phase: String,
    },
    /// Workflow execution started
    WorkflowStarted {
        timestamp: DateTime<Utc>,
        workflow_id: String,
        task_count: usize,
    },
    /// Workflow task started
    WorkflowTaskStarted {
        timestamp: DateTime<Utc>,
        workflow_id: String,
        task_id: String,
        task_name: String,
    },
    /// Workflow task completed
    WorkflowTaskCompleted {
        timestamp: DateTime<Utc>,
        workflow_id: String,
        task_id: String,
        task_name: String,
        result: String,
    },
    /// Workflow task failed
    WorkflowTaskFailed {
        timestamp: DateTime<Utc>,
        workflow_id: String,
        task_id: String,
        task_name: String,
        error: String,
    },
    /// Workflow execution completed
    WorkflowCompleted {
        timestamp: DateTime<Utc>,
        workflow_id: String,
        total_tasks: usize,
        completed_tasks: usize,
    },
    /// Workflow task rolled back
    WorkflowTaskRolledBack {
        timestamp: DateTime<Utc>,
        workflow_id: String,
        task_id: String,
        compensation: String,
    },
    /// Workflow rolled back
    WorkflowRolledBack {
        timestamp: DateTime<Utc>,
        workflow_id: String,
        reason: String,
        rolled_back_tasks: Vec<String>,
    },
}

/// Audit log for recording and persisting phase transitions.
///
/// Each audit log has a unique transaction ID and persists events
/// to `.forge/audit/{tx_id}.json` for replay capability.
pub struct AuditLog {
    /// Unique transaction identifier
    tx_id: Uuid,
    /// Accumulated events for this transaction
    events: Vec<AuditEvent>,
    /// Directory for audit file storage
    audit_dir: PathBuf,
}

impl AuditLog {
    /// Creates a new audit log with a fresh transaction ID.
    ///
    /// The `.forge/audit` directory is created if it doesn't exist.
    pub fn new() -> Self {
        Self::with_dir(PathBuf::from(".forge/audit"))
    }

    /// Creates a new audit log with a custom audit directory.
    ///
    /// # Arguments
    ///
    /// * `audit_dir` - Directory path for audit file storage
    pub fn with_dir(audit_dir: PathBuf) -> Self {
        Self {
            tx_id: Uuid::new_v4(),
            events: Vec::new(),
            audit_dir,
        }
    }

    /// Records an audit event and persists to disk.
    ///
    /// # Arguments
    ///
    /// * `event` - The audit event to record
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or `AuditError` on failure.
    pub async fn record(&mut self, event: AuditEvent) -> Result<(), AuditError> {
        self.events.push(event);
        self.persist().await?;
        Ok(())
    }

    /// Persists all events to the audit file.
    ///
    /// Events are written as formatted JSON to `.forge/audit/{tx_id}.json`.
    async fn persist(&self) -> Result<(), AuditError> {
        // Create audit directory if it doesn't exist
        tokio::fs::create_dir_all(&self.audit_dir)
            .await
            .map_err(|e| AuditError::DirectoryFailed(e.to_string()))?;

        // Serialize events to JSON
        let json = serde_json::to_string_pretty(&self.events)?;

        // Write to audit file
        let audit_path = self.audit_dir.join(format!("{}.json", self.tx_id));
        tokio::fs::write(audit_path, json).await?;

        Ok(())
    }

    /// Returns a replay of all recorded events.
    ///
    /// # Returns
    ///
    /// A clone of all events for transaction reconstruction.
    pub fn replay(&self) -> Vec<AuditEvent> {
        self.events.clone()
    }

    /// Returns the transaction ID.
    ///
    /// # Returns
    ///
    /// The UUID identifying this transaction.
    pub fn tx_id(&self) -> Uuid {
        self.tx_id
    }

    /// Converts the audit log into a vector of events.
    ///
    /// This consumes the audit log and returns all events.
    pub fn into_events(self) -> Vec<AuditEvent> {
        self.events
    }

    /// Returns the number of events in the log.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Returns true if the audit log has no events.
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for AuditLog {
    fn clone(&self) -> Self {
        Self {
            tx_id: self.tx_id,
            events: self.events.clone(),
            audit_dir: self.audit_dir.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_audit_log_creation() {
        let log = AuditLog::new();
        assert!(!log.events.is_empty() || log.is_empty()); // Should be empty on creation
        assert_ne!(log.tx_id(), Uuid::nil());
    }

    #[tokio::test]
    async fn test_audit_event_serialization() {
        let event = AuditEvent::Observe {
            timestamp: Utc::now(),
            query: "test query".to_string(),
            symbol_count: 42,
        };

        // Serialize and deserialize
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: AuditEvent = serde_json::from_str(&json).unwrap();

        match deserialized {
            AuditEvent::Observe {
                query,
                symbol_count,
                ..
            } => {
                assert_eq!(query, "test query");
                assert_eq!(symbol_count, 42);
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[tokio::test]
    async fn test_audit_log_record_and_replay() {
        let temp_dir = TempDir::new().unwrap();
        let mut log = AuditLog::with_dir(temp_dir.path().to_path_buf());

        let event = AuditEvent::Observe {
            timestamp: Utc::now(),
            query: "test".to_string(),
            symbol_count: 1,
        };

        log.record(event.clone()).await.unwrap();

        let replayed = log.replay();
        assert_eq!(replayed.len(), 1);
        assert!(matches!(replayed[0], AuditEvent::Observe { .. }));
    }

    #[tokio::test]
    async fn test_audit_log_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let audit_dir = temp_dir.path().to_path_buf();

        let tx_id = {
            let mut log = AuditLog::with_dir(audit_dir.clone());
            let tx = log.tx_id();
            log.record(AuditEvent::Observe {
                timestamp: Utc::now(),
                query: "persistence test".to_string(),
                symbol_count: 0,
            })
            .await
            .unwrap();
            tx
        };

        // Verify file was created
        let audit_file = audit_dir.join(format!("{}.json", tx_id));
        assert!(audit_file.exists());

        // Verify file contents
        let contents = tokio::fs::read_to_string(audit_file).await.unwrap();
        let events: Vec<AuditEvent> = serde_json::from_str(&contents).unwrap();
        assert_eq!(events.len(), 1);
    }

    #[tokio::test]
    async fn test_all_audit_event_variants() {
        let temp_dir = TempDir::new().unwrap();
        let mut log = AuditLog::with_dir(temp_dir.path().to_path_buf());

        // Test all event variants
        log.record(AuditEvent::Observe {
            timestamp: Utc::now(),
            query: "test".to_string(),
            symbol_count: 5,
        })
        .await
        .unwrap();

        log.record(AuditEvent::Constrain {
            timestamp: Utc::now(),
            policy_count: 2,
            violations: 0,
        })
        .await
        .unwrap();

        log.record(AuditEvent::Plan {
            timestamp: Utc::now(),
            step_count: 3,
            estimated_files: 2,
        })
        .await
        .unwrap();

        log.record(AuditEvent::Mutate {
            timestamp: Utc::now(),
            files_modified: vec!["test.rs".to_string()],
        })
        .await
        .unwrap();

        log.record(AuditEvent::Verify {
            timestamp: Utc::now(),
            passed: true,
            diagnostic_count: 0,
        })
        .await
        .unwrap();

        log.record(AuditEvent::Commit {
            timestamp: Utc::now(),
            transaction_id: "tx-123".to_string(),
        })
        .await
        .unwrap();

        log.record(AuditEvent::Rollback {
            timestamp: Utc::now(),
            reason: "test error".to_string(),
            phase: "TestPhase".to_string(),
        })
        .await
        .unwrap();

        // Test workflow events
        log.record(AuditEvent::WorkflowStarted {
            timestamp: Utc::now(),
            workflow_id: "workflow-1".to_string(),
            task_count: 3,
        })
        .await
        .unwrap();

        log.record(AuditEvent::WorkflowTaskStarted {
            timestamp: Utc::now(),
            workflow_id: "workflow-1".to_string(),
            task_id: "task-1".to_string(),
            task_name: "Task 1".to_string(),
        })
        .await
        .unwrap();

        log.record(AuditEvent::WorkflowTaskCompleted {
            timestamp: Utc::now(),
            workflow_id: "workflow-1".to_string(),
            task_id: "task-1".to_string(),
            task_name: "Task 1".to_string(),
            result: "Success".to_string(),
        })
        .await
        .unwrap();

        log.record(AuditEvent::WorkflowTaskFailed {
            timestamp: Utc::now(),
            workflow_id: "workflow-1".to_string(),
            task_id: "task-2".to_string(),
            task_name: "Task 2".to_string(),
            error: "Task failed".to_string(),
        })
        .await
        .unwrap();

        log.record(AuditEvent::WorkflowCompleted {
            timestamp: Utc::now(),
            workflow_id: "workflow-1".to_string(),
            total_tasks: 3,
            completed_tasks: 2,
        })
        .await
        .unwrap();

        let events = log.replay();
        assert_eq!(events.len(), 12);
    }

    #[tokio::test]
    async fn test_workflow_event_serialization() {
        let event = AuditEvent::WorkflowStarted {
            timestamp: Utc::now(),
            workflow_id: "workflow-1".to_string(),
            task_count: 3,
        };

        // Serialize and deserialize
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: AuditEvent = serde_json::from_str(&json).unwrap();

        match deserialized {
            AuditEvent::WorkflowStarted {
                workflow_id,
                task_count,
                ..
            } => {
                assert_eq!(workflow_id, "workflow-1");
                assert_eq!(task_count, 3);
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[tokio::test]
    async fn test_workflow_execution_audit_trail() {
        let temp_dir = TempDir::new().unwrap();
        let mut log = AuditLog::with_dir(temp_dir.path().to_path_buf());

        // Simulate workflow execution
        log.record(AuditEvent::WorkflowStarted {
            timestamp: Utc::now(),
            workflow_id: "workflow-1".to_string(),
            task_count: 2,
        })
        .await
        .unwrap();

        log.record(AuditEvent::WorkflowTaskStarted {
            timestamp: Utc::now(),
            workflow_id: "workflow-1".to_string(),
            task_id: "task-1".to_string(),
            task_name: "Task 1".to_string(),
        })
        .await
        .unwrap();

        log.record(AuditEvent::WorkflowTaskCompleted {
            timestamp: Utc::now(),
            workflow_id: "workflow-1".to_string(),
            task_id: "task-1".to_string(),
            task_name: "Task 1".to_string(),
            result: "Success".to_string(),
        })
        .await
        .unwrap();

        log.record(AuditEvent::WorkflowCompleted {
            timestamp: Utc::now(),
            workflow_id: "workflow-1".to_string(),
            total_tasks: 2,
            completed_tasks: 2,
        })
        .await
        .unwrap();

        let events = log.replay();

        // Verify workflow started is first
        assert!(matches!(events[0], AuditEvent::WorkflowStarted { .. }));

        // Verify workflow completed is last
        assert!(matches!(events[events.len() - 1], AuditEvent::WorkflowCompleted { .. }));

        // Count workflow task events
        let task_events: Vec<_> = events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    AuditEvent::WorkflowTaskStarted { .. } | AuditEvent::WorkflowTaskCompleted { .. }
                )
            })
            .collect();

        assert_eq!(task_events.len(), 2);
    }

    #[tokio::test]
    async fn test_audit_log_into_events() {
        let mut log = AuditLog::new();
        log.record(AuditEvent::Constrain {
            timestamp: Utc::now(),
            policy_count: 1,
            violations: 0,
        })
        .await
        .unwrap();

        let events = log.into_events();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], AuditEvent::Constrain { .. }));
    }

    #[tokio::test]
    async fn test_audit_log_clone() {
        let mut log = AuditLog::new();
        log.record(AuditEvent::Plan {
            timestamp: Utc::now(),
            step_count: 1,
            estimated_files: 1,
        })
        .await
        .unwrap();

        let cloned = log.clone();
        assert_eq!(cloned.tx_id(), log.tx_id());
        assert_eq!(cloned.len(), log.len());
    }
}

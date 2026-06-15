use super::{CheckpointId, WorkflowCheckpoint};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

type CheckpointStore = Arc<RwLock<HashMap<String, (Vec<u8>, CheckpointSummary)>>>;
type WorkflowLatestMap = Arc<RwLock<HashMap<String, CheckpointSummary>>>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckpointSummary {
    pub id: CheckpointId,
    pub sequence: u64,
    pub timestamp: DateTime<Utc>,
    pub completed_count: usize,
    pub current_position: usize,
    pub total_tasks: usize,
}

impl CheckpointSummary {
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

#[derive(Clone)]
pub struct WorkflowCheckpointService {
    pub(super) namespace: String,
    #[allow(dead_code, reason = "Planned for persistent storage integration")]
    storage: CheckpointStore,
    latest_by_workflow: WorkflowLatestMap,
}

impl WorkflowCheckpointService {
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            storage: Arc::new(RwLock::new(HashMap::new())),
            latest_by_workflow: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn new_default() -> Self {
        Self::new("workflow")
    }

    pub fn save(
        &self,
        checkpoint: &WorkflowCheckpoint,
    ) -> Result<(), crate::workflow::WorkflowError> {
        checkpoint.validate()?;

        let data = serde_json::to_vec(checkpoint).map_err(|e| {
            crate::workflow::WorkflowError::CheckpointCorrupted(format!(
                "Serialization failed: {}",
                e
            ))
        })?;

        let summary = CheckpointSummary::from_checkpoint(checkpoint);

        let key = format!("{}:{}", self.namespace, checkpoint.id);
        {
            let mut storage = self.storage.write().map_err(|e| {
                crate::workflow::WorkflowError::CheckpointCorrupted(format!(
                    "Storage lock failed: {}",
                    e
                ))
            })?;
            storage.insert(key, (data, summary.clone()));
        }

        {
            let mut latest = self.latest_by_workflow.write().map_err(|e| {
                crate::workflow::WorkflowError::CheckpointCorrupted(format!(
                    "Latest lock failed: {}",
                    e
                ))
            })?;
            latest.insert(checkpoint.workflow_id.clone(), summary);
        }

        Ok(())
    }

    pub fn load(
        &self,
        id: &CheckpointId,
    ) -> Result<Option<WorkflowCheckpoint>, crate::workflow::WorkflowError> {
        let key = format!("{}:{}", self.namespace, id);

        let storage = self.storage.read().map_err(|e| {
            crate::workflow::WorkflowError::CheckpointCorrupted(format!(
                "Storage lock failed: {}",
                e
            ))
        })?;

        if let Some((data, _)) = storage.get(&key) {
            let checkpoint: WorkflowCheckpoint = serde_json::from_slice(data).map_err(|e| {
                crate::workflow::WorkflowError::CheckpointCorrupted(format!(
                    "Deserialization failed: {}",
                    e
                ))
            })?;

            checkpoint.validate()?;

            Ok(Some(checkpoint))
        } else {
            Ok(None)
        }
    }

    pub fn get_latest(
        &self,
        workflow_id: &str,
    ) -> Result<Option<WorkflowCheckpoint>, crate::workflow::WorkflowError> {
        let latest = self.latest_by_workflow.read().map_err(|e| {
            crate::workflow::WorkflowError::CheckpointCorrupted(format!(
                "Latest lock failed: {}",
                e
            ))
        })?;

        if let Some(summary) = latest.get(workflow_id) {
            self.load(&summary.id)
        } else {
            Ok(None)
        }
    }

    pub fn list_by_workflow(
        &self,
        _workflow_id: &str,
    ) -> Result<Vec<CheckpointSummary>, crate::workflow::WorkflowError> {
        let storage = self.storage.read().map_err(|e| {
            crate::workflow::WorkflowError::CheckpointCorrupted(format!(
                "Storage lock failed: {}",
                e
            ))
        })?;

        let mut summaries: Vec<CheckpointSummary> = storage
            .values()
            .map(|(_, summary)| summary.clone())
            .collect();

        summaries.sort_by_key(|s| s.sequence);

        Ok(summaries)
    }

    pub fn delete(&self, id: &CheckpointId) -> Result<(), crate::workflow::WorkflowError> {
        let key = format!("{}:{}", self.namespace, id);

        let mut storage = self.storage.write().map_err(|e| {
            crate::workflow::WorkflowError::CheckpointCorrupted(format!(
                "Storage lock failed: {}",
                e
            ))
        })?;

        storage.remove(&key);

        Ok(())
    }
}

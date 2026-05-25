//! Sequential workflow executor with audit logging and rollback.
//!
//! Executes tasks in topological order, recording all events to the audit log.
//! On failure, triggers selective rollback of dependent tasks using Saga compensation.

mod audit;
mod parallel;
mod result;
mod serial;

pub use result::WorkflowResult;

use crate::audit::AuditLog;
use crate::workflow::checkpoint::{
    validate_checkpoint, validate_workflow_consistency, ValidationCheckpoint, ValidationResult,
    WorkflowCheckpoint, WorkflowCheckpointService,
};
use crate::workflow::dag::Workflow;
use crate::workflow::rollback::{
    CompensationRegistry, RollbackEngine, RollbackStrategy, ToolCompensation,
};
use crate::workflow::task::{TaskContext, TaskId, TaskResult};
use crate::workflow::timeout::{TimeoutConfig, TimeoutError};
use crate::workflow::tools::ToolRegistry;
use forge_core::Forge;
use std::collections::HashSet;
use std::sync::Arc;

pub struct WorkflowExecutor {
    pub(in crate::workflow) workflow: Workflow,
    pub(in crate::workflow) audit_log: AuditLog,
    pub(in crate::workflow) completed_tasks: HashSet<TaskId>,
    pub(in crate::workflow) failed_tasks: Vec<TaskId>,
    pub(in crate::workflow::executor) rollback_engine: RollbackEngine,
    pub(in crate::workflow::executor) rollback_strategy: RollbackStrategy,
    pub(in crate::workflow) compensation_registry: CompensationRegistry,
    pub(in crate::workflow) checkpoint_service: Option<WorkflowCheckpointService>,
    pub(in crate::workflow) checkpoint_sequence: u64,
    pub(in crate::workflow) validation_config: Option<ValidationCheckpoint>,
    cancellation_source: Option<crate::workflow::cancellation::CancellationTokenSource>,
    pub(in crate::workflow) timeout_config: Option<TimeoutConfig>,
    pub(in crate::workflow) tool_registry: Option<Arc<ToolRegistry>>,
    pub(in crate::workflow) deadlock_timeout: Option<std::time::Duration>,
    forge: Option<Arc<Forge>>,
}

impl WorkflowExecutor {
    pub fn new(workflow: Workflow) -> Self {
        Self {
            workflow,
            audit_log: AuditLog::new(),
            completed_tasks: HashSet::new(),
            failed_tasks: Vec::new(),
            rollback_engine: RollbackEngine::new(),
            rollback_strategy: RollbackStrategy::AllDependent,
            compensation_registry: CompensationRegistry::new(),
            checkpoint_service: None,
            checkpoint_sequence: 0,
            validation_config: None,
            cancellation_source: None,
            timeout_config: None,
            tool_registry: None,
            deadlock_timeout: Some(std::time::Duration::from_secs(300)),
            forge: None,
        }
    }

    pub fn with_forge(mut self, forge: Arc<Forge>) -> Self {
        self.forge = Some(forge);
        self
    }

    pub fn with_rollback_strategy(mut self, strategy: RollbackStrategy) -> Self {
        self.rollback_strategy = strategy;
        self
    }

    pub fn with_checkpoint_service(mut self, service: WorkflowCheckpointService) -> Self {
        self.checkpoint_service = Some(service);
        self
    }

    pub fn with_validation_config(mut self, config: ValidationCheckpoint) -> Self {
        self.validation_config = Some(config);
        self
    }

    pub fn with_cancellation_source(
        mut self,
        source: crate::workflow::cancellation::CancellationTokenSource,
    ) -> Self {
        self.cancellation_source = Some(source);
        self
    }

    pub fn cancellation_token(&self) -> Option<crate::workflow::cancellation::CancellationToken> {
        self.cancellation_source
            .as_ref()
            .map(|source| source.token())
    }

    pub fn cancel(&self) {
        if let Some(source) = &self.cancellation_source {
            source.cancel();
        }
    }

    pub fn with_timeout_config(mut self, config: TimeoutConfig) -> Self {
        self.timeout_config = Some(config);
        self
    }

    pub fn with_tool_registry(mut self, registry: ToolRegistry) -> Self {
        self.tool_registry = Some(Arc::new(registry));
        self
    }

    pub fn with_deadlock_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.deadlock_timeout = Some(timeout);
        self
    }

    pub fn without_deadlock_timeout(mut self) -> Self {
        self.deadlock_timeout = None;
        self
    }

    pub fn tool_registry(&self) -> Option<&Arc<ToolRegistry>> {
        self.tool_registry.as_ref()
    }

    pub fn timeout_config(&self) -> Option<&TimeoutConfig> {
        self.timeout_config.as_ref()
    }

    pub fn register_compensation(&mut self, task_id: TaskId, compensation: ToolCompensation) {
        self.compensation_registry.register(task_id, compensation);
    }

    pub fn register_file_compensation(&mut self, task_id: TaskId, file_path: impl Into<String>) {
        self.compensation_registry
            .register_file_creation(task_id, file_path);
    }

    pub fn validate_compensation_coverage(&self) -> crate::workflow::rollback::CompensationReport {
        let task_ids = self.workflow.task_ids();
        let report = self.compensation_registry.validate_coverage(&task_ids);

        if report.coverage_percentage < 1.0 {
            let missing = &report.tasks_without_compensation;
            if !missing.is_empty() {
                eprintln!(
                    "Warning: {} tasks lack compensation: {:?}",
                    missing.len(),
                    missing
                );
            }
        }

        report
    }

    async fn execute_task(
        &mut self,
        workflow_id: &str,
        task_id: &TaskId,
    ) -> Result<TaskResult, crate::workflow::WorkflowError> {
        let node_idx = self
            .workflow
            .task_map
            .get(task_id)
            .ok_or_else(|| crate::workflow::WorkflowError::TaskNotFound(task_id.clone()))?;

        let task_node = self.workflow.graph.node_weight(*node_idx).ok_or_else(|| {
            crate::workflow::WorkflowError::TaskFailed(
                "Node index exists but node not found in graph".to_string(),
            )
        })?;

        let task_arc = std::sync::Arc::clone(task_node.task());
        let task_name = task_node.name.clone();

        self.record_task_started(workflow_id, task_id, &task_name)
            .await;

        let mut context = if let Some(token) = self.cancellation_token() {
            TaskContext::new(workflow_id, task_id.clone()).with_cancellation_token(token)
        } else {
            TaskContext::new(workflow_id, task_id.clone())
        };

        if let Some(config) = &self.timeout_config {
            if let Some(task_timeout) = config.task_timeout {
                context = context.with_task_timeout(task_timeout.duration());
            }
        }

        if let Some(ref registry) = self.tool_registry {
            context = context.with_tool_registry(Arc::clone(registry));
        }

        context = context.with_audit_log(self.audit_log.clone());

        if let Some(ref f) = self.forge {
            context = context.with_forge((**f).clone());
        }

        let execution_result = if let Some(timeout_duration) = context.task_timeout {
            match tokio::time::timeout(timeout_duration, self.do_execute_task(&task_arc, &context))
                .await
            {
                Ok(result) => result,
                Err(_) => {
                    self.record_task_timeout(
                        workflow_id,
                        task_id,
                        &task_name,
                        timeout_duration.as_secs(),
                    )
                    .await;

                    return Err(crate::workflow::WorkflowError::Timeout(
                        TimeoutError::TaskTimeout {
                            task_id: task_id.to_string(),
                            timeout: timeout_duration,
                        },
                    ));
                }
            }
        } else {
            self.do_execute_task(&task_arc, &context).await
        };

        match execution_result {
            Ok(result) => {
                self.completed_tasks.insert(task_id.clone());
                self.record_task_completed(workflow_id, task_id, &task_name)
                    .await;
                Ok(result)
            }
            Err(e) => Err(e),
        }
    }

    async fn do_execute_task(
        &mut self,
        task: &std::sync::Arc<dyn crate::workflow::WorkflowTask>,
        context: &TaskContext,
    ) -> Result<TaskResult, crate::workflow::WorkflowError> {
        let result = task
            .execute(context)
            .await
            .map_err(|e| crate::workflow::WorkflowError::TaskFailed(e.to_string()))?;

        match result {
            TaskResult::WithCompensation {
                result,
                compensation,
            } => {
                let task_id = task.id();
                let tool_comp: ToolCompensation = compensation.into();
                self.compensation_registry.register(task_id, tool_comp);
                Ok(*result)
            }
            other => Ok(other),
        }
    }

    async fn handle_task_failure(
        &mut self,
        workflow_id: &str,
        task_id: &TaskId,
        error_msg: &str,
    ) -> Result<WorkflowResult, crate::workflow::WorkflowError> {
        let completed: Vec<TaskId> = self.completed_tasks.iter().cloned().collect();

        let rollback_set = self
            .rollback_engine
            .find_rollback_set(&self.workflow, task_id, self.rollback_strategy)
            .map_err(|_err| crate::workflow::WorkflowError::TaskNotFound(task_id.clone()))?;

        let rollback_report = self
            .rollback_engine
            .execute_rollback(
                &self.workflow,
                rollback_set,
                workflow_id,
                &mut self.audit_log,
                &self.compensation_registry,
            )
            .await
            .map_err(|_err| crate::workflow::dag::WorkflowError::TaskNotFound(task_id.clone()))?;

        self.record_workflow_failed(workflow_id, task_id, error_msg)
            .await;

        Ok(WorkflowResult::new_failed_with_rollback(
            completed,
            task_id.clone(),
            error_msg.to_string(),
            rollback_report,
        ))
    }

    pub fn audit_log(&self) -> &AuditLog {
        &self.audit_log
    }

    pub fn completed_count(&self) -> usize {
        self.completed_tasks.len()
    }

    pub fn failed_count(&self) -> usize {
        self.failed_tasks.len()
    }

    pub fn task_count(&self) -> usize {
        self.workflow.task_count()
    }

    pub fn task_ids(&self) -> Vec<TaskId> {
        self.workflow.task_ids()
    }

    pub fn completed_task_ids(&self) -> Vec<TaskId> {
        self.completed_tasks.iter().cloned().collect()
    }

    pub fn failed_task_ids(&self) -> Vec<TaskId> {
        self.failed_tasks.clone()
    }

    pub fn is_task_completed(&self, id: &TaskId) -> bool {
        self.completed_tasks.contains(id)
    }

    pub fn is_task_failed(&self, id: &TaskId) -> bool {
        self.failed_tasks.contains(id)
    }

    pub fn progress(&self) -> f64 {
        let total = self.workflow.task_count();
        if total == 0 {
            return 0.0;
        }
        self.completed_tasks.len() as f64 / total as f64
    }

    pub fn rollback_strategy(&self) -> RollbackStrategy {
        self.rollback_strategy
    }

    pub(in crate::workflow::executor) async fn create_checkpoint(
        &mut self,
        workflow_id: &str,
        position: usize,
    ) {
        let service = match &self.checkpoint_service {
            Some(s) => s,
            None => return,
        };

        let checkpoint = WorkflowCheckpoint::from_executor(
            workflow_id,
            self.checkpoint_sequence,
            self,
            position,
        );

        if let Err(e) = service.save(&checkpoint) {
            let _ = self
                .audit_log
                .record(crate::audit::AuditEvent::WorkflowTaskFailed {
                    timestamp: chrono::Utc::now(),
                    workflow_id: workflow_id.to_string(),
                    task_id: format!("checkpoint-{}", self.checkpoint_sequence),
                    task_name: "Checkpoint".to_string(),
                    error: format!("Checkpoint save failed: {}", e),
                })
                .await;
        } else {
            self.checkpoint_sequence += 1;
        }
    }

    fn restore_state_from_checkpoint(
        &mut self,
        checkpoint: &WorkflowCheckpoint,
    ) -> Result<(), crate::workflow::WorkflowError> {
        self.completed_tasks.clear();
        self.failed_tasks.clear();

        for task_id in &checkpoint.completed_tasks {
            self.completed_tasks.insert(task_id.clone());
        }

        self.failed_tasks = checkpoint.failed_tasks.clone();
        self.checkpoint_sequence = checkpoint.sequence + 1;

        Ok(())
    }

    pub fn restore_checkpoint_state(
        &mut self,
        checkpoint: &WorkflowCheckpoint,
    ) -> Result<(), crate::workflow::WorkflowError> {
        validate_workflow_consistency(&self.workflow, checkpoint)?;
        self.restore_state_from_checkpoint(checkpoint)?;
        Ok(())
    }

    fn _validate_task_result(
        &self,
        task_result: &TaskResult,
    ) -> Result<ValidationResult, crate::workflow::WorkflowError> {
        let config = self.validation_config.as_ref().ok_or_else(|| {
            crate::workflow::WorkflowError::CheckpointCorrupted(
                "Validation configuration not set".to_string(),
            )
        })?;

        let validation = validate_checkpoint(task_result, config);
        Ok(validation)
    }

    pub fn can_resume(&self) -> bool {
        let service = match &self.checkpoint_service {
            Some(s) => s,
            None => return false,
        };

        let workflow_id = self.audit_log.tx_id().to_string();

        let checkpoint = match service.get_latest(&workflow_id) {
            Ok(Some(cp)) => cp,
            _ => return false,
        };

        if checkpoint.validate().is_err() {
            return false;
        }

        validate_workflow_consistency(&self.workflow, &checkpoint).is_ok()
    }

    pub async fn resume(&mut self) -> Result<WorkflowResult, crate::workflow::WorkflowError> {
        let service = self.checkpoint_service.as_ref().ok_or_else(|| {
            crate::workflow::WorkflowError::CheckpointNotFound(
                "No checkpoint service configured".to_string(),
            )
        })?;

        let workflow_id = self.audit_log.tx_id().to_string();

        let checkpoint = service.get_latest(&workflow_id)?.ok_or_else(|| {
            crate::workflow::WorkflowError::CheckpointNotFound(format!(
                "No checkpoint found for workflow: {}",
                workflow_id
            ))
        })?;

        self.resume_from_checkpoint_id(&checkpoint.id).await
    }

    pub async fn resume_from_checkpoint_id(
        &mut self,
        checkpoint_id: &crate::workflow::checkpoint::CheckpointId,
    ) -> Result<WorkflowResult, crate::workflow::WorkflowError> {
        let service = self.checkpoint_service.as_ref().ok_or_else(|| {
            crate::workflow::WorkflowError::CheckpointNotFound(
                "No checkpoint service configured".to_string(),
            )
        })?;

        let checkpoint = service.load(checkpoint_id)?.ok_or_else(|| {
            crate::workflow::WorkflowError::CheckpointNotFound(format!(
                "Checkpoint not found: {}",
                checkpoint_id
            ))
        })?;

        checkpoint.validate()?;
        validate_workflow_consistency(&self.workflow, &checkpoint)?;
        self.restore_state_from_checkpoint(&checkpoint)?;

        let workflow_id = self.audit_log.tx_id().to_string();

        if checkpoint.completed_tasks.len() == checkpoint.total_tasks {
            return Ok(WorkflowResult::new(checkpoint.completed_tasks));
        }

        let execution_order = self.workflow.execution_order()?;
        let start_position = checkpoint.current_position + 1;

        for (position, task_id) in execution_order.iter().enumerate().skip(start_position) {
            if let Err(e) = self.execute_task(&workflow_id, task_id).await {
                let completed: Vec<TaskId> = self.completed_tasks.iter().cloned().collect();

                let rollback_set = self
                    .rollback_engine
                    .find_rollback_set(&self.workflow, task_id, self.rollback_strategy)
                    .map_err(|_err| {
                        crate::workflow::WorkflowError::TaskNotFound(task_id.clone())
                    })?;

                let rollback_report = self
                    .rollback_engine
                    .execute_rollback(
                        &self.workflow,
                        rollback_set,
                        &workflow_id,
                        &mut self.audit_log,
                        &self.compensation_registry,
                    )
                    .await
                    .map_err(|_err| {
                        crate::workflow::dag::WorkflowError::TaskNotFound(task_id.clone())
                    })?;

                self.record_workflow_failed(&workflow_id, task_id, &e.to_string())
                    .await;

                return Ok(WorkflowResult::new_failed_with_rollback(
                    completed,
                    task_id.clone(),
                    e.to_string(),
                    rollback_report,
                ));
            }

            self.create_checkpoint(&workflow_id, position).await;
        }

        self.record_workflow_completed(&workflow_id).await;

        let completed: Vec<TaskId> = self.completed_tasks.iter().cloned().collect();
        Ok(WorkflowResult::new(completed))
    }
}

#[cfg(test)]
mod tests;

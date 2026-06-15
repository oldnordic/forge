use super::{WorkflowExecutor, WorkflowResult};
use crate::workflow::checkpoint::{can_proceed, requires_rollback, validate_checkpoint};
use crate::workflow::task::{TaskId, TaskResult};
use crate::workflow::timeout::TimeoutError;
use chrono::Utc;

impl WorkflowExecutor {
    pub async fn execute(&mut self) -> Result<WorkflowResult, crate::workflow::WorkflowError> {
        let workflow_id = self.audit_log.tx_id().to_string();
        self.record_workflow_started(&workflow_id).await;

        let execution_order = self.workflow.execution_order()?;

        for (position, task_id) in execution_order.iter().enumerate() {
            if let Some(token) = self.cancellation_token() {
                if token.is_cancelled() {
                    self.record_workflow_cancelled(&workflow_id).await;
                    let completed: Vec<TaskId> = self.completed_tasks.iter().cloned().collect();
                    return Ok(WorkflowResult {
                        success: false,
                        completed_tasks: completed,
                        failed_tasks: Vec::new(),
                        error: Some("Workflow cancelled".to_string()),
                        rollback_report: None,
                    });
                }
            }

            let task_result = match self.execute_task(&workflow_id, task_id).await {
                Ok(result) => result,
                Err(e) => {
                    return self
                        .handle_task_failure(&workflow_id, task_id, &e.to_string())
                        .await;
                }
            };

            if let TaskResult::Failed(msg) = &task_result {
                self.completed_tasks.remove(task_id);
                return self.handle_task_failure(&workflow_id, task_id, msg).await;
            }

            if let Some(validation_config) = &self.validation_config {
                let node_idx =
                    self.workflow.task_map.get(task_id).ok_or_else(|| {
                        crate::workflow::WorkflowError::TaskNotFound(task_id.clone())
                    })?;
                let task_node = self.workflow.graph.node_weight(*node_idx).ok_or_else(|| {
                    crate::workflow::WorkflowError::TaskFailed(
                        "Node index exists but node not found in graph".to_string(),
                    )
                })?;
                let task_name = task_node.name.clone();

                let validation = validate_checkpoint(&task_result, validation_config);

                let _ = self
                    .audit_log
                    .record(crate::audit::AuditEvent::WorkflowTaskCompleted {
                        timestamp: Utc::now(),
                        workflow_id: workflow_id.to_string(),
                        task_id: task_id.to_string(),
                        task_name: task_name.clone(),
                        result: format!("Validation: {:?}", validation.status),
                    })
                    .await;

                if !can_proceed(&validation) {
                    let completed: Vec<TaskId> = self.completed_tasks.iter().cloned().collect();

                    if requires_rollback(&validation) {
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

                        return Ok(WorkflowResult::new_failed_with_rollback(
                            completed,
                            task_id.clone(),
                            validation.message,
                            rollback_report,
                        ));
                    } else {
                        return Ok(WorkflowResult::new_failed(
                            completed,
                            task_id.clone(),
                            validation.message,
                        ));
                    }
                }

                if matches!(
                    validation.status,
                    crate::workflow::checkpoint::ValidationStatus::Warning
                ) {
                    eprintln!("Warning: {} - {}", task_id, validation.message);
                }
            }

            self.create_checkpoint(&workflow_id, position).await;
        }

        self.record_workflow_completed(&workflow_id).await;
        let completed: Vec<TaskId> = self.completed_tasks.iter().cloned().collect();
        Ok(WorkflowResult::new(completed))
    }

    pub async fn execute_with_validations(
        &mut self,
    ) -> Result<WorkflowResult, crate::workflow::WorkflowError> {
        if self.validation_config.is_none() {
            self.validation_config =
                Some(crate::workflow::checkpoint::ValidationCheckpoint::default());
        }
        self.execute().await
    }

    pub async fn execute_with_timeout(
        &mut self,
    ) -> Result<WorkflowResult, crate::workflow::WorkflowError> {
        if let Some(config) = &self.timeout_config {
            if let Some(workflow_timeout) = config.workflow_timeout {
                let duration = workflow_timeout.duration();
                match tokio::time::timeout(duration, self.execute()).await {
                    Ok(result) => result,
                    Err(_) => {
                        let workflow_id = self.audit_log.tx_id().to_string();
                        self.record_workflow_timeout(&workflow_id, duration.as_secs())
                            .await;
                        Err(crate::workflow::WorkflowError::Timeout(
                            TimeoutError::WorkflowTimeout { timeout: duration },
                        ))
                    }
                }
            } else {
                self.execute().await
            }
        } else {
            self.execute().await
        }
    }
}

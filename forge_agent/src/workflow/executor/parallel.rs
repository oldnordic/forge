use super::{WorkflowExecutor, WorkflowResult};
use crate::workflow::deadlock::DeadlockError;
use crate::workflow::state::TaskStatus;
use crate::workflow::task::TaskId;
use chrono::Utc;
use std::sync::Arc;

impl WorkflowExecutor {
    pub async fn execute_parallel(
        &mut self,
    ) -> Result<WorkflowResult, crate::workflow::WorkflowError> {
        use crate::workflow::state::{ConcurrentState, TaskSummary, WorkflowState};
        use tokio::task::JoinSet;

        let workflow_id = self.audit_log.tx_id().to_string();
        self.record_workflow_started(&workflow_id).await;

        self.check_for_deadlocks_before_execution(&workflow_id)
            .await?;

        let initial_state = WorkflowState::new(&workflow_id)
            .with_status(crate::workflow::state::WorkflowStatus::Running);
        let concurrent_state = std::sync::Arc::new(ConcurrentState::new(initial_state));

        let execution_layers = self.workflow.execution_layers()?;

        for (layer_index, layer) in execution_layers.iter().enumerate() {
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

            let _ = self
                .audit_log
                .record(crate::audit::AuditEvent::WorkflowTaskParallelStarted {
                    timestamp: Utc::now(),
                    workflow_id: workflow_id.to_string(),
                    layer_index,
                    task_count: layer.len(),
                })
                .await;

            let mut set: JoinSet<Result<(TaskId, String), crate::workflow::WorkflowError>> =
                JoinSet::new();

            for task_id in layer {
                let node_idx =
                    self.workflow.task_map.get(task_id).ok_or_else(|| {
                        crate::workflow::WorkflowError::TaskNotFound(task_id.clone())
                    })?;

                let task_node = self.workflow.graph.node_weight(*node_idx).ok_or_else(|| {
                    crate::workflow::WorkflowError::TaskFailed(
                        "Node index exists but node not found in graph".to_string(),
                    )
                })?;

                let task_arc = std::sync::Arc::clone(task_node.task());
                let task_id_clone = task_id.clone();
                let task_name = task_node.name.clone();
                let workflow_id_clone = workflow_id.clone();
                let cancellation_token = self.cancellation_token();
                let timeout_config = self.timeout_config.clone();
                let tool_registry = self.tool_registry.clone();
                let audit_log = self.audit_log.clone();
                let concurrent_state_clone = std::sync::Arc::clone(&concurrent_state);

                set.spawn(async move {
                    let mut task_audit_log = audit_log.clone();
                    let _ = task_audit_log
                        .record(crate::audit::AuditEvent::WorkflowTaskStarted {
                            timestamp: Utc::now(),
                            workflow_id: workflow_id_clone.clone(),
                            task_id: task_id_clone.to_string(),
                            task_name: task_name.clone(),
                        })
                        .await;

                    {
                        let _state_reader = concurrent_state_clone.read();
                    }

                    let mut context = if let Some(token) = cancellation_token {
                        crate::workflow::task::TaskContext::new(
                            &workflow_id_clone,
                            task_id_clone.clone(),
                        )
                        .with_cancellation_token(token)
                    } else {
                        crate::workflow::task::TaskContext::new(
                            &workflow_id_clone,
                            task_id_clone.clone(),
                        )
                    };

                    if let Some(config) = &timeout_config {
                        if let Some(task_timeout) = config.task_timeout {
                            context = context.with_task_timeout(task_timeout.duration());
                        }
                    }

                    if let Some(ref registry) = tool_registry {
                        context = context.with_tool_registry(Arc::clone(registry));
                    }

                    context = context.with_audit_log(task_audit_log.clone());

                    let result = task_arc
                        .execute(&context)
                        .await
                        .map_err(|e| crate::workflow::WorkflowError::TaskFailed(e.to_string()));

                    match result {
                        Ok(_) => Ok((task_id_clone, task_name)),
                        Err(e) => Err(e),
                    }
                });
            }

            let (layer_succeeded, failed_task, error_message): (
                bool,
                Option<TaskId>,
                Option<String>,
            ) = if let Some(timeout) = self.deadlock_timeout {
                let layer_result = tokio::time::timeout(timeout, async {
                    let mut layer_succeeded = true;
                    let failed_task: Option<TaskId> = None;
                    let mut error_message: Option<String> = None;

                    while let Some(result) = set.join_next().await {
                        match result {
                            Ok(Ok((task_id, task_name))) => {
                                self.completed_tasks.insert(task_id.clone());
                                {
                                    let mut state = concurrent_state.write();
                                    state.completed_tasks.push(TaskSummary::new(
                                        task_id.as_str(),
                                        &task_name,
                                        TaskStatus::Completed,
                                    ));
                                }
                                self.record_task_completed(&workflow_id, &task_id, &task_name)
                                    .await;
                            }
                            Ok(Err(_e)) => {
                                layer_succeeded = false;
                                error_message = Some("Task execution failed".to_string());
                            }
                            Err(_e) => {
                                layer_succeeded = false;
                                error_message = Some("Task panicked".to_string());
                            }
                        }
                    }

                    (layer_succeeded, failed_task, error_message)
                })
                .await;

                match layer_result {
                    Ok(result) => result,
                    Err(_) => {
                        let timeout_secs = timeout.as_secs();
                        self.record_deadlock_timeout(&workflow_id, layer_index, timeout_secs)
                            .await;
                        return Err(DeadlockError::ResourceDeadlock(format!(
                            "Layer {} exceeded deadlock timeout of {} seconds",
                            layer_index, timeout_secs
                        ))
                        .into());
                    }
                }
            } else {
                let mut layer_succeeded = true;
                let failed_task: Option<TaskId> = None;
                let mut error_message: Option<String> = None;

                while let Some(result) = set.join_next().await {
                    match result {
                        Ok(Ok((task_id, task_name))) => {
                            self.completed_tasks.insert(task_id.clone());
                            {
                                let mut state = concurrent_state.write();
                                state.completed_tasks.push(TaskSummary::new(
                                    task_id.as_str(),
                                    &task_name,
                                    TaskStatus::Completed,
                                ));
                            }
                            self.record_task_completed(&workflow_id, &task_id, &task_name)
                                .await;
                        }
                        Ok(Err(_e)) => {
                            layer_succeeded = false;
                            error_message = Some("Task execution failed".to_string());
                        }
                        Err(_e) => {
                            layer_succeeded = false;
                            error_message = Some("Task panicked".to_string());
                        }
                    }
                }

                (layer_succeeded, failed_task, error_message)
            };

            let _ = self
                .audit_log
                .record(crate::audit::AuditEvent::WorkflowTaskParallelCompleted {
                    timestamp: Utc::now(),
                    workflow_id: workflow_id.to_string(),
                    layer_index,
                    task_count: layer.len(),
                })
                .await;

            if !layer_succeeded {
                let completed: Vec<TaskId> = self.completed_tasks.iter().cloned().collect();
                let failed_id = failed_task.unwrap_or_else(|| {
                    layer
                        .first()
                        .cloned()
                        .unwrap_or_else(|| TaskId::new("unknown"))
                });

                let rollback_set = self
                    .rollback_engine
                    .find_rollback_set(&self.workflow, &failed_id, self.rollback_strategy)
                    .map_err(|_err| {
                        crate::workflow::WorkflowError::TaskNotFound(failed_id.clone())
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
                        crate::workflow::dag::WorkflowError::TaskNotFound(failed_id.clone())
                    })?;

                let error_msg = error_message
                    .clone()
                    .unwrap_or_else(|| "Layer execution failed".to_string());
                self.record_workflow_failed(&workflow_id, &failed_id, &error_msg)
                    .await;

                return Ok(WorkflowResult::new_failed_with_rollback(
                    completed,
                    failed_id,
                    error_msg,
                    rollback_report,
                ));
            }

            self.create_checkpoint(&workflow_id, layer_index).await;
        }

        {
            let mut state = concurrent_state.write();
            state.status = crate::workflow::state::WorkflowStatus::Completed;
        }

        self.record_workflow_completed(&workflow_id).await;
        let completed: Vec<TaskId> = self.completed_tasks.iter().cloned().collect();
        Ok(WorkflowResult::new(completed))
    }
}

use super::WorkflowExecutor;
use crate::workflow::task::TaskId;
use chrono::Utc;

impl WorkflowExecutor {
    pub(in crate::workflow::executor) async fn record_workflow_started(
        &mut self,
        workflow_id: &str,
    ) {
        let _ = self
            .audit_log
            .record(crate::audit::AuditEvent::WorkflowStarted {
                timestamp: Utc::now(),
                workflow_id: workflow_id.to_string(),
                task_count: self.workflow.task_count(),
            })
            .await;
    }

    pub(in crate::workflow::executor) async fn record_task_started(
        &mut self,
        workflow_id: &str,
        task_id: &TaskId,
        task_name: &str,
    ) {
        let _ = self
            .audit_log
            .record(crate::audit::AuditEvent::WorkflowTaskStarted {
                timestamp: Utc::now(),
                workflow_id: workflow_id.to_string(),
                task_id: task_id.to_string(),
                task_name: task_name.to_string(),
            })
            .await;
    }

    pub(in crate::workflow::executor) async fn record_task_completed(
        &mut self,
        workflow_id: &str,
        task_id: &TaskId,
        task_name: &str,
    ) {
        let _ = self
            .audit_log
            .record(crate::audit::AuditEvent::WorkflowTaskCompleted {
                timestamp: Utc::now(),
                workflow_id: workflow_id.to_string(),
                task_id: task_id.to_string(),
                task_name: task_name.to_string(),
                result: "Success".to_string(),
            })
            .await;
    }

    pub(in crate::workflow::executor) async fn record_workflow_failed(
        &mut self,
        workflow_id: &str,
        task_id: &TaskId,
        error: &str,
    ) {
        let _ = self
            .audit_log
            .record(crate::audit::AuditEvent::WorkflowTaskFailed {
                timestamp: Utc::now(),
                workflow_id: workflow_id.to_string(),
                task_id: task_id.to_string(),
                task_name: task_id.to_string(),
                error: error.to_string(),
            })
            .await;

        let _ = self
            .audit_log
            .record(crate::audit::AuditEvent::WorkflowCompleted {
                timestamp: Utc::now(),
                workflow_id: workflow_id.to_string(),
                total_tasks: self.workflow.task_count(),
                completed_tasks: self.completed_tasks.len(),
            })
            .await;
    }

    pub(in crate::workflow::executor) async fn record_workflow_cancelled(
        &mut self,
        workflow_id: &str,
    ) {
        let _ = self
            .audit_log
            .record(crate::audit::AuditEvent::WorkflowCancelled {
                timestamp: Utc::now(),
                workflow_id: workflow_id.to_string(),
            })
            .await;

        let _ = self
            .audit_log
            .record(crate::audit::AuditEvent::WorkflowCompleted {
                timestamp: Utc::now(),
                workflow_id: workflow_id.to_string(),
                total_tasks: self.workflow.task_count(),
                completed_tasks: self.completed_tasks.len(),
            })
            .await;
    }

    pub(in crate::workflow::executor) async fn record_workflow_timeout(
        &mut self,
        workflow_id: &str,
        _timeout_secs: u64,
    ) {
        let _ = self
            .audit_log
            .record(crate::audit::AuditEvent::WorkflowCompleted {
                timestamp: Utc::now(),
                workflow_id: workflow_id.to_string(),
                total_tasks: self.workflow.task_count(),
                completed_tasks: self.completed_tasks.len(),
            })
            .await;
    }

    pub(in crate::workflow::executor) async fn check_for_deadlocks_before_execution(
        &mut self,
        workflow_id: &str,
    ) -> Result<(), crate::workflow::WorkflowError> {
        let detector = crate::workflow::deadlock::DeadlockDetector::new();

        match detector.validate_workflow(&self.workflow) {
            Ok(warnings) => {
                let warning_strings: Vec<String> =
                    warnings.iter().map(|w| w.description()).collect();

                let _ = self
                    .audit_log
                    .record(crate::audit::AuditEvent::WorkflowDeadlockCheck {
                        timestamp: Utc::now(),
                        workflow_id: workflow_id.to_string(),
                        has_cycles: false,
                        warnings: warning_strings.clone(),
                    })
                    .await;

                for warning in &warning_strings {
                    eprintln!("Deadlock warning: {}", warning);
                }

                Ok(())
            }
            Err(crate::workflow::deadlock::DeadlockError::DependencyCycle(cycle)) => {
                let _ = self
                    .audit_log
                    .record(crate::audit::AuditEvent::WorkflowDeadlockCheck {
                        timestamp: Utc::now(),
                        workflow_id: workflow_id.to_string(),
                        has_cycles: true,
                        warnings: vec![format!("Dependency cycle detected: {:?}", cycle)],
                    })
                    .await;

                Err(crate::workflow::deadlock::DeadlockError::DependencyCycle(cycle).into())
            }
            Err(e) => Err(e.into()),
        }
    }

    pub(in crate::workflow::executor) async fn record_deadlock_timeout(
        &mut self,
        workflow_id: &str,
        layer_index: usize,
        timeout_secs: u64,
    ) {
        let _ = self
            .audit_log
            .record(crate::audit::AuditEvent::WorkflowDeadlockTimeout {
                timestamp: Utc::now(),
                workflow_id: workflow_id.to_string(),
                layer_index,
                timeout_secs,
            })
            .await;
    }

    pub(in crate::workflow::executor) async fn record_task_timeout(
        &mut self,
        workflow_id: &str,
        task_id: &TaskId,
        task_name: &str,
        timeout_secs: u64,
    ) {
        let _ = self
            .audit_log
            .record(crate::audit::AuditEvent::WorkflowTaskTimedOut {
                timestamp: Utc::now(),
                workflow_id: workflow_id.to_string(),
                task_id: task_id.to_string(),
                task_name: task_name.to_string(),
                timeout_secs,
            })
            .await;
    }

    pub(in crate::workflow::executor) async fn record_workflow_completed(
        &mut self,
        workflow_id: &str,
    ) {
        let _ = self
            .audit_log
            .record(crate::audit::AuditEvent::WorkflowCompleted {
                timestamp: Utc::now(),
                workflow_id: workflow_id.to_string(),
                total_tasks: self.workflow.task_count(),
                completed_tasks: self.completed_tasks.len(),
            })
            .await;
    }
}

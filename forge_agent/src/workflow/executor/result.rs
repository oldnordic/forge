use crate::workflow::rollback::RollbackReport;
use crate::workflow::task::TaskId;

/// Result of workflow execution.
#[derive(Clone, Debug)]
pub struct WorkflowResult {
    pub success: bool,
    pub completed_tasks: Vec<TaskId>,
    pub failed_tasks: Vec<TaskId>,
    pub error: Option<String>,
    pub rollback_report: Option<RollbackReport>,
}

impl WorkflowResult {
    pub(in crate::workflow::executor) fn new(completed_tasks: Vec<TaskId>) -> Self {
        Self {
            success: true,
            completed_tasks,
            failed_tasks: Vec::new(),
            error: None,
            rollback_report: None,
        }
    }

    pub(in crate::workflow::executor) fn new_failed(
        completed_tasks: Vec<TaskId>,
        failed_task: TaskId,
        error: String,
    ) -> Self {
        Self {
            success: false,
            completed_tasks,
            failed_tasks: vec![failed_task],
            error: Some(error),
            rollback_report: None,
        }
    }

    pub(in crate::workflow::executor) fn new_failed_with_rollback(
        completed_tasks: Vec<TaskId>,
        failed_task: TaskId,
        error: String,
        rollback_report: RollbackReport,
    ) -> Self {
        Self {
            success: false,
            completed_tasks,
            failed_tasks: vec![failed_task],
            error: Some(error),
            rollback_report: Some(rollback_report),
        }
    }
}

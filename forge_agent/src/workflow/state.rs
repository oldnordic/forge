//! Workflow state inspection API.
//!
//! Provides runtime state inspection for workflows including task status,
//! progress tracking, and serialization for external monitoring.

use crate::workflow::dag::TaskNode;
use crate::workflow::executor::WorkflowExecutor;
use crate::workflow::task::TaskId;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Status of a workflow execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowStatus {
    /// Workflow is pending execution
    Pending,
    /// Workflow is currently running
    Running,
    /// Workflow completed successfully
    Completed,
    /// Workflow failed
    Failed,
    /// Workflow was rolled back
    RolledBack,
}

/// Status of an individual task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Task is pending execution
    Pending,
    /// Task is currently running
    Running,
    /// Task completed successfully
    Completed,
    /// Task failed
    Failed,
    /// Task was skipped
    Skipped,
}

/// Summary of a task's state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskSummary {
    /// Task identifier
    pub id: String,
    /// Task name
    pub name: String,
    /// Current task status
    pub status: TaskStatus,
}

impl TaskSummary {
    /// Creates a new TaskSummary.
    pub fn new(id: impl Into<String>, name: impl Into<String>, status: TaskStatus) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            status,
        }
    }
}

/// Snapshot of workflow execution state.
///
/// Provides a complete view of the workflow's current execution status
/// including completed, pending, and failed tasks.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowState {
    /// Workflow identifier
    pub workflow_id: String,
    /// Current workflow status
    pub status: WorkflowStatus,
    /// Currently executing task (if any)
    pub current_task: Option<TaskSummary>,
    /// Tasks that have completed
    pub completed_tasks: Vec<TaskSummary>,
    /// Tasks that are pending execution
    pub pending_tasks: Vec<TaskSummary>,
    /// Tasks that have failed
    pub failed_tasks: Vec<TaskSummary>,
}

impl WorkflowState {
    /// Creates a new WorkflowState.
    pub fn new(workflow_id: impl Into<String>) -> Self {
        Self {
            workflow_id: workflow_id.into(),
            status: WorkflowStatus::Pending,
            current_task: None,
            completed_tasks: Vec::new(),
            pending_tasks: Vec::new(),
            failed_tasks: Vec::new(),
        }
    }

    /// Sets the workflow status.
    pub fn with_status(mut self, status: WorkflowStatus) -> Self {
        self.status = status;
        self
    }

    /// Adds a completed task.
    pub fn with_completed_task(mut self, task: TaskSummary) -> Self {
        self.completed_tasks.push(task);
        self
    }

    /// Adds a pending task.
    pub fn with_pending_task(mut self, task: TaskSummary) -> Self {
        self.pending_tasks.push(task);
        self
    }

    /// Adds a failed task.
    pub fn with_failed_task(mut self, task: TaskSummary) -> Self {
        self.failed_tasks.push(task);
        self
    }

    /// Sets the current task.
    pub fn with_current_task(mut self, task: TaskSummary) -> Self {
        self.current_task = Some(task);
        self
    }
}

impl WorkflowExecutor {
    /// Returns a snapshot of the current workflow state.
    ///
    /// This method provides a complete view of the workflow's execution
    /// status including all tasks and their current states.
    ///
    /// # Returns
    ///
    /// A `WorkflowState` snapshot containing task summaries and status.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut executor = WorkflowExecutor::new(workflow);
    /// executor.execute().await?;
    /// let state = executor.state();
    /// println!("Status: {:?}", state.status);
    /// println!("Completed: {}", state.completed_tasks.len());
    /// ```
    pub fn state(&self) -> WorkflowState {
        // Determine workflow status based on completion state
        let status = if self.failed_tasks.is_empty() && self.completed_tasks.is_empty() {
            WorkflowStatus::Pending
        } else if !self.failed_tasks.is_empty() {
            WorkflowStatus::Failed
        } else if self.completed_tasks.len() == self.workflow.task_count() {
            WorkflowStatus::Completed
        } else {
            WorkflowStatus::Running
        };

        // Build completed task summaries
        let completed_tasks: Vec<TaskSummary> = self
            .completed_tasks
            .iter()
            .map(|id| {
                let name = self.get_task_name(id)
                    .unwrap_or_else(|| "Unknown".to_string());
                TaskSummary::new(
                    id.as_str(),
                    &name,
                    TaskStatus::Completed,
                )
            })
            .collect();

        // Build pending task summaries
        let pending_task_ids: HashSet<_> = self.workflow.task_ids().into_iter().collect();
        let completed_ids: HashSet<_> = self.completed_tasks.iter().cloned().collect();
        let failed_ids: HashSet<_> = self.failed_tasks.iter().cloned().collect();

        let pending_tasks: Vec<TaskSummary> = pending_task_ids
            .difference(&completed_ids)
            .filter(|id| !failed_ids.contains(id))
            .map(|id| {
                let name = self.get_task_name(id)
                    .unwrap_or_else(|| "Unknown".to_string());
                TaskSummary::new(
                    id.as_str(),
                    &name,
                    TaskStatus::Pending,
                )
            })
            .collect();

        // Build failed task summaries
        let failed_tasks: Vec<TaskSummary> = self
            .failed_tasks
            .iter()
            .map(|id| {
                let name = self.get_task_name(id)
                    .unwrap_or_else(|| "Unknown".to_string());
                TaskSummary::new(
                    id.as_str(),
                    &name,
                    TaskStatus::Failed,
                )
            })
            .collect();

        WorkflowState {
            workflow_id: format!("workflow-{:?}", self.audit_log.tx_id()),
            status,
            current_task: None,
            completed_tasks,
            pending_tasks,
            failed_tasks,
        }
    }

    /// Helper method to get task name from workflow.
    fn get_task_name(&self, id: &TaskId) -> Option<String> {
        self.workflow.task_name(id)
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
    }

    #[test]
    fn test_task_summary_creation() {
        let summary = TaskSummary::new("task-1", "Task 1", TaskStatus::Pending);
        assert_eq!(summary.id, "task-1");
        assert_eq!(summary.name, "Task 1");
        assert_eq!(summary.status, TaskStatus::Pending);
    }

    #[test]
    fn test_workflow_state_creation() {
        let state = WorkflowState::new("workflow-1");
        assert_eq!(state.workflow_id, "workflow-1");
        assert_eq!(state.status, WorkflowStatus::Pending);
        assert!(state.completed_tasks.is_empty());
        assert!(state.pending_tasks.is_empty());
        assert!(state.failed_tasks.is_empty());
    }

    #[test]
    fn test_workflow_state_builder() {
        let state = WorkflowState::new("workflow-1")
            .with_status(WorkflowStatus::Running)
            .with_completed_task(TaskSummary::new("task-1", "Task 1", TaskStatus::Completed))
            .with_pending_task(TaskSummary::new("task-2", "Task 2", TaskStatus::Pending));

        assert_eq!(state.status, WorkflowStatus::Running);
        assert_eq!(state.completed_tasks.len(), 1);
        assert_eq!(state.pending_tasks.len(), 1);
    }

    #[tokio::test]
    async fn test_workflow_state_snapshot() {
        use crate::workflow::executor::WorkflowExecutor;

        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));
        workflow.add_task(Box::new(MockTask::new("task-2", "Task 2")));
        workflow.add_task(Box::new(MockTask::new("task-3", "Task 3")));

        let executor = WorkflowExecutor::new(workflow);
        let state = executor.state();

        // Before execution, all tasks should be pending
        assert_eq!(state.status, WorkflowStatus::Pending);
        assert_eq!(state.pending_tasks.len(), 3);
        assert_eq!(state.completed_tasks.len(), 0);
    }

    #[tokio::test]
    async fn test_progress_calculation() {
        use crate::workflow::executor::WorkflowExecutor;

        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("task-1", "Task 1")));
        workflow.add_task(Box::new(MockTask::new("task-2", "Task 2")));
        workflow.add_task(Box::new(MockTask::new("task-3", "Task 3")));
        workflow.add_task(Box::new(MockTask::new("task-4", "Task 4")));

        let executor = WorkflowExecutor::new(workflow);

        // Before execution: 0/4 = 0.0
        assert_eq!(executor.progress(), 0.0);
    }

    #[test]
    fn test_progress_empty_workflow() {
        use crate::workflow::executor::WorkflowExecutor;

        let workflow = Workflow::new();
        let executor = WorkflowExecutor::new(workflow);

        // Empty workflow: 0 tasks = 0.0 progress
        assert_eq!(executor.progress(), 0.0);
    }

    #[tokio::test]
    async fn test_state_serialization() {
        let state = WorkflowState::new("workflow-1")
            .with_status(WorkflowStatus::Completed)
            .with_completed_task(TaskSummary::new("task-1", "Task 1", TaskStatus::Completed));

        // Serialize to JSON
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("workflow-1"));
        assert!(json.contains("Completed"));

        // Deserialize back
        let deserialized: WorkflowState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.workflow_id, "workflow-1");
        assert_eq!(deserialized.status, WorkflowStatus::Completed);
        assert_eq!(deserialized.completed_tasks.len(), 1);
    }

    #[test]
    fn test_task_status_equality() {
        assert_eq!(TaskStatus::Pending, TaskStatus::Pending);
        assert_ne!(TaskStatus::Pending, TaskStatus::Running);
        assert_eq!(WorkflowStatus::Completed, WorkflowStatus::Completed);
        assert_ne!(WorkflowStatus::Completed, WorkflowStatus::Failed);
    }
}

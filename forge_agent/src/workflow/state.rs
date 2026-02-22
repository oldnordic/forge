//! Workflow state inspection API.
//!
//! Provides runtime state inspection for workflows including task status,
//! progress tracking, and serialization for external monitoring.
//!
//! # Thread Safety
//!
//! This module provides [`ConcurrentState`] for thread-safe state access during
//! parallel workflow execution. It uses `Arc<RwLock<T>>` to allow:
//! - Multiple concurrent reads (tasks checking state)
//! - Exclusive writes (executor updating task status)
//!
//! ## Thread-Safety Audit (Task 1 of Phase 12-02)
//!
//! **Findings:**
//! - `WorkflowState` uses `Vec<TaskSummary>` - NOT thread-safe
//! - `WorkflowExecutor.completed_tasks: HashSet<TaskId>` - NOT thread-safe
//! - `WorkflowExecutor.failed_tasks: Vec<TaskId>` - NOT thread-safe
//!
//! **Decision:** Use `Arc<RwLock<T>>` instead of:
//! - `Arc<Mutex<T>>`: RwLock allows concurrent reads
//! - `dashmap`: Not needed - we don't require per-key concurrent access
//!
//! **Data Race Identified:** In `execute_parallel()`, line 850:
//! ```rust
//! self.completed_tasks.insert(task_id.clone());  // DATA RACE!
//! ```
//! Fixed by using `ConcurrentState` for all state mutations.

use crate::workflow::dag::TaskNode;
use crate::workflow::executor::WorkflowExecutor;
use crate::workflow::task::TaskId;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::{Arc, RwLock};

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

impl TaskStatus {
    /// Creates TaskStatus from a parallel execution result.
    pub(crate) fn from_parallel_result(success: bool) -> Self {
        if success {
            TaskStatus::Completed
        } else {
            TaskStatus::Failed
        }
    }
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

/// Thread-safe wrapper for workflow state during parallel execution.
///
/// Uses `Arc<RwLock<T>>` to allow multiple concurrent reads with exclusive writes.
/// This is optimal for workflow execution where:
/// - Multiple tasks may read state concurrently
/// - Only the executor writes state updates
///
/// # Example
///
/// ```ignore
/// let state = ConcurrentState::new(WorkflowState::new("workflow-1"));
///
/// // Concurrent reads (tasks checking state)
/// {
///     let reader = state.read().unwrap();
///     assert_eq!(reader.status, WorkflowStatus::Running);
/// }
///
/// // Exclusive write (executor updating state)
/// {
///     let mut writer = state.write().unwrap();
///     writer.status = WorkflowStatus::Completed;
/// }
/// ```
#[derive(Clone)]
pub struct ConcurrentState {
    /// Inner state wrapped in Arc<RwLock for thread-safe access
    inner: Arc<RwLock<WorkflowState>>,
}

impl ConcurrentState {
    /// Creates a new ConcurrentState from a WorkflowState.
    pub fn new(state: WorkflowState) -> Self {
        Self {
            inner: Arc::new(RwLock::new(state)),
        }
    }

    /// Acquires a read lock, allowing concurrent access from multiple readers.
    ///
    /// # Returns
    ///
    /// A `RwLockReadGuard` that provides immutable access to the state.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned (another thread panicked while holding the lock).
    pub fn read(&self) -> Result<std::sync::RwLockReadGuard<WorkflowState>, std::sync::PoisonError<std::sync::RwLockReadGuard<WorkflowState>>> {
        self.inner.read()
    }

    /// Acquires a write lock, providing exclusive mutable access.
    ///
    /// # Returns
    ///
    /// A `RwLockWriteGuard` that provides mutable access to the state.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned (another thread panicked while holding the lock).
    pub fn write(&self) -> Result<std::sync::RwLockWriteGuard<WorkflowState>, std::sync::PoisonError<std::sync::RwLockWriteGuard<WorkflowState>>> {
        self.inner.write()
    }

    /// Attempts to acquire a read lock without blocking.
    ///
    /// # Returns
    ///
    /// - `Some(guard)` if the lock was acquired immediately
    /// - `None` if the lock is held by a writer
    pub fn try_read(&self) -> Option<std::sync::RwLockReadGuard<'_, WorkflowState>> {
        self.inner.try_read().ok()
    }

    /// Attempts to acquire a write lock without blocking.
    ///
    /// # Returns
    ///
    /// - `Some(guard)` if the lock was acquired immediately
    /// - `None` if the lock is held by another reader or writer
    pub fn try_write(&self) -> Option<std::sync::RwLockWriteGuard<'_, WorkflowState>> {
        self.inner.try_write().ok()
    }

    /// Returns the number of strong references to the inner state.
    ///
    /// Useful for debugging to see how many clones exist.
    pub fn ref_count(&self) -> usize {
        Arc::strong_count(&self.inner)
    }
}

// SAFETY: ConcurrentState is Send + Sync because:
// - Arc<T> is Send + Sync when T: Send + Sync
// - RwLock<T> is Send + Sync when T: Send
// - WorkflowState is Send (all fields are Send)
unsafe impl Send for ConcurrentState {}
unsafe impl Sync for ConcurrentState {}

#[cfg(test)]
mod concurrent_state_tests {
    use super::*;
    use std::sync::Barrier;
    use tokio::task::JoinSet;

    #[test]
    fn test_concurrent_state_creation() {
        let state = WorkflowState::new("workflow-1");
        let concurrent = ConcurrentState::new(state);

        let reader = concurrent.read().unwrap();
        assert_eq!(reader.workflow_id, "workflow-1");
        assert_eq!(reader.status, WorkflowStatus::Pending);
    }

    #[test]
    fn test_concurrent_state_clone_is_cheap() {
        let state = WorkflowState::new("workflow-1");
        let concurrent = ConcurrentState::new(state);

        // Clone is cheap (just Arc increment)
        let cloned = concurrent.clone();
        assert_eq!(concurrent.ref_count(), 2);

        let cloned2 = cloned.clone();
        assert_eq!(concurrent.ref_count(), 3);
    }

    #[test]
    fn test_concurrent_read_write() {
        let state = WorkflowState::new("workflow-1");
        let concurrent = ConcurrentState::new(state);

        // Read initial state
        {
            let reader = concurrent.read().unwrap();
            assert_eq!(reader.status, WorkflowStatus::Pending);
        }

        // Write new state
        {
            let mut writer = concurrent.write().unwrap();
            writer.status = WorkflowStatus::Completed;
        }

        // Read updated state
        {
            let reader = concurrent.read().unwrap();
            assert_eq!(reader.status, WorkflowStatus::Completed);
        }
    }

    #[test]
    fn test_try_read_write() {
        let state = WorkflowState::new("workflow-1");
        let concurrent = ConcurrentState::new(state);

        // Try read should succeed
        assert!(concurrent.try_read().is_some());

        // Try write should succeed
        assert!(concurrent.try_write().is_some());
    }

    #[tokio::test]
    async fn test_concurrent_state_thread_safety() {
        let state = WorkflowState::new("workflow-1").with_status(WorkflowStatus::Running);
        let concurrent = Arc::new(ConcurrentState::new(state));
        let barrier = Arc::new(Barrier::new(3)); // 2 readers + 1 writer

        let mut handles = JoinSet::new();

        // Spawn reader 1
        let concurrent1 = Arc::clone(&concurrent);
        let barrier1 = Arc::clone(&barrier);
        handles.spawn(async move {
            barrier1.wait();
            let reader = concurrent1.read().unwrap();
            assert_eq!(reader.workflow_id, "workflow-1");
        });

        // Spawn reader 2
        let concurrent2 = Arc::clone(&concurrent);
        let barrier2 = Arc::clone(&barrier);
        handles.spawn(async move {
            barrier2.wait();
            let reader = concurrent2.read().unwrap();
            assert_eq!(reader.status, WorkflowStatus::Running);
        });

        // Spawn writer
        let concurrent3 = Arc::clone(&concurrent);
        let barrier3 = Arc::clone(&barrier);
        handles.spawn(async move {
            barrier3.wait();
            // Small delay to let readers read first
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            let mut writer = concurrent3.write().unwrap();
            writer.status = WorkflowStatus::Completed;
        });

        // Wait for all tasks
        while let Some(result) = handles.join_next().await {
            result.expect("Task should complete successfully");
        }

        // Verify final state
        let reader = concurrent.read().unwrap();
        assert_eq!(reader.status, WorkflowStatus::Completed);
    }

    #[tokio::test]
    async fn test_concurrent_state_stress_test() {
        let state = WorkflowState::new("workflow-stress");
        let concurrent = Arc::new(ConcurrentState::new(state));

        let mut handles = JoinSet::new();

        // Spawn 10 concurrent readers/writers
        for i in 0..10 {
            let concurrent_clone = Arc::clone(&concurrent);
            handles.spawn(async move {
                // Read (guard dropped before await)
                {
                    let _reader = concurrent_clone.read().unwrap();
                }

                // Small delay
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;

                // Write (if even number)
                if i % 2 == 0 {
                    let mut writer = concurrent_clone.write().unwrap();
                    writer.completed_tasks.push(TaskSummary::new(
                        format!("task-{}", i),
                        format!("Task {}", i),
                        TaskStatus::Completed,
                    ));
                }
            });
        }

        // Wait for all tasks
        while let Some(result) = handles.join_next().await {
            result.expect("Task should complete successfully");
        }

        // Verify no corruption - should have 5 completed tasks (even numbers 0,2,4,6,8)
        let reader = concurrent.read().unwrap();
        assert_eq!(reader.completed_tasks.len(), 5);
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

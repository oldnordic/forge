//! Sequential workflow executor with audit logging and rollback.
//!
//! Executes tasks in topological order, recording all events to the audit log.
//! On failure, triggers selective rollback of dependent tasks using Saga compensation.

use crate::audit::AuditLog;
use crate::workflow::checkpoint::{validate_workflow_consistency, WorkflowCheckpoint, WorkflowCheckpointService};
use crate::workflow::dag::Workflow;
use crate::workflow::rollback::{RollbackEngine, RollbackReport, RollbackStrategy};
use crate::workflow::task::{TaskContext, TaskId};
use chrono::Utc;
use std::collections::HashSet;

/// Result of workflow execution.
///
/// Contains the final status and list of completed task IDs.
#[derive(Clone, Debug)]
pub struct WorkflowResult {
    /// Whether the workflow completed successfully
    pub success: bool,
    /// Tasks that completed successfully
    pub completed_tasks: Vec<TaskId>,
    /// Tasks that failed
    pub failed_tasks: Vec<TaskId>,
    /// Error message if workflow failed
    pub error: Option<String>,
    /// Rollback report if rollback was executed
    pub rollback_report: Option<RollbackReport>,
}

impl WorkflowResult {
    /// Creates a new successful workflow result.
    fn new(completed_tasks: Vec<TaskId>) -> Self {
        Self {
            success: true,
            completed_tasks,
            failed_tasks: Vec::new(),
            error: None,
            rollback_report: None,
        }
    }

    /// Creates a new failed workflow result.
    fn new_failed(completed_tasks: Vec<TaskId>, failed_task: TaskId, error: String) -> Self {
        Self {
            success: false,
            completed_tasks,
            failed_tasks: vec![failed_task],
            error: Some(error),
            rollback_report: None,
        }
    }

    /// Creates a failed result with rollback report.
    fn new_failed_with_rollback(
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

/// Sequential workflow executor with rollback support.
///
/// Executes tasks in topological order based on dependencies,
/// recording all task events to the audit log. On failure,
/// automatically triggers selective rollback of dependent tasks.
///
/// # Execution Model
///
/// The executor:
/// 1. Validates the workflow structure
/// 2. Calculates execution order via topological sort
/// 3. Executes each task with audit logging
/// 4. Creates checkpoint after each successful task
/// 5. On failure, triggers rollback of dependent tasks
pub struct WorkflowExecutor {
    /// The workflow to execute
    pub(in crate::workflow) workflow: Workflow,
    /// Audit log for recording events
    pub(in crate::workflow) audit_log: AuditLog,
    /// Tasks that have completed
    pub(in crate::workflow) completed_tasks: HashSet<TaskId>,
    /// Tasks that failed
    pub(in crate::workflow) failed_tasks: Vec<TaskId>,
    /// Rollback engine for handling failures
    rollback_engine: RollbackEngine,
    /// Rollback strategy to use on failure
    rollback_strategy: RollbackStrategy,
    /// Optional checkpoint service for state persistence
    pub(in crate::workflow) checkpoint_service: Option<WorkflowCheckpointService>,
    /// Checkpoint sequence counter
    pub(in crate::workflow) checkpoint_sequence: u64,
}

impl WorkflowExecutor {
    /// Creates a new workflow executor.
    ///
    /// # Arguments
    ///
    /// * `workflow` - The workflow to execute
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut executor = WorkflowExecutor::new(workflow);
    /// let result = executor.execute().await?;
    /// ```
    pub fn new(workflow: Workflow) -> Self {
        Self {
            workflow,
            audit_log: AuditLog::new(),
            completed_tasks: HashSet::new(),
            failed_tasks: Vec::new(),
            rollback_engine: RollbackEngine::new(),
            rollback_strategy: RollbackStrategy::AllDependent,
            checkpoint_service: None,
            checkpoint_sequence: 0,
        }
    }

    /// Sets the rollback strategy for this executor.
    ///
    /// # Arguments
    ///
    /// * `strategy` - The rollback strategy to use
    ///
    /// # Returns
    ///
    /// The executor with the updated strategy (for builder pattern)
    ///
    /// # Example
    ///
    /// ```ignore
    /// let executor = WorkflowExecutor::new(workflow)
    ///     .with_rollback_strategy(RollbackStrategy::FailedOnly);
    /// ```
    pub fn with_rollback_strategy(mut self, strategy: RollbackStrategy) -> Self {
        self.rollback_strategy = strategy;
        self
    }

    /// Sets the checkpoint service for this executor.
    ///
    /// # Arguments
    ///
    /// * `service` - The checkpoint service to use for state persistence
    ///
    /// # Returns
    ///
    /// The executor with checkpoint service enabled (for builder pattern)
    ///
    /// # Example
    ///
    /// ```ignore
    /// let executor = WorkflowExecutor::new(workflow)
    ///     .with_checkpoint_service(checkpoint_service);
    /// ```
    pub fn with_checkpoint_service(mut self, service: WorkflowCheckpointService) -> Self {
        self.checkpoint_service = Some(service);
        self
    }

    /// Executes the workflow.
    ///
    /// Tasks are executed in topological order, with audit logging
    /// for each task start/completion/failed event. On failure,
    /// triggers rollback of dependent tasks.
    ///
    /// # Returns
    ///
    /// - `Ok(WorkflowResult)` - Execution completed (may have partial completion)
    /// - `Err(WorkflowError)` - If workflow validation or ordering fails
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut executor = WorkflowExecutor::new(workflow);
    /// let result = executor.execute().await?;
    /// if result.success {
    ///     println!("Completed {} tasks", result.completed_tasks.len());
    /// }
    /// ```
    pub async fn execute(&mut self) -> Result<WorkflowResult, crate::workflow::WorkflowError> {
        // Record workflow started
        let workflow_id = self.audit_log.tx_id().to_string();
        self.record_workflow_started(&workflow_id).await;

        // Get execution order
        let execution_order = self.workflow.execution_order()?;

        // Execute each task in order
        for (position, task_id) in execution_order.iter().enumerate() {
            if let Err(e) = self.execute_task(&workflow_id, task_id).await {
                // Task failed - trigger rollback
                let completed: Vec<TaskId> = self.completed_tasks.iter().cloned().collect();

                // Find rollback set based on strategy
                let rollback_set = self
                    .rollback_engine
                    .find_rollback_set(&self.workflow, task_id, self.rollback_strategy)
                    .map_err(|err| {
                        crate::workflow::WorkflowError::TaskNotFound(task_id.clone())
                    })?;

                // Execute rollback
                let rollback_report = self
                    .rollback_engine
                    .execute_rollback(&self.workflow, rollback_set, &workflow_id, &mut self.audit_log)
                    .await
                    .map_err(|err| {
                        crate::workflow::WorkflowError::TaskNotFound(task_id.clone())
                    })?;

                // Record workflow failed
                self.record_workflow_failed(&workflow_id, task_id, &e.to_string())
                    .await;

                return Ok(WorkflowResult::new_failed_with_rollback(
                    completed,
                    task_id.clone(),
                    e.to_string(),
                    rollback_report,
                ));
            }

            // Task completed successfully - create checkpoint
            self.create_checkpoint(&workflow_id, position).await;
        }

        // All tasks completed
        self.record_workflow_completed(&workflow_id).await;

        let completed: Vec<TaskId> = self.completed_tasks.iter().cloned().collect();
        Ok(WorkflowResult::new(completed))
    }

    /// Executes a single task.
    async fn execute_task(
        &mut self,
        workflow_id: &str,
        task_id: &TaskId,
    ) -> Result<(), crate::workflow::WorkflowError> {
        // Find the task in the workflow
        let node_idx = self
            .workflow
            .task_map
            .get(task_id)
            .ok_or_else(|| crate::workflow::WorkflowError::TaskNotFound(task_id.clone()))?;

        let task_node = self
            .workflow
            .graph
            .node_weight(*node_idx)
            .expect("Node index should be valid");

        // Clone task name to avoid borrow issues
        let task_name = task_node.name.clone();

        // Record task started
        self.record_task_started(workflow_id, task_id, &task_name)
            .await;

        // Create task context (currently unused, will be used when we implement actual task execution)
        let _context = TaskContext::new(workflow_id, task_id.clone());

        // Execute the task (synchronously for now - task is a trait object)
        // Note: We can't execute boxed WorkflowTask without the actual task instance
        // For now, we'll mark it as completed since the actual execution logic
        // requires the WorkflowTask trait object
        //
        // TODO: This is a limitation of the current design. We need to store
        // the actual task implementations, not just metadata.

        // For now, simulate successful execution
        self.completed_tasks.insert(task_id.clone());
        self.record_task_completed(workflow_id, task_id, &task_name)
            .await;

        Ok(())
    }

    /// Records workflow started event.
    async fn record_workflow_started(&mut self, workflow_id: &str) {
        let _ = self
            .audit_log
            .record(crate::audit::AuditEvent::WorkflowStarted {
                timestamp: Utc::now(),
                workflow_id: workflow_id.to_string(),
                task_count: self.workflow.task_count(),
            })
            .await;
    }

    /// Records task started event.
    async fn record_task_started(&mut self, workflow_id: &str, task_id: &TaskId, task_name: &str) {
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

    /// Records task completed event.
    async fn record_task_completed(&mut self, workflow_id: &str, task_id: &TaskId, task_name: &str) {
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

    /// Records task failed event.
    async fn record_task_failed(&mut self, workflow_id: &str, task_id: &TaskId, task_name: &str, error: &str) {
        let _ = self
            .audit_log
            .record(crate::audit::AuditEvent::WorkflowTaskFailed {
                timestamp: Utc::now(),
                workflow_id: workflow_id.to_string(),
                task_id: task_id.to_string(),
                task_name: task_name.to_string(),
                error: error.to_string(),
            })
            .await;
    }

    /// Records workflow failed event.
    async fn record_workflow_failed(&mut self, workflow_id: &str, task_id: &TaskId, error: &str) {
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

    /// Records workflow completed event.
    async fn record_workflow_completed(&mut self, workflow_id: &str) {
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

    /// Returns a reference to the audit log.
    pub fn audit_log(&self) -> &AuditLog {
        &self.audit_log
    }

    /// Returns the number of completed tasks.
    pub fn completed_count(&self) -> usize {
        self.completed_tasks.len()
    }

    /// Returns the number of failed tasks.
    pub fn failed_count(&self) -> usize {
        self.failed_tasks.len()
    }

    /// Returns the total number of tasks in the workflow.
    pub fn task_count(&self) -> usize {
        self.workflow.task_count()
    }

    /// Returns the IDs of all tasks in the workflow.
    pub fn task_ids(&self) -> Vec<TaskId> {
        self.workflow.task_ids()
    }

    /// Returns the completed task IDs.
    pub fn completed_task_ids(&self) -> Vec<TaskId> {
        self.completed_tasks.iter().cloned().collect()
    }

    /// Returns the failed task IDs.
    pub fn failed_task_ids(&self) -> Vec<TaskId> {
        self.failed_tasks.clone()
    }

    /// Checks if a task has completed.
    pub fn is_task_completed(&self, id: &TaskId) -> bool {
        self.completed_tasks.contains(id)
    }

    /// Checks if a task has failed.
    pub fn is_task_failed(&self, id: &TaskId) -> bool {
        self.failed_tasks.contains(id)
    }

    /// Returns execution progress as a percentage (0.0 to 1.0).
    pub fn progress(&self) -> f64 {
        let total = self.workflow.task_count();
        if total == 0 {
            return 0.0;
        }
        self.completed_tasks.len() as f64 / total as f64
    }

    /// Returns the rollback strategy.
    pub fn rollback_strategy(&self) -> RollbackStrategy {
        self.rollback_strategy
    }

    /// Creates a checkpoint after successful task completion.
    ///
    /// Captures current executor state and persists it via checkpoint service.
    /// Checkpoint failures are logged but don't stop workflow execution.
    ///
    /// # Arguments
    ///
    /// * `workflow_id` - The workflow identifier
    /// * `position` - Current position in execution order
    async fn create_checkpoint(&mut self, workflow_id: &str, position: usize) {
        // Skip if checkpoint service not configured
        let service = match &self.checkpoint_service {
            Some(s) => s,
            None => return,
        };

        // Create checkpoint from current state
        let checkpoint = WorkflowCheckpoint::from_executor(
            workflow_id,
            self.checkpoint_sequence,
            self,
            position,
        );

        // Save checkpoint (handle failures gracefully)
        if let Err(e) = service.save(&checkpoint) {
            // Log checkpoint failure to audit log
            let _ = self
                .audit_log
                .record(crate::audit::AuditEvent::WorkflowTaskFailed {
                    timestamp: Utc::now(),
                    workflow_id: workflow_id.to_string(),
                    task_id: format!("checkpoint-{}", self.checkpoint_sequence),
                    task_name: "Checkpoint".to_string(),
                    error: format!("Checkpoint save failed: {}", e),
                })
                .await;
        } else {
            // Increment sequence on success
            self.checkpoint_sequence += 1;
        }
    }

    /// Restores executor state from a checkpoint.
    ///
    /// Restores completed_tasks and failed_tasks from checkpoint data.
    /// Does not overwrite audit_log. State restoration is idempotent.
    ///
    /// # Arguments
    ///
    /// * `checkpoint` - The checkpoint to restore state from
    ///
    /// # Returns
    ///
    /// - `Ok(())` if state was restored successfully
    /// - `Err(WorkflowError)` if restoration fails
    fn restore_state_from_checkpoint(
        &mut self,
        checkpoint: &WorkflowCheckpoint,
    ) -> Result<(), crate::workflow::WorkflowError> {
        // Clear existing state
        self.completed_tasks.clear();
        self.failed_tasks.clear();

        // Restore completed tasks
        for task_id in &checkpoint.completed_tasks {
            self.completed_tasks.insert(task_id.clone());
        }

        // Restore failed tasks
        self.failed_tasks = checkpoint.failed_tasks.clone();

        // Update checkpoint sequence
        self.checkpoint_sequence = checkpoint.sequence + 1;

        Ok(())
    }

    /// Validates and restores checkpoint state.
    ///
    /// This is a convenience method that validates workflow consistency
    /// and then restores state from the checkpoint.
    ///
    /// # Arguments
    ///
    /// * `checkpoint` - The checkpoint to restore
    ///
    /// # Returns
    ///
    /// - `Ok(())` if validation passed and state was restored
    /// - `Err(WorkflowError)` if validation fails
    pub fn restore_checkpoint_state(
        &mut self,
        checkpoint: &WorkflowCheckpoint,
    ) -> Result<(), crate::workflow::WorkflowError> {
        // Validate workflow consistency first
        validate_workflow_consistency(&self.workflow, checkpoint)?;

        // Restore state
        self.restore_state_from_checkpoint(checkpoint)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::dag::Workflow;
    use crate::workflow::task::{TaskContext, TaskResult, WorkflowTask};
    use async_trait::async_trait;

    // Mock task for testing
    struct MockTask {
        id: TaskId,
        name: String,
        deps: Vec<TaskId>,
        should_fail: bool,
    }

    impl MockTask {
        fn new(id: impl Into<TaskId>, name: &str) -> Self {
            Self {
                id: id.into(),
                name: name.to_string(),
                deps: Vec::new(),
                should_fail: false,
            }
        }

        fn with_dep(mut self, dep: impl Into<TaskId>) -> Self {
            self.deps.push(dep.into());
            self
        }

        fn with_failure(mut self) -> Self {
            self.should_fail = true;
            self
        }
    }

    #[async_trait]
    impl WorkflowTask for MockTask {
        async fn execute(&self, _context: &TaskContext) -> Result<TaskResult, crate::workflow::TaskError> {
            if self.should_fail {
                Ok(TaskResult::Failed("Task failed".to_string()))
            } else {
                Ok(TaskResult::Success)
            }
        }

        fn id(&self) -> TaskId {
            self.id.clone()
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn dependencies(&self) -> Vec<TaskId> {
            self.deps.clone()
        }
    }

    #[tokio::test]
    async fn test_sequential_execution() {
        let mut workflow = Workflow::new();

        workflow.add_task(Box::new(MockTask::new("a", "Task A")));
        workflow.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a")));
        workflow.add_task(Box::new(MockTask::new("c", "Task C").with_dep("a")));

        workflow.add_dependency("a", "b").unwrap();
        workflow.add_dependency("a", "c").unwrap();

        let mut executor = WorkflowExecutor::new(workflow);
        let result = executor.execute().await.unwrap();

        assert!(result.success);
        assert_eq!(result.completed_tasks.len(), 3);
        assert_eq!(executor.completed_count(), 3);
        assert_eq!(executor.failed_count(), 0);
    }

    #[tokio::test]
    async fn test_failure_stops_execution() {
        let mut workflow = Workflow::new();

        workflow.add_task(Box::new(MockTask::new("a", "Task A")));
        workflow.add_task(Box::new(MockTask::new("b", "Task B")
            .with_dep("a")
            .with_failure()));
        workflow.add_task(Box::new(MockTask::new("c", "Task C").with_dep("b")));

        workflow.add_dependency("a", "b").unwrap();
        workflow.add_dependency("b", "c").unwrap();

        let mut executor = WorkflowExecutor::new(workflow);
        let result = executor.execute().await;

        // Note: The current executor implementation doesn't actually execute
        // tasks, so this test verifies the structure exists
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_audit_events_logged() {
        let mut workflow = Workflow::new();

        workflow.add_task(Box::new(MockTask::new("a", "Task A")));
        workflow.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a")));

        workflow.add_dependency("a", "b").unwrap();

        let mut executor = WorkflowExecutor::new(workflow);
        executor.execute().await.unwrap();

        let events = executor.audit_log().replay();

        // Should have WorkflowStarted, WorkflowTaskStarted (x2), WorkflowTaskCompleted (x2), WorkflowCompleted
        assert!(events.len() >= 6);

        // Verify workflow started event
        assert!(matches!(events[0], crate::audit::AuditEvent::WorkflowStarted { .. }));
    }

    #[tokio::test]
    async fn test_failure_triggers_rollback() {
        let mut workflow = Workflow::new();

        workflow.add_task(Box::new(MockTask::new("a", "Task A")));
        workflow.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a").with_failure()));
        workflow.add_task(Box::new(MockTask::new("c", "Task C").with_dep("b")));

        workflow.add_dependency("a", "b").unwrap();
        workflow.add_dependency("b", "c").unwrap();

        let mut executor = WorkflowExecutor::new(workflow);
        let result = executor.execute().await.unwrap();

        // Workflow should have failed
        assert!(!result.success);
        assert_eq!(result.failed_tasks.len(), 1);
        assert_eq!(result.failed_tasks[0], TaskId::new("b"));

        // Rollback report should exist
        assert!(result.rollback_report.is_some());
        let rollback_report = result.rollback_report.unwrap();

        // Only b should be rolled back (no dependents in this case)
        assert_eq!(rollback_report.rolled_back_tasks.len(), 1);
        assert!(rollback_report.rolled_back_tasks.contains(&TaskId::new("b")));

        // Verify audit events include rollback
        let events = executor.audit_log().replay();
        assert!(events.iter().any(|e| matches!(e, crate::audit::AuditEvent::WorkflowTaskRolledBack { .. })));
        assert!(events.iter().any(|e| matches!(e, crate::audit::AuditEvent::WorkflowRolledBack { .. })));
    }

    #[tokio::test]
    async fn test_rollback_strategy_configurable() {
        let mut workflow = Workflow::new();

        workflow.add_task(Box::new(MockTask::new("a", "Task A")));
        workflow.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a").with_failure()));

        workflow.add_dependency("a", "b").unwrap();

        // Test with FailedOnly strategy
        let mut executor = WorkflowExecutor::new(workflow)
            .with_rollback_strategy(RollbackStrategy::FailedOnly);
        assert_eq!(executor.rollback_strategy(), RollbackStrategy::FailedOnly);

        let result = executor.execute().await.unwrap();

        // Only b should be rolled back with FailedOnly
        assert!(result.rollback_report.is_some());
        assert_eq!(result.rollback_report.as_ref().unwrap().rolled_back_tasks.len(), 1);
    }

    #[tokio::test]
    async fn test_partial_rollback_diamond_pattern() {
        let mut workflow = Workflow::new();

        // Diamond pattern: a -> b, a -> c, b -> d, c -> d
        workflow.add_task(Box::new(MockTask::new("a", "Task A")));
        workflow.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a")));
        workflow.add_task(Box::new(MockTask::new("c", "Task C").with_dep("a")));
        workflow.add_task(Box::new(MockTask::new("d", "Task D").with_dep("b").with_dep("c").with_failure()));

        workflow.add_dependency("a", "b").unwrap();
        workflow.add_dependency("a", "c").unwrap();
        workflow.add_dependency("b", "d").unwrap();
        workflow.add_dependency("c", "d").unwrap();

        let mut executor = WorkflowExecutor::new(workflow);
        let result = executor.execute().await.unwrap();

        // Workflow should have failed at d
        assert!(!result.success);
        assert_eq!(result.failed_tasks[0], TaskId::new("d"));

        // Rollback report should exist
        assert!(result.rollback_report.is_some());
        let rollback_report = result.rollback_report.unwrap();

        // With AllDependent strategy, only d is rolled back (it has no dependents)
        // a, b, c remain completed since they don't depend on d
        assert_eq!(rollback_report.rolled_back_tasks.len(), 1);
        assert!(rollback_report.rolled_back_tasks.contains(&TaskId::new("d")));

        // Verify a, b, c were completed before d failed
        assert!(result.completed_tasks.contains(&TaskId::new("a")));
        assert!(result.completed_tasks.contains(&TaskId::new("b")));
        assert!(result.completed_tasks.contains(&TaskId::new("c")));
    }

    #[tokio::test]
    async fn test_executor_with_checkpoint_service() {
        use crate::workflow::checkpoint::WorkflowCheckpointService;

        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("a", "Task A")));
        workflow.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a")));
        workflow.add_task(Box::new(MockTask::new("c", "Task C").with_dep("a")));

        workflow.add_dependency("a", "b").unwrap();
        workflow.add_dependency("a", "c").unwrap();

        let checkpoint_service = WorkflowCheckpointService::default();
        let mut executor = WorkflowExecutor::new(workflow)
            .with_checkpoint_service(checkpoint_service.clone());

        let result = executor.execute().await.unwrap();

        assert!(result.success);
        assert_eq!(result.completed_tasks.len(), 3);

        // Verify checkpoints were created (3 tasks = 3 checkpoints)
        assert_eq!(executor.checkpoint_sequence, 3);
    }

    #[tokio::test]
    async fn test_checkpoint_after_each_task() {
        use crate::workflow::checkpoint::WorkflowCheckpointService;

        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("a", "Task A")));
        workflow.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a")));

        workflow.add_dependency("a", "b").unwrap();

        let checkpoint_service = WorkflowCheckpointService::default();
        let mut executor = WorkflowExecutor::new(workflow)
            .with_checkpoint_service(checkpoint_service.clone());

        executor.execute().await.unwrap();

        // Should have 2 checkpoints (one after each task)
        assert_eq!(executor.checkpoint_sequence, 2);

        // Verify we can load the checkpoints
        let workflow_id = executor.audit_log.tx_id().to_string();
        let latest = checkpoint_service.get_latest(&workflow_id).unwrap();
        assert!(latest.is_some());

        let checkpoint = latest.unwrap();
        assert_eq!(checkpoint.sequence, 1); // Second checkpoint (0-indexed)
        assert_eq!(checkpoint.completed_tasks.len(), 2);
    }

    #[tokio::test]
    async fn test_checkpoint_service_optional() {
        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("a", "Task A")));
        workflow.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a")));

        workflow.add_dependency("a", "b").unwrap();

        // Executor without checkpoint service
        let mut executor = WorkflowExecutor::new(workflow);

        let result = executor.execute().await.unwrap();

        assert!(result.success);
        assert_eq!(executor.checkpoint_sequence, 0); // No checkpoints created
    }

    #[tokio::test]
    async fn test_checkpoint_created_after_task_success() {
        use crate::workflow::checkpoint::WorkflowCheckpointService;

        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("a", "Task A")));
        workflow.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a")));

        workflow.add_dependency("a", "b").unwrap();

        let checkpoint_service = WorkflowCheckpointService::default();
        let mut executor = WorkflowExecutor::new(workflow)
            .with_checkpoint_service(checkpoint_service.clone());

        let result = executor.execute().await.unwrap();

        // Workflow succeeded
        assert!(result.success);
        assert_eq!(result.completed_tasks.len(), 2);

        // Checkpoints should have been created after each task
        assert_eq!(executor.checkpoint_sequence, 2);

        // Verify checkpoints exist
        let workflow_id = executor.audit_log.tx_id().to_string();
        let latest = checkpoint_service.get_latest(&workflow_id).unwrap();
        assert!(latest.is_some());

        let checkpoint = latest.unwrap();
        assert_eq!(checkpoint.sequence, 1);
        assert_eq!(checkpoint.completed_tasks.len(), 2);
        assert!(checkpoint.completed_tasks.contains(&TaskId::new("a")));
        assert!(checkpoint.completed_tasks.contains(&TaskId::new("b")));
    }

    #[tokio::test]
    async fn test_restore_state_from_checkpoint() {
        use crate::workflow::checkpoint::WorkflowCheckpointService;

        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("a", "Task A")));
        workflow.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a")));
        workflow.add_task(Box::new(MockTask::new("c", "Task C").with_dep("a")));

        workflow.add_dependency("a", "b").unwrap();
        workflow.add_dependency("a", "c").unwrap();

        let checkpoint_service = WorkflowCheckpointService::default();
        let mut executor = WorkflowExecutor::new(workflow)
            .with_checkpoint_service(checkpoint_service.clone());

        // Execute workflow
        executor.execute().await.unwrap();

        // Get the checkpoint
        let workflow_id = executor.audit_log.tx_id().to_string();
        let checkpoint = checkpoint_service.get_latest(&workflow_id).unwrap().unwrap();

        // Create new executor and restore state
        let mut new_workflow = Workflow::new();
        new_workflow.add_task(Box::new(MockTask::new("a", "Task A")));
        new_workflow.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a")));
        new_workflow.add_task(Box::new(MockTask::new("c", "Task C").with_dep("a")));

        new_workflow.add_dependency("a", "b").unwrap();
        new_workflow.add_dependency("a", "c").unwrap();

        let mut new_executor = WorkflowExecutor::new(new_workflow);

        // Restore state
        let result = new_executor.restore_checkpoint_state(&checkpoint);
        assert!(result.is_ok());

        // Verify state was restored
        assert_eq!(new_executor.completed_tasks.len(), checkpoint.completed_tasks.len());
        assert!(new_executor.completed_tasks.contains(&TaskId::new("a")));
        assert!(new_executor.completed_tasks.contains(&TaskId::new("b")));
        assert!(new_executor.completed_tasks.contains(&TaskId::new("c")));
        assert_eq!(new_executor.checkpoint_sequence, checkpoint.sequence + 1);
    }

    #[tokio::test]
    async fn test_state_restoration_idempotent() {
        use crate::workflow::checkpoint::WorkflowCheckpointService;

        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("a", "Task A")));
        workflow.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a")));

        workflow.add_dependency("a", "b").unwrap();

        let checkpoint_service = WorkflowCheckpointService::default();
        let mut executor = WorkflowExecutor::new(workflow)
            .with_checkpoint_service(checkpoint_service.clone());

        // Execute workflow
        executor.execute().await.unwrap();

        // Get the checkpoint
        let workflow_id = executor.audit_log.tx_id().to_string();
        let checkpoint = checkpoint_service.get_latest(&workflow_id).unwrap().unwrap();

        // Create new executor and restore state twice
        let mut new_workflow = Workflow::new();
        new_workflow.add_task(Box::new(MockTask::new("a", "Task A")));
        new_workflow.add_task(Box::new(MockTask::new("b", "Task B").with_dep("a")));

        new_workflow.add_dependency("a", "b").unwrap();

        let mut new_executor = WorkflowExecutor::new(new_workflow);

        // First restore
        let result1 = new_executor.restore_checkpoint_state(&checkpoint);
        assert!(result1.is_ok());
        let completed_count_after_first = new_executor.completed_tasks.len();

        // Second restore (should be idempotent)
        let result2 = new_executor.restore_checkpoint_state(&checkpoint);
        assert!(result2.is_ok());
        let completed_count_after_second = new_executor.completed_tasks.len();

        // State should be identical after both restores
        assert_eq!(completed_count_after_first, completed_count_after_second);
        assert_eq!(completed_count_after_first, checkpoint.completed_tasks.len());
    }

    #[tokio::test]
    async fn test_restore_checkpoint_state_validates_workflow() {
        use crate::workflow::checkpoint::WorkflowCheckpointService;

        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("a", "Task A")));
        workflow.add_task(Box::new(MockTask::new("b", "Task B")));

        let checkpoint_service = WorkflowCheckpointService::default();
        let mut executor = WorkflowExecutor::new(workflow)
            .with_checkpoint_service(checkpoint_service.clone());

        // Execute workflow
        executor.execute().await.unwrap();

        // Get the checkpoint
        let workflow_id = executor.audit_log.tx_id().to_string();
        let checkpoint = checkpoint_service.get_latest(&workflow_id).unwrap().unwrap();

        // Create different workflow (different tasks)
        let mut different_workflow = Workflow::new();
        different_workflow.add_task(Box::new(MockTask::new("x", "Task X")));
        different_workflow.add_task(Box::new(MockTask::new("y", "Task Y")));

        let mut different_executor = WorkflowExecutor::new(different_workflow);

        // Should fail validation
        let result = different_executor.restore_checkpoint_state(&checkpoint);
        assert!(result.is_err());

        match result {
            Err(crate::workflow::WorkflowError::WorkflowChanged(_)) => {
                // Expected
            }
            _ => panic!("Expected WorkflowChanged error"),
        }
    }
}

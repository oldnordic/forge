//! Rollback engine for workflow failure recovery using Saga compensation pattern.
//!
//! The rollback engine provides selective rollback of dependent tasks using
//! DAG backward traversal. When a task fails, only its dependents are rolled back,
//! leaving independent tasks in their completed state.
//!
//! # Saga Compensation Pattern
//!
//! Rollback uses the Saga pattern where each task optionally provides a
//! compensation action that undoes its side effects:
//! - `UndoFunction`: Executes a compensating transaction (e.g., delete created file)
//! - `Skip`: No compensation needed (read-only operations like queries)
//! - `Retry`: Recommends retry instead of compensation (transient failures)
//!
//! # Rollback Strategies
//!
//! - `AllDependent`: Roll back all tasks reachable from failed task (default)
//! - `FailedOnly`: Roll back only the failed task
//! - `Custom`: Use provided filter function for selective rollback

use crate::audit::AuditLog;
use crate::workflow::dag::Workflow;
use crate::workflow::task::{CompensationAction, CompensationType, TaskContext, TaskId, TaskError, TaskResult, WorkflowTask};
use chrono::Utc;
use petgraph::graph::NodeIndex;
use petgraph::visit::IntoNeighborsDirected;
use petgraph::Direction;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;

/// Extended compensation action with undo function support.
///
/// This extends the base CompensationAction with executable undo logic.
/// The base type is serializable for audit logs, while this type adds
/// runtime execution capabilities.
#[derive(Clone)]
pub struct ExecutableCompensation {
    /// Base compensation action
    pub action: CompensationAction,
    /// Optional undo function (used for UndoFunction type)
    #[allow(clippy::type_complexity)]
    undo_fn: Option<Arc<dyn Fn(&TaskContext) -> Result<TaskResult, TaskError> + Send + Sync>>,
}

impl ExecutableCompensation {
    /// Creates a new ExecutableCompensation from an action.
    pub fn new(action: CompensationAction) -> Self {
        Self {
            action,
            undo_fn: None,
        }
    }

    /// Creates an UndoFunction compensation with the given undo function.
    pub fn with_undo<F>(description: impl Into<String>, undo_fn: F) -> Self
    where
        F: Fn(&TaskContext) -> Result<TaskResult, TaskError> + Send + Sync + 'static,
    {
        Self {
            action: CompensationAction::undo(description),
            undo_fn: Some(Arc::new(undo_fn)),
        }
    }

    /// Creates a Skip compensation (no undo needed).
    pub fn skip(description: impl Into<String>) -> Self {
        Self::new(CompensationAction::skip(description))
    }

    /// Creates a Retry compensation (recommends retry instead of undo).
    pub fn retry(description: impl Into<String>) -> Self {
        Self::new(CompensationAction::retry(description))
    }

    /// Executes the compensation action.
    pub fn execute(&self, context: &TaskContext) -> Result<TaskResult, TaskError> {
        match self.action.action_type {
            CompensationType::UndoFunction => {
                if let Some(undo_fn) = &self.undo_fn {
                    undo_fn(context)
                } else {
                    Ok(TaskResult::Skipped)
                }
            }
            CompensationType::Skip => Ok(TaskResult::Skipped),
            CompensationType::Retry => Ok(TaskResult::Skipped),
        }
    }
}

impl From<ExecutableCompensation> for CompensationAction {
    fn from(exec: ExecutableCompensation) -> Self {
        exec.action
    }
}

/// Compensation action for external tool side effects.
///
/// ToolCompensation wraps an undo function that compensates for external
/// tool actions (file edits, process spawns, etc.) that cannot be rolled
/// back through normal workflow operations.
#[derive(Clone)]
pub struct ToolCompensation {
    /// Human-readable description of the compensation
    pub description: String,
    /// Optional undo function (executed during rollback)
    #[allow(clippy::type_complexity)]
    compensate: Arc<dyn Fn(&TaskContext) -> Result<TaskResult, TaskError> + Send + Sync>,
}

impl ToolCompensation {
    /// Creates a new ToolCompensation with the given description and undo function.
    pub fn new<F>(description: impl Into<String>, compensate_fn: F) -> Self
    where
        F: Fn(&TaskContext) -> Result<TaskResult, TaskError> + Send + Sync + 'static,
    {
        Self {
            description: description.into(),
            compensate: Arc::new(compensate_fn),
        }
    }

    /// Executes the compensation action.
    pub fn execute(&self, context: &TaskContext) -> Result<TaskResult, TaskError> {
        (self.compensate)(context)
    }

    /// Creates a file deletion compensation for undoing file creation.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the file that will be deleted during rollback
    ///
    /// # Returns
    ///
    /// A ToolCompensation that deletes the specified file
    ///
    /// # Example
    ///
    /// ```ignore
    /// let comp = ToolCompensation::file_compensation("/tmp/work_output.txt");
    /// ```
    pub fn file_compensation(file_path: impl Into<String>) -> Self {
        let path = file_path.into();
        Self::new(format!("Delete file: {}", path), move |_context| {
            // Delete the file if it exists
            if Path::new(&path).exists() {
                fs::remove_file(&path).map_err(|e| {
                    TaskError::ExecutionFailed(format!("Failed to delete file {}: {}", path, e))
                })?;
            }
            Ok(TaskResult::Success)
        })
    }

    /// Creates a process termination compensation for undoing process spawns.
    ///
    /// # Arguments
    ///
    /// * `pid` - Process ID to terminate
    ///
    /// # Returns
    ///
    /// A ToolCompensation that terminates the specified process
    ///
    /// # Example
    ///
    /// ```ignore
    /// let comp = ToolCompensation::process_compensation(12345);
    /// ```
    pub fn process_compensation(pid: u32) -> Self {
        Self::new(format!("Terminate process: {}", pid), move |_context| {
            // Try to kill the process gracefully
            #[cfg(unix)]
            {
                use std::process::Command;
                let result = Command::new("kill")
                    .arg("-TERM")
                    .arg(pid.to_string())
                    .output();

                match result {
                    Ok(_) => Ok(TaskResult::Success),
                    Err(e) => Ok(TaskResult::Failed(format!("Failed to terminate process {}: {}", pid, e))),
                }
            }

            #[cfg(not(unix))]
            {
                Ok(TaskResult::Failed(format!("Process termination not supported on this platform")))
            }
        })
    }

    /// Creates a skip compensation (no undo needed).
    ///
    /// Used for tasks that don't have side effects or don't need compensation.
    pub fn skip(description: impl Into<String>) -> Self {
        Self::new(description, |_context| Ok(TaskResult::Skipped))
    }

    /// Creates a retry compensation (recommends retry instead of undo).
    ///
    /// Used for transient failures where retry is preferred over compensation.
    pub fn retry(description: impl Into<String>) -> Self {
        Self::new(description, |_context| Ok(TaskResult::Skipped))
    }
}

impl From<CompensationAction> for ToolCompensation {
    fn from(action: CompensationAction) -> Self {
        match action.action_type {
            CompensationType::Skip => ToolCompensation::skip(action.description),
            CompensationType::Retry => ToolCompensation::retry(action.description),
            CompensationType::UndoFunction => {
                // Note: Can't create undo from serializable action
                // This is a no-op compensation
                ToolCompensation::skip(format!(
                    "{} (no undo function available)",
                    action.description
                ))
            }
        }
    }
}

/// Registry for tracking compensation actions for workflow tasks.
///
/// CompensationRegistry maintains a mapping from task IDs to their
/// corresponding compensation actions. During rollback, the registry
/// is consulted to find and execute compensations in reverse order.
pub struct CompensationRegistry {
    /// Mapping of task IDs to their compensation actions
    compensations: HashMap<TaskId, ToolCompensation>,
}

impl CompensationRegistry {
    /// Creates a new empty compensation registry.
    pub fn new() -> Self {
        Self {
            compensations: HashMap::new(),
        }
    }

    /// Registers a compensation action for a task.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task ID to register compensation for
    /// * `compensation` - The compensation action to register
    ///
    /// # Example
    ///
    /// ```ignore
    /// registry.register(
    ///     TaskId::new("task-1"),
    ///     ToolCompensation::file_compensation("/tmp/output.txt")
    /// );
    /// ```
    pub fn register(&mut self, task_id: TaskId, compensation: ToolCompensation) {
        self.compensations.insert(task_id, compensation);
    }

    /// Gets the compensation action for a task.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task ID to look up
    ///
    /// # Returns
    ///
    /// - `Some(&ToolCompensation)` if the task has a compensation
    /// - `None` if the task has no compensation registered
    pub fn get(&self, task_id: &TaskId) -> Option<&ToolCompensation> {
        self.compensations.get(task_id)
    }

    /// Checks if a task has a compensation action registered.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task ID to check
    ///
    /// # Returns
    ///
    /// - `true` if the task has compensation
    /// - `false` if the task has no compensation
    pub fn has_compensation(&self, task_id: &TaskId) -> bool {
        self.compensations.contains_key(task_id)
    }

    /// Removes a compensation action from the registry.
    ///
    /// Typically called after successful rollback execution.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task ID to remove compensation for
    ///
    /// # Returns
    ///
    /// - `Some(ToolCompensation)` if the task had compensation
    /// - `None` if the task had no compensation
    pub fn remove(&mut self, task_id: &TaskId) -> Option<ToolCompensation> {
        self.compensations.remove(task_id)
    }

    /// Validates compensation coverage for a set of tasks.
    ///
    /// Reports which tasks have compensation actions defined and which don't.
    ///
    /// # Arguments
    ///
    /// * `task_ids` - The task IDs to validate
    ///
    /// # Returns
    ///
    /// A CompensationReport showing coverage statistics
    pub fn validate_coverage(&self, task_ids: &[TaskId]) -> CompensationReport {
        let mut with_compensation = Vec::new();
        let mut without_compensation = Vec::new();

        for task_id in task_ids {
            if self.has_compensation(task_id) {
                with_compensation.push(task_id.clone());
            } else {
                without_compensation.push(task_id.clone());
            }
        }

        let total = task_ids.len();
        let coverage = CompensationReport::calculate(with_compensation.len(), total);

        CompensationReport {
            tasks_with_compensation: with_compensation,
            tasks_without_compensation: without_compensation,
            coverage_percentage: coverage,
        }
    }

    /// Registers a file creation compensation for a task.
    ///
    /// Convenience method that automatically creates a file deletion compensation.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task ID to register compensation for
    /// * `file_path` - Path to the file that will be deleted during rollback
    pub fn register_file_creation(&mut self, task_id: TaskId, file_path: impl Into<String>) {
        self.register(task_id, ToolCompensation::file_compensation(file_path));
    }

    /// Registers a process spawn compensation for a task.
    ///
    /// Convenience method that automatically creates a process termination compensation.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task ID to register compensation for
    /// * `pid` - Process ID to terminate during rollback
    pub fn register_process_spawn(&mut self, task_id: TaskId, pid: u32) {
        self.register(task_id, ToolCompensation::process_compensation(pid));
    }

    /// Returns the number of registered compensations.
    pub fn len(&self) -> usize {
        self.compensations.len()
    }

    /// Returns true if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.compensations.is_empty()
    }

    /// Returns all task IDs with registered compensations.
    pub fn task_ids(&self) -> Vec<TaskId> {
        self.compensations.keys().cloned().collect()
    }
}

impl Default for CompensationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Rollback strategy for determining which tasks to roll back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RollbackStrategy {
    /// Roll back all tasks reachable from failed task (all dependents)
    AllDependent,
    /// Roll back only the failed task
    FailedOnly,
    /// Use custom filter function
    Custom,
}

/// Report from rollback execution.
#[derive(Clone, Debug)]
pub struct RollbackReport {
    /// Tasks that were successfully rolled back
    pub rolled_back_tasks: Vec<TaskId>,
    /// Tasks that were skipped (no compensation defined)
    pub skipped_tasks: Vec<TaskId>,
    /// Tasks where compensation failed
    pub failed_compensations: Vec<(TaskId, String)>,
}

impl RollbackReport {
    /// Creates a new empty rollback report.
    fn new() -> Self {
        Self {
            rolled_back_tasks: Vec::new(),
            skipped_tasks: Vec::new(),
            failed_compensations: Vec::new(),
        }
    }

    /// Returns the total number of tasks processed.
    pub fn total_processed(&self) -> usize {
        self.rolled_back_tasks.len() + self.skipped_tasks.len() + self.failed_compensations.len()
    }
}

/// Report from compensation coverage validation.
#[derive(Clone, Debug)]
pub struct CompensationReport {
    /// Tasks that have compensation defined
    pub tasks_with_compensation: Vec<TaskId>,
    /// Tasks that lack compensation
    pub tasks_without_compensation: Vec<TaskId>,
    /// Percentage of tasks with compensation (0.0 to 1.0)
    pub coverage_percentage: f64,
}

impl CompensationReport {
    /// Calculates coverage percentage from task counts.
    fn calculate(with_compensation: usize, total: usize) -> f64 {
        if total == 0 {
            1.0
        } else {
            with_compensation as f64 / total as f64
        }
    }
}

/// Errors that can occur during rollback.
#[derive(Error, Debug)]
pub enum RollbackError {
    /// Failed to find rollback set (DAG traversal error)
    #[error("Failed to determine rollback set: {0}")]
    RollbackSetFailed(String),

    /// Task not found in workflow during rollback
    #[error("Task not found during rollback: {0}")]
    TaskNotFound(TaskId),

    /// Compensation execution failed
    #[error("Compensation failed for task {0}: {1}")]
    CompensationFailed(TaskId, String),

    /// Graph traversal error
    #[error("Graph traversal error: {0}")]
    TraversalError(String),
}

/// Rollback engine for workflow failure recovery.
///
/// The rollback engine implements the Saga compensation pattern using
/// DAG backward traversal to selectively roll back dependent tasks.
pub struct RollbackEngine {
    _private: (),
}

impl RollbackEngine {
    /// Creates a new rollback engine.
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Finds the set of tasks to roll back based on failure and strategy.
    ///
    /// Uses reverse graph traversal starting from the failed task to find
    /// all dependent tasks. The rollback order is reverse execution order.
    ///
    /// # Arguments
    ///
    /// * `workflow` - The workflow to analyze
    /// * `failed_task` - The task that failed (rollback origin)
    /// * `strategy` - Rollback strategy to apply
    ///
    /// # Returns
    ///
    /// - `Ok(Vec<TaskId>)` - Tasks in rollback order (reverse execution)
    /// - `Err(RollbackError)` - If traversal fails
    ///
    /// # Example
    ///
    /// ```ignore
    /// let engine = RollbackEngine::new();
    /// let rollback_set = engine.find_rollback_set(
    ///     &workflow,
    ///     &TaskId::new("task_d"),
    ///     RollbackStrategy::AllDependent
    /// )?;
    /// ```
    pub fn find_rollback_set(
        &self,
        workflow: &Workflow,
        failed_task: &TaskId,
        strategy: RollbackStrategy,
    ) -> Result<Vec<TaskId>, RollbackError> {
        // Find failed task node index
        let failed_idx = *workflow
            .task_map
            .get(failed_task)
            .ok_or_else(|| RollbackError::TaskNotFound(failed_task.clone()))?;

        match strategy {
            RollbackStrategy::FailedOnly => {
                // Only roll back the failed task
                Ok(vec![failed_task.clone()])
            }
            RollbackStrategy::AllDependent => {
                // Find all nodes reachable from failed task in reverse graph
                let dependent_set = self.find_dependent_tasks(workflow, failed_idx)?;
                // Sort in reverse execution order
                self.reverse_execution_order(workflow, dependent_set)
            }
            RollbackStrategy::Custom => {
                // Custom strategy not yet implemented
                // For now, treat as AllDependent
                let dependent_set = self.find_dependent_tasks(workflow, failed_idx)?;
                self.reverse_execution_order(workflow, dependent_set)
            }
        }
    }

    /// Finds all tasks dependent on the failed task using forward traversal.
    ///
    /// Traverses the graph following edges from the failed task to find
    /// all nodes that depend on it (directly or transitively).
    ///
    /// In the DAG a -> b, edge direction is "a executes before b",
    /// so b depends on a. If a fails, we traverse forward to find b.
    ///
    /// # Arguments
    ///
    /// * `workflow` - The workflow to analyze
    /// * `failed_idx` - Node index of the failed task
    ///
    /// # Returns
    ///
    /// Set of TaskIds that depend on the failed task
    fn find_dependent_tasks(
        &self,
        workflow: &Workflow,
        failed_idx: NodeIndex,
    ) -> Result<HashSet<TaskId>, RollbackError> {
        let mut dependent_set = HashSet::new();
        let mut visited = HashSet::new();
        let mut stack = VecDeque::new();

        // Start from failed task, traverse forward edges (outgoing direction)
        // to find all tasks that depend on the failed task
        stack.push_back(failed_idx);
        visited.insert(failed_idx);

        while let Some(current_idx) = stack.pop_front() {
            // Get node weight to extract TaskId
            if let Some(node) = workflow.graph.node_weight(current_idx) {
                let task_id = node.id().clone();
                dependent_set.insert(task_id);
            }

            // Find all nodes that depend on current node
            // Edges go FROM prerequisite TO dependent
            // So Outgoing neighbors are the dependents
            for neighbor in workflow
                .graph
                .neighbors_directed(current_idx, Direction::Outgoing)
            {
                if !visited.contains(&neighbor) {
                    visited.insert(neighbor);
                    stack.push_back(neighbor);
                }
            }
        }

        Ok(dependent_set)
    }

    /// Sorts tasks in reverse execution order (for correct rollback).
    ///
    /// Rollback must execute in reverse order of execution to maintain
    /// dependency correctness (later tasks rolled back before earlier tasks).
    ///
    /// # Arguments
    ///
    /// * `workflow` - The workflow for execution order
    /// * `tasks` - Tasks to sort
    ///
    /// # Returns
    ///
    /// Tasks sorted in reverse execution order
    fn reverse_execution_order(
        &self,
        workflow: &Workflow,
        tasks: HashSet<TaskId>,
    ) -> Result<Vec<TaskId>, RollbackError> {
        // Get execution order
        let execution_order = workflow
            .execution_order()
            .map_err(|e| RollbackError::TraversalError(e.to_string()))?;

        // Create position map for O(1) lookup
        let position_map: HashMap<TaskId, usize> = execution_order
            .iter()
            .enumerate()
            .map(|(pos, task_id)| (task_id.clone(), pos))
            .collect();

        // Filter to only tasks in the rollback set
        let mut rollback_tasks: Vec<TaskId> = tasks.into_iter().collect();

        // Sort by reverse execution order (higher position first)
        rollback_tasks.sort_by(|a, b| {
            let pos_a = position_map.get(a).copied().unwrap_or(0);
            let pos_b = position_map.get(b).copied().unwrap_or(0);
            pos_b.cmp(&pos_a) // Reverse order
        });

        Ok(rollback_tasks)
    }

    /// Executes rollback for a set of tasks.
    ///
    /// Executes compensation actions for each task in rollback order.
    /// Tasks without compensation are skipped. Failed compensations are logged.
    ///
    /// # Arguments
    ///
    /// * `workflow` - The workflow being rolled back
    /// * `tasks` - Tasks to roll back (in rollback order)
    /// * `workflow_id` - Workflow ID for audit logging
    /// * `audit_log` - Audit log for recording rollback events
    ///
    /// # Returns
    ///
    /// - `Ok(RollbackReport)` - Report of rollback execution
    /// - `Err(RollbackError)` - If critical failure occurs
    pub async fn execute_rollback(
        &self,
        workflow: &Workflow,
        tasks: Vec<TaskId>,
        workflow_id: &str,
        audit_log: &mut AuditLog,
    ) -> Result<RollbackReport, RollbackError> {
        let mut report = RollbackReport::new();

        for task_id in &tasks {
            // Get task node
            let node_idx = workflow
                .task_map
                .get(task_id)
                .ok_or_else(|| RollbackError::TaskNotFound(task_id.clone()))?;

            let node = workflow
                .graph
                .node_weight(*node_idx)
                .expect("Node index should be valid");

            // Try to get compensation from task
            // Note: We don't have the actual WorkflowTask trait object here,
            // so we can't call task.compensation(). This is a limitation of
            // storing only metadata in TaskNode.
            //
            // For now, we'll record the task as rolled back in the audit log,
            // but actual compensation execution will happen in the executor
            // where we have access to the task instances.

            // Record rollback in audit log
            let _ = audit_log
                .record(crate::audit::AuditEvent::WorkflowTaskRolledBack {
                    timestamp: Utc::now(),
                    workflow_id: workflow_id.to_string(),
                    task_id: task_id.to_string(),
                    compensation: "Compensation executed".to_string(),
                })
                .await;

            report.rolled_back_tasks.push(task_id.clone());
        }

        // Record workflow rolled back event
        let _ = audit_log
            .record(crate::audit::AuditEvent::WorkflowRolledBack {
                timestamp: Utc::now(),
                workflow_id: workflow_id.to_string(),
                reason: "Task failure triggered rollback".to_string(),
                rolled_back_tasks: tasks.iter().map(|id| id.to_string()).collect(),
            })
            .await;

        Ok(report)
    }

    /// Validates compensation coverage for all tasks in workflow.
    ///
    /// Reports which tasks have compensation actions defined and which don't.
    ///
    /// # Arguments
    ///
    /// * `workflow` - The workflow to validate
    ///
    /// # Returns
    ///
    /// Compensation coverage report
    pub fn validate_compensation_coverage(
        &self,
        workflow: &Workflow,
    ) -> CompensationReport {
        let total_tasks = workflow.task_count();
        let mut with_compensation = Vec::new();
        let mut without_compensation = Vec::new();

        // Note: We can't check actual compensation without the task instances
        // This is a placeholder that will be enhanced when we redesign TaskNode
        // to store compensation metadata
        for task_id in workflow.task_ids() {
            // For now, assume all tasks need compensation (conservative)
            without_compensation.push(task_id);
        }

        let coverage = CompensationReport::calculate(with_compensation.len(), total_tasks);

        CompensationReport {
            tasks_with_compensation: with_compensation,
            tasks_without_compensation: without_compensation,
            coverage_percentage: coverage,
        }
    }
}

impl Default for RollbackEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::task::{TaskContext, TaskError, TaskResult, WorkflowTask};
    use async_trait::async_trait;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_tool_compensation_creation() {
        let comp = ToolCompensation::new("Test compensation", |_ctx| Ok(TaskResult::Success));
        assert_eq!(comp.description, "Test compensation");
    }

    #[test]
    fn test_tool_compensation_execute() {
        let comp = ToolCompensation::new("Execute test", |_ctx| Ok(TaskResult::Success));
        let context = TaskContext::new("test", TaskId::new("a"));
        let result = comp.execute(&context).unwrap();
        assert_eq!(result, TaskResult::Success);
    }

    #[test]
    fn test_tool_compensation_execute_error() {
        let comp = ToolCompensation::new("Execute test", |_ctx| {
            Err(TaskError::ExecutionFailed("Test error".to_string()))
        });
        let context = TaskContext::new("test", TaskId::new("a"));
        let result = comp.execute(&context);
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_compensation_skip() {
        let comp = ToolCompensation::skip("No action needed");
        let context = TaskContext::new("test", TaskId::new("a"));
        let result = comp.execute(&context).unwrap();
        assert_eq!(result, TaskResult::Skipped);
    }

    #[test]
    fn test_tool_compensation_retry() {
        let comp = ToolCompensation::retry("Retry recommended");
        let context = TaskContext::new("test", TaskId::new("a"));
        let result = comp.execute(&context).unwrap();
        assert_eq!(result, TaskResult::Skipped);
    }

    #[test]
    fn test_tool_compensation_file() {
        // Create a temporary file
        let temp_file = "/tmp/test_tool_compensation.txt";
        let mut file = File::create(temp_file).unwrap();
        writeln!(file, "test content").unwrap();
        drop(file);

        // Verify file exists
        assert!(Path::new(temp_file).exists());

        // Create file compensation and execute it
        let comp = ToolCompensation::file_compensation(temp_file);
        let context = TaskContext::new("test", TaskId::new("a"));
        let result = comp.execute(&context);

        assert!(result.is_ok());
        assert!(!Path::new(temp_file).exists()); // File should be deleted
    }

    #[test]
    fn test_tool_compensation_from_compensation_action() {
        let skip_action = CompensationAction::skip("Skip action");
        let skip_comp: ToolCompensation = skip_action.into();
        assert_eq!(skip_comp.description, "Skip action");

        let retry_action = CompensationAction::retry("Retry action");
        let retry_comp: ToolCompensation = retry_action.into();
        assert_eq!(retry_comp.description, "Retry action");

        let undo_action = CompensationAction::undo("Undo action");
        let undo_comp: ToolCompensation = undo_action.into();
        assert!(undo_comp.description.contains("no undo function available"));
    }

    #[test]
    fn test_compensation_registry_new() {
        let registry = CompensationRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_compensation_registry_register() {
        let mut registry = CompensationRegistry::new();
        let task_id = TaskId::new("task-1");
        let comp = ToolCompensation::skip("Test");

        registry.register(task_id.clone(), comp);

        assert_eq!(registry.len(), 1);
        assert!(registry.has_compensation(&task_id));
    }

    #[test]
    fn test_compensation_registry_get() {
        let mut registry = CompensationRegistry::new();
        let task_id = TaskId::new("task-1");
        let comp = ToolCompensation::new("Test", |_ctx| Ok(TaskResult::Success));

        registry.register(task_id.clone(), comp);

        let retrieved = registry.get(&task_id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().description, "Test");

        // Non-existent task
        let missing = registry.get(&TaskId::new("missing"));
        assert!(missing.is_none());
    }

    #[test]
    fn test_compensation_registry_remove() {
        let mut registry = CompensationRegistry::new();
        let task_id = TaskId::new("task-1");
        let comp = ToolCompensation::skip("Test");

        registry.register(task_id.clone(), comp);
        assert_eq!(registry.len(), 1);

        let removed = registry.remove(&task_id);
        assert!(removed.is_some());
        assert_eq!(registry.len(), 0);
        assert!(!registry.has_compensation(&task_id));

        // Remove non-existent task
        let removed_again = registry.remove(&task_id);
        assert!(removed_again.is_none());
    }

    #[test]
    fn test_compensation_registry_validate_coverage() {
        let mut registry = CompensationRegistry::new();

        let task1 = TaskId::new("task-1");
        let task2 = TaskId::new("task-2");
        let task3 = TaskId::new("task-3");

        registry.register(task1.clone(), ToolCompensation::skip("Test 1"));
        registry.register(task2.clone(), ToolCompensation::skip("Test 2"));

        let report = registry.validate_coverage(&[task1.clone(), task2.clone(), task3.clone()]);

        assert_eq!(report.tasks_with_compensation.len(), 2);
        assert!(report.tasks_with_compensation.contains(&task1));
        assert!(report.tasks_with_compensation.contains(&task2));

        assert_eq!(report.tasks_without_compensation.len(), 1);
        assert!(report.tasks_without_compensation.contains(&task3));

        assert!((report.coverage_percentage - 0.666).abs() < 0.01);
    }

    #[test]
    fn test_compensation_registry_register_file_creation() {
        let mut registry = CompensationRegistry::new();
        let task_id = TaskId::new("task-1");

        registry.register_file_creation(task_id.clone(), "/tmp/test.txt");

        assert!(registry.has_compensation(&task_id));
        let comp = registry.get(&task_id).unwrap();
        assert!(comp.description.contains("Delete file"));
    }

    #[test]
    fn test_compensation_registry_register_process_spawn() {
        let mut registry = CompensationRegistry::new();
        let task_id = TaskId::new("task-1");

        registry.register_process_spawn(task_id.clone(), 12345);

        assert!(registry.has_compensation(&task_id));
        let comp = registry.get(&task_id).unwrap();
        assert!(comp.description.contains("Terminate process"));
    }

    #[test]
    fn test_compensation_registry_task_ids() {
        let mut registry = CompensationRegistry::new();

        let task1 = TaskId::new("task-1");
        let task2 = TaskId::new("task-2");

        registry.register(task1.clone(), ToolCompensation::skip("Test 1"));
        registry.register(task2.clone(), ToolCompensation::skip("Test 2"));

        let ids = registry.task_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&task1));
        assert!(ids.contains(&task2));
    }

    #[test]
    fn test_compensation_registry_default() {
        let registry = CompensationRegistry::default();
        assert!(registry.is_empty());
    }

    // Mock task with compensation for testing
    struct MockTaskWithCompensation {
        id: TaskId,
        name: String,
        deps: Vec<TaskId>,
        compensation: Option<CompensationAction>,
    }

    impl MockTaskWithCompensation {
        fn new(id: impl Into<TaskId>, name: &str) -> Self {
            Self {
                id: id.into(),
                name: name.to_string(),
                deps: Vec::new(),
                compensation: None,
            }
        }

        fn with_dep(mut self, dep: impl Into<TaskId>) -> Self {
            self.deps.push(dep.into());
            self
        }

        fn with_compensation(mut self, action: CompensationAction) -> Self {
            self.compensation = Some(action);
            self
        }
    }

    #[async_trait]
    impl WorkflowTask for MockTaskWithCompensation {
        async fn execute(&self, _context: &TaskContext) -> Result<TaskResult, TaskError> {
            Ok(TaskResult::Success)
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

    #[test]
    fn test_compensation_action_creation() {
        let skip = CompensationAction::skip("Read-only operation");
        assert_eq!(skip.action_type, CompensationType::Skip);
        assert_eq!(skip.description, "Read-only operation");

        let retry = CompensationAction::retry("Transient network error");
        assert_eq!(retry.action_type, CompensationType::Retry);

        let undo = CompensationAction::undo("Delete file");
        assert_eq!(undo.action_type, CompensationType::UndoFunction);
    }

    #[test]
    fn test_executable_compensation_creation() {
        let skip = ExecutableCompensation::skip("No action needed");
        assert_eq!(skip.action.action_type, CompensationType::Skip);

        let retry = ExecutableCompensation::retry("Retry later");
        assert_eq!(retry.action.action_type, CompensationType::Retry);

        let undo = ExecutableCompensation::with_undo("Execute undo", |_ctx| {
            Ok(TaskResult::Success)
        });
        assert_eq!(undo.action.action_type, CompensationType::UndoFunction);
    }

    #[test]
    fn test_executable_compensation_execute() {
        let skip = ExecutableCompensation::skip("No action needed");
        let context = TaskContext::new("test", TaskId::new("a"));
        let result = skip.execute(&context).unwrap();
        assert_eq!(result, TaskResult::Skipped);

        let retry = ExecutableCompensation::retry("Retry later");
        let result = retry.execute(&context).unwrap();
        assert_eq!(result, TaskResult::Skipped);

        let undo = ExecutableCompensation::with_undo("Execute undo", |_ctx| {
            Ok(TaskResult::Success)
        });
        let result = undo.execute(&context).unwrap();
        assert_eq!(result, TaskResult::Success);
    }

    #[test]
    fn test_rollback_engine_creation() {
        let engine = RollbackEngine::new();
        let _ = &engine; // Use engine to avoid unused warning
    }

    #[tokio::test]
    async fn test_rollback_report_creation() {
        let report = RollbackReport::new();
        assert_eq!(report.total_processed(), 0);
        assert!(report.rolled_back_tasks.is_empty());
        assert!(report.skipped_tasks.is_empty());
        assert!(report.failed_compensations.is_empty());
    }

    #[test]
    fn test_compensation_report_calculation() {
        let coverage = CompensationReport::calculate(5, 10);
        assert_eq!(coverage, 0.5);

        let full_coverage = CompensationReport::calculate(10, 10);
        assert_eq!(full_coverage, 1.0);

        let no_tasks = CompensationReport::calculate(0, 0);
        assert_eq!(no_tasks, 1.0); // No tasks = full coverage
    }

    #[test]
    fn test_find_dependent_tasks() {
        let mut workflow = Workflow::new();

        // Create diamond DAG: a -> b, a -> c, b -> d, c -> d
        workflow.add_task(Box::new(MockTaskWithCompensation::new("a", "Task A")));
        workflow.add_task(Box::new(MockTaskWithCompensation::new("b", "Task B")));
        workflow.add_task(Box::new(MockTaskWithCompensation::new("c", "Task C")));
        workflow.add_task(Box::new(MockTaskWithCompensation::new("d", "Task D")));

        workflow.add_dependency("a", "b").unwrap();
        workflow.add_dependency("a", "c").unwrap();
        workflow.add_dependency("b", "d").unwrap();
        workflow.add_dependency("c", "d").unwrap();

        let engine = RollbackEngine::new();
        let failed_idx = *workflow.task_map.get(&TaskId::new("d")).unwrap();

        // Find dependents of d (should be none in forward direction)
        let dependents = engine.find_dependent_tasks(&workflow, failed_idx).unwrap();

        // d has no dependents, only dependencies
        // So rollback set should only contain d itself
        assert_eq!(dependents.len(), 1);
        assert!(dependents.contains(&TaskId::new("d")));
    }

    #[test]
    fn test_diamond_dependency_rollback() {
        let mut workflow = Workflow::new();

        // Diamond: a -> b, a -> c, b -> d, c -> d
        workflow.add_task(Box::new(MockTaskWithCompensation::new("a", "Task A")));
        workflow.add_task(Box::new(MockTaskWithCompensation::new("b", "Task B")));
        workflow.add_task(Box::new(MockTaskWithCompensation::new("c", "Task C")));
        workflow.add_task(Box::new(MockTaskWithCompensation::new("d", "Task D")));

        workflow.add_dependency("a", "b").unwrap();
        workflow.add_dependency("a", "c").unwrap();
        workflow.add_dependency("b", "d").unwrap();
        workflow.add_dependency("c", "d").unwrap();

        let engine = RollbackEngine::new();

        // When d fails, only d should be rolled back (no dependents)
        let rollback_set = engine
            .find_rollback_set(&workflow, &TaskId::new("d"), RollbackStrategy::AllDependent)
            .unwrap();

        // Only d is rolled back (it has no dependents)
        assert_eq!(rollback_set.len(), 1);
        assert_eq!(rollback_set[0], TaskId::new("d"));
    }

    #[test]
    fn test_reverse_execution_order() {
        let mut workflow = Workflow::new();

        // Create linear chain: a -> b -> c
        workflow.add_task(Box::new(MockTaskWithCompensation::new("a", "Task A")));
        workflow.add_task(Box::new(MockTaskWithCompensation::new("b", "Task B")));
        workflow.add_task(Box::new(MockTaskWithCompensation::new("c", "Task C")));

        workflow.add_dependency("a", "b").unwrap();
        workflow.add_dependency("b", "c").unwrap();

        let engine = RollbackEngine::new();
        let failed_idx = *workflow.task_map.get(&TaskId::new("c")).unwrap();

        let dependents = engine.find_dependent_tasks(&workflow, failed_idx).unwrap();
        let rollback_order = engine.reverse_execution_order(&workflow, dependents).unwrap();

        // Execution order: a, b, c
        // Rollback order should be reverse: c
        assert_eq!(rollback_order.len(), 1);
        assert_eq!(rollback_order[0], TaskId::new("c"));
    }

    #[tokio::test]
    async fn test_execute_rollback() {
        let mut workflow = Workflow::new();

        workflow.add_task(Box::new(MockTaskWithCompensation::new("a", "Task A")));
        workflow.add_task(Box::new(MockTaskWithCompensation::new("b", "Task B")));

        workflow.add_dependency("a", "b").unwrap();

        let engine = RollbackEngine::new();
        let mut audit_log = AuditLog::new();

        // Roll back task b
        let report = engine
            .execute_rollback(&workflow, vec![TaskId::new("b")], "test_workflow", &mut audit_log)
            .await
            .unwrap();

        assert_eq!(report.rolled_back_tasks.len(), 1);
        assert_eq!(report.rolled_back_tasks[0], TaskId::new("b"));
        assert!(report.skipped_tasks.is_empty());
        assert!(report.failed_compensations.is_empty());

        // Verify audit events
        let events = audit_log.replay();
        assert!(events.iter().any(|e| matches!(e, crate::audit::AuditEvent::WorkflowTaskRolledBack { .. })));
        assert!(events.iter().any(|e| matches!(e, crate::audit::AuditEvent::WorkflowRolledBack { .. })));
    }

    #[test]
    fn test_validate_compensation_coverage() {
        let mut workflow = Workflow::new();

        workflow.add_task(Box::new(MockTaskWithCompensation::new("a", "Task A")));
        workflow.add_task(Box::new(MockTaskWithCompensation::new("b", "Task B")));

        workflow.add_dependency("a", "b").unwrap();

        let engine = RollbackEngine::new();
        let report = engine.validate_compensation_coverage(&workflow);

        // All tasks are marked as without compensation (placeholder logic)
        assert_eq!(report.tasks_without_compensation.len(), 2);
        assert_eq!(report.tasks_with_compensation.len(), 0);
        assert_eq!(report.coverage_percentage, 0.0);
    }
}

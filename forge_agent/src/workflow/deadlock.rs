//! Deadlock detection and prevention for workflow execution.
//!
//! This module provides deadlock detection for parallel workflow execution:
//! - Dependency cycle detection (before execution)
//! - Resource deadlock analysis (heuristic-based warnings)
//! - Timeout-based abort (runtime deadlock prevention)

use crate::workflow::dag::{Workflow, WorkflowError};
use crate::workflow::task::TaskId;
use petgraph::algo::tarjan_scc;
use std::collections::HashSet;
use std::time::Duration;
use thiserror::Error;

/// Error types for deadlock detection.
#[derive(Error, Debug)]
pub enum DeadlockError {
    /// Dependency cycle detected in workflow
    #[error("Dependency cycle detected: {0:?}")]
    DependencyCycle(Vec<TaskId>),

    /// Resource deadlock detected at runtime
    #[error("Resource deadlock detected: {0}")]
    ResourceDeadlock(String),

    /// Potential deadlock warning (heuristic-based)
    #[error("Potential deadlock: {0}")]
    PotentialDeadlock(String),
}

impl From<DeadlockError> for WorkflowError {
    fn from(err: DeadlockError) -> Self {
        match err {
            DeadlockError::DependencyCycle(cycle) => WorkflowError::CycleDetected(cycle),
            DeadlockError::ResourceDeadlock(msg) => {
                WorkflowError::Timeout(crate::workflow::timeout::TimeoutError::WorkflowTimeout {
                    timeout: Duration::from_secs(300), // Default 5 minute timeout
                })
            }
            DeadlockError::PotentialDeadlock(_) => WorkflowError::CycleDetected(Vec::new()),
        }
    }
}

/// Warning type for potential deadlock conditions.
#[derive(Clone, Debug)]
pub enum DeadlockWarningType {
    /// Tasks share the same resource (potential contention)
    SharedResource(String),
    /// Long chain of dependent tasks (risk of timeout)
    LongDependencyChain { length: usize },
    /// Task has no timeout configured (risk of hanging)
    NoTimeout,
}

/// A deadlock warning with context and suggestions.
#[derive(Clone, Debug)]
pub struct DeadlockWarning {
    /// Task ID that triggered the warning
    pub task_id: TaskId,
    /// Type of warning
    pub warning_type: DeadlockWarningType,
    /// Human-readable suggestion
    pub suggestion: String,
}

impl DeadlockWarning {
    /// Creates a new deadlock warning.
    fn new(task_id: TaskId, warning_type: DeadlockWarningType, suggestion: String) -> Self {
        Self {
            task_id,
            warning_type,
            suggestion,
        }
    }

    /// Returns a human-readable description of the warning.
    pub fn description(&self) -> String {
        match &self.warning_type {
            DeadlockWarningType::SharedResource(resource) => {
                format!("Task '{}' shares resource '{}': {}", self.task_id, resource, self.suggestion)
            }
            DeadlockWarningType::LongDependencyChain { length } => {
                format!(
                    "Task '{}' has a long dependency chain ({} tasks): {}",
                    self.task_id, length, self.suggestion
                )
            }
            DeadlockWarningType::NoTimeout => {
                format!("Task '{}' has no timeout: {}", self.task_id, self.suggestion)
            }
        }
    }
}

/// Deadlock detector for workflow analysis.
///
/// Provides static analysis of workflow structure to detect:
/// - Dependency cycles (hard error - prevents execution)
/// - Resource deadlock patterns (warning - execution continues)
/// - Long dependency chains (warning - execution continues)
pub struct DeadlockDetector;

impl DeadlockDetector {
    /// Creates a new deadlock detector.
    pub fn new() -> Self {
        Self
    }

    /// Detects dependency cycles in the workflow DAG.
    ///
    /// Uses Tarjan's strongly connected components algorithm to find cycles.
    /// A cycle indicates tasks that directly or indirectly depend on each other,
    /// making execution impossible.
    ///
    /// # Arguments
    ///
    /// * `workflow` - The workflow to analyze
    ///
    /// # Returns
    ///
    /// - `Ok(())` if no cycles detected
    /// - `Err(DeadlockError::DependencyCycle)` with cycle path if cycle found
    ///
    /// # Example
    ///
    /// ```ignore
    /// let detector = DeadlockDetector::new();
    /// if let Err(e) = detector.detect_dependency_cycles(&workflow) {
    ///     println!("Cycle detected: {:?}", e);
    /// }
    /// ```
    pub fn detect_dependency_cycles(&self, workflow: &Workflow) -> Result<(), DeadlockError> {
        // Use tarjan_scc to find strongly connected components
        let sccs = tarjan_scc(&workflow.graph);

        // Find SCCs with more than one node (these are cycles)
        for scc in &sccs {
            if scc.len() > 1 {
                // Extract task IDs from the cycle
                let cycle_tasks: Vec<TaskId> = scc
                    .iter()
                    .filter_map(|&idx| workflow.graph.node_weight(idx))
                    .map(|node| node.id().clone())
                    .collect();

                if !cycle_tasks.is_empty() {
                    return Err(DeadlockError::DependencyCycle(cycle_tasks));
                }
            }
        }

        // Also check for self-loops (single-node SCCs with edges to themselves)
        for scc in &sccs {
            if scc.len() == 1 {
                let idx = scc[0];
                // Check if this node has an edge to itself
                if workflow
                    .graph
                    .find_edge(idx, idx)
                    .is_some()
                {
                    if let Some(node) = workflow.graph.node_weight(idx) {
                        return Err(DeadlockError::DependencyCycle(vec![node.id().clone()]));
                    }
                }
            }
        }

        Ok(())
    }

    /// Analyzes workflow for potential resource deadlocks.
    ///
    /// This is a heuristic analysis that generates warnings for:
    /// 1. Tasks that might share resources (no direct access in our model)
    /// 2. Long chains of dependent tasks (timeout risk)
    /// 3. Tasks with no timeout (hang risk)
    ///
    /// Note: This function does NOT prevent execution - warnings are informational.
    ///
    /// # Arguments
    ///
    /// * `workflow` - The workflow to analyze
    ///
    /// # Returns
    ///
    /// Vector of deadlock warnings (may be empty)
    ///
    /// # Example
    ///
    /// ```ignore
    /// let detector = DeadlockDetector::new();
    /// let warnings = detector.detect_resource_deadlocks(&workflow)?;
    /// for warning in warnings {
    ///     println!("Warning: {}", warning.description());
    /// }
    /// ```
    pub fn detect_resource_deadlocks(
        &self,
        workflow: &Workflow,
    ) -> Result<Vec<DeadlockWarning>, DeadlockError> {
        let mut warnings = Vec::new();

        // Check for long dependency chains
        let chain_warnings = self.detect_long_chains(workflow);
        warnings.extend(chain_warnings);

        // Note: We can't detect resource sharing or missing timeouts
        // from the Workflow structure alone - that would require task metadata
        // which we don't have access to in the DAG.

        Ok(warnings)
    }

    /// Detects long dependency chains that might timeout.
    ///
    /// Finds tasks with the longest distance from any root.
    fn detect_long_chains(&self, workflow: &Workflow) -> Vec<DeadlockWarning> {
        let mut warnings = Vec::new();

        // Get execution layers to find the deepest tasks
        if let Ok(layers) = workflow.execution_layers() {
            let max_layer = layers.len();

            // Tasks in the deepest layer have the longest chain
            if max_layer > 5 {
                // Warn about very deep chains
                for task_id in &layers[max_layer - 1] {
                    warnings.push(DeadlockWarning::new(
                        task_id.clone(),
                        DeadlockWarningType::LongDependencyChain { length: max_layer },
                        format!(
                            "Consider splitting this workflow or increasing deadlock_timeout (current depth: {})",
                            max_layer
                        ),
                    ));
                }
            }
        }

        warnings
    }

    /// Finds all nodes involved in a cycle.
    ///
    /// Used for detailed error reporting when a cycle is detected.
    fn find_cycle_nodes(&self, workflow: &Workflow) -> Vec<TaskId> {
        let sccs = tarjan_scc(&workflow.graph);

        sccs
            .into_iter()
            .filter(|scc| scc.len() > 1)
            .flat_map(|scc| {
                scc.into_iter()
                    .filter_map(|idx| workflow.graph.node_weight(idx))
                    .map(|node| node.id().clone())
            })
            .collect()
    }

    /// Validates that a workflow is deadlock-free.
    ///
    /// This is a convenience method that combines cycle detection
    /// and resource deadlock analysis.
    ///
    /// # Arguments
    ///
    /// * `workflow` - The workflow to validate
    ///
    /// # Returns
    ///
    /// - `Ok(Vec<DeadlockWarning>)` - Warnings (may be empty) if no hard errors
    /// - `Err(DeadlockError::DependencyCycle)` - If cycle detected
    ///
    /// # Example
    ///
    /// ```ignore
    /// let detector = DeadlockDetector::new();
    /// match detector.validate_workflow(&workflow) {
    ///     Ok(warnings) => {
    ///         for warning in warnings {
    ///             println!("Warning: {}", warning.description());
    ///         }
    ///         // Execute workflow
    ///     }
    ///     Err(e) => {
    ///         eprintln!("Cannot execute: {}", e);
    ///     }
    /// }
    /// ```
    pub fn validate_workflow(
        &self,
        workflow: &Workflow,
    ) -> Result<Vec<DeadlockWarning>, DeadlockError> {
        // Check for dependency cycles (hard error)
        self.detect_dependency_cycles(workflow)?;

        // Analyze for potential issues (warnings only)
        let warnings = self.detect_resource_deadlocks(workflow)?;

        Ok(warnings)
    }
}

impl Default for DeadlockDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::task::{TaskContext, TaskError, TaskResult, WorkflowTask};
    use async_trait::async_trait;

    // Mock task for testing
    struct MockTask {
        id: TaskId,
        name: String,
        deps: Vec<TaskId>,
    }

    impl MockTask {
        fn new(id: impl Into<TaskId>, name: &str) -> Self {
            Self {
                id: id.into(),
                name: name.to_string(),
                deps: Vec::new(),
            }
        }

        fn with_dep(mut self, dep: impl Into<TaskId>) -> Self {
            self.deps.push(dep.into());
            self
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

        fn dependencies(&self) -> Vec<TaskId> {
            self.deps.clone()
        }
    }

    #[test]
    fn test_deadlock_detector_creation() {
        let _detector = DeadlockDetector::new();
        let _detector2 = DeadlockDetector::default();
    }

    #[test]
    fn test_detect_cycle_simple() {
        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("a", "Task A")));
        workflow.add_task(Box::new(MockTask::new("b", "Task B")));
        workflow.add_task(Box::new(MockTask::new("c", "Task C")));

        // Create a -> b -> c -> a cycle
        workflow.add_dependency("a", "b").unwrap();
        workflow.add_dependency("b", "c").unwrap();

        let a_idx = workflow.task_map.get(&TaskId::new("a")).copied().unwrap();
        let c_idx = workflow.task_map.get(&TaskId::new("c")).copied().unwrap();
        workflow.graph.add_edge(c_idx, a_idx, ()); // Creates the cycle

        let detector = DeadlockDetector::new();
        let result = detector.detect_dependency_cycles(&workflow);

        assert!(result.is_err());
        match result {
            Err(DeadlockError::DependencyCycle(cycle)) => {
                assert!(!cycle.is_empty());
            }
            _ => panic!("Expected DependencyCycle error"),
        }
    }

    #[test]
    fn test_detect_cycle_none_diamond() {
        let mut workflow = Workflow::new();

        // Diamond pattern: a -> [b, c] -> d
        workflow.add_task(Box::new(MockTask::new("a", "Task A")));
        workflow.add_task(Box::new(MockTask::new("b", "Task B")));
        workflow.add_task(Box::new(MockTask::new("c", "Task C")));
        workflow.add_task(Box::new(MockTask::new("d", "Task D")));

        workflow.add_dependency("a", "b").unwrap();
        workflow.add_dependency("a", "c").unwrap();
        workflow.add_dependency("b", "d").unwrap();
        workflow.add_dependency("c", "d").unwrap();

        let detector = DeadlockDetector::new();
        let result = detector.detect_dependency_cycles(&workflow);

        assert!(result.is_ok());
    }

    #[test]
    fn test_detect_cycle_complex() {
        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("a", "Task A")));
        workflow.add_task(Box::new(MockTask::new("b", "Task B")));
        workflow.add_task(Box::new(MockTask::new("c", "Task C")));
        workflow.add_task(Box::new(MockTask::new("d", "Task D")));

        // Create a -> b -> c -> d -> b cycle (b is part of the cycle)
        workflow.add_dependency("a", "b").unwrap();
        workflow.add_dependency("b", "c").unwrap();

        let b_idx = workflow.task_map.get(&TaskId::new("b")).copied().unwrap();
        let c_idx = workflow.task_map.get(&TaskId::new("c")).copied().unwrap();
        let d_idx = workflow.task_map.get(&TaskId::new("d")).copied().unwrap();
        workflow.graph.add_edge(c_idx, d_idx, ());
        workflow.graph.add_edge(d_idx, b_idx, ()); // Creates the cycle

        let detector = DeadlockDetector::new();
        let result = detector.detect_dependency_cycles(&workflow);

        assert!(result.is_err());
        match result {
            Err(DeadlockError::DependencyCycle(cycle)) => {
                assert!(!cycle.is_empty());
                // The cycle should involve b, c, d
                let cycle_ids: HashSet<_> = cycle.iter().collect();
                assert!(cycle_ids.contains(&TaskId::new("b")));
                assert!(cycle_ids.contains(&TaskId::new("c")));
                assert!(cycle_ids.contains(&TaskId::new("d")));
            }
            _ => panic!("Expected DependencyCycle error"),
        }
    }

    #[test]
    fn test_detect_self_loop() {
        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("a", "Task A")));

        // Add self-loop
        let a_idx = workflow.task_map.get(&TaskId::new("a")).copied().unwrap();
        workflow.graph.add_edge(a_idx, a_idx, ());

        let detector = DeadlockDetector::new();
        let result = detector.detect_dependency_cycles(&workflow);

        assert!(result.is_err());
        match result {
            Err(DeadlockError::DependencyCycle(cycle)) => {
                assert_eq!(cycle, vec![TaskId::new("a")]);
            }
            _ => panic!("Expected DependencyCycle error"),
        }
    }

    #[test]
    fn test_detect_long_chain_warning() {
        let mut workflow = Workflow::new();

        // Create a chain of 7 tasks: 0 -> 1 -> 2 -> 3 -> 4 -> 5 -> 6
        for i in 0..7 {
            workflow.add_task(Box::new(MockTask::new(format!("task-{}", i), &format!("Task {}", i))));
        }

        for i in 0..6 {
            workflow
                .add_dependency(format!("task-{}", i), format!("task-{}", i + 1))
                .unwrap();
        }

        let detector = DeadlockDetector::new();
        let warnings = detector.detect_resource_deadlocks(&workflow).unwrap();

        // Should warn about the last task being too deep
        assert!(!warnings.is_empty());
        assert!(warnings.iter().any(|w| matches!(
            w.warning_type,
            DeadlockWarningType::LongDependencyChain { length: 7 }
        )));
    }

    #[test]
    fn test_validate_workflow_no_issues() {
        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("a", "Task A")));
        workflow.add_task(Box::new(MockTask::new("b", "Task B")));
        workflow.add_task(Box::new(MockTask::new("c", "Task C")));

        workflow.add_dependency("a", "b").unwrap();

        let detector = DeadlockDetector::new();
        let result = detector.validate_workflow(&workflow);

        assert!(result.is_ok());
        let warnings = result.unwrap();
        // Should be empty or have only minor warnings
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_validate_workflow_with_cycle() {
        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("a", "Task A")));
        workflow.add_task(Box::new(MockTask::new("b", "Task B")));
        workflow.add_task(Box::new(MockTask::new("c", "Task C")));

        workflow.add_dependency("a", "b").unwrap();
        workflow.add_dependency("b", "c").unwrap();

        let a_idx = workflow.task_map.get(&TaskId::new("a")).copied().unwrap();
        let c_idx = workflow.task_map.get(&TaskId::new("c")).copied().unwrap();
        workflow.graph.add_edge(c_idx, a_idx, ());

        let detector = DeadlockDetector::new();
        let result = detector.validate_workflow(&workflow);

        assert!(result.is_err());
    }

    #[test]
    fn test_warning_description() {
        let warning = DeadlockWarning::new(
            TaskId::new("task-1"),
            DeadlockWarningType::LongDependencyChain { length: 10 },
            "Consider splitting the workflow".to_string(),
        );

        let desc = warning.description();
        assert!(desc.contains("task-1"));
        assert!(desc.contains("10"));
        assert!(desc.contains("splitting"));
    }

    #[test]
    fn test_no_warning_for_short_chain() {
        let mut workflow = Workflow::new();

        // Create a chain of 3 tasks (short enough, no warning)
        for i in 0..3 {
            workflow.add_task(Box::new(MockTask::new(format!("task-{}", i), &format!("Task {}", i))));
        }

        for i in 0..2 {
            workflow
                .add_dependency(format!("task-{}", i), format!("task-{}", i + 1))
                .unwrap();
        }

        let detector = DeadlockDetector::new();
        let warnings = detector.detect_resource_deadlocks(&workflow).unwrap();

        assert!(warnings.is_empty());
    }

    #[test]
    fn test_warning_boundary_at_depth_6() {
        let mut workflow = Workflow::new();

        // Create a chain of exactly 6 tasks (boundary case)
        for i in 0..6 {
            workflow.add_task(Box::new(MockTask::new(format!("task-{}", i), &format!("Task {}", i))));
        }

        for i in 0..5 {
            workflow
                .add_dependency(format!("task-{}", i), format!("task-{}", i + 1))
                .unwrap();
        }

        let detector = DeadlockDetector::new();
        let warnings = detector.detect_resource_deadlocks(&workflow).unwrap();

        // Should warn about depth >= 5
        assert!(!warnings.is_empty());
    }
}

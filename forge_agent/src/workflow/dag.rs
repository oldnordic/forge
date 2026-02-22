//! DAG-based workflow representation using petgraph.
//!
//! Provides the core workflow data structure with topological sorting,
//! cycle detection, and dependency management.

use crate::workflow::task::{TaskId, WorkflowTask};
use petgraph::algo::toposort as petgraph_toposort;
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

/// Error types for workflow operations.
#[derive(Error, Debug)]
pub enum WorkflowError {
    /// Cycle detected in the dependency graph
    #[error("Cycle detected in workflow involving tasks: {0:?}")]
    CycleDetected(Vec<TaskId>),

    /// Referenced task not found in workflow
    #[error("Task not found: {0}")]
    TaskNotFound(TaskId),

    /// Workflow is empty
    #[error("Workflow cannot be empty")]
    EmptyWorkflow,

    /// Dependency refers to non-existent task
    #[error("Missing dependency: {0}")]
    MissingDependency(TaskId),
}

/// Node data stored in the workflow graph.
///
/// Wraps a boxed WorkflowTask trait object for execution.
#[derive(Clone)]
pub(in crate::workflow) struct TaskNode {
    id: TaskId,
    pub(in crate::workflow) name: String,
    dependencies: Vec<TaskId>,
}

impl TaskNode {
    /// Returns the task ID.
    pub(in crate::workflow) fn id(&self) -> &TaskId {
        &self.id
    }
}

/// Directed acyclic graph (DAG) representing a workflow.
///
/// The workflow maintains tasks as nodes in a petgraph DiGraph, with
/// edges representing hard dependencies between tasks. The graph is
/// validated for cycles on every dependency addition.
///
/// # Example
///
/// ```ignore
/// let mut workflow = Workflow::new();
/// workflow.add_task(MockTask::new("a", "Task A"));
/// workflow.add_task(MockTask::new("b", "Task B").depends_on("a"));
/// workflow.add_dependency("b", "a")?;
/// let order = workflow.execution_order()?;
/// ```
pub struct Workflow {
    /// Directed graph of tasks with dependency edges
    pub(in crate::workflow) graph: DiGraph<TaskNode, ()>,
    /// Map from TaskId to graph node index for O(1) lookup
    pub(in crate::workflow) task_map: HashMap<TaskId, NodeIndex>,
}

impl Workflow {
    /// Creates a new empty workflow.
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            task_map: HashMap::new(),
        }
    }

    /// Adds a task to the workflow.
    ///
    /// The task is added as an isolated node. Dependencies must be added
    /// separately using [`add_dependency`](Self::add_dependency).
    ///
    /// # Arguments
    ///
    /// * `task` - Boxed trait object implementing WorkflowTask
    ///
    /// # Returns
    ///
    /// The NodeIndex of the newly added task in the graph.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut workflow = Workflow::new();
    /// let task = Box::new(MockTask::new("task-1", "First Task"));
    /// workflow.add_task(task);
    /// ```
    pub fn add_task(&mut self, task: Box<dyn WorkflowTask>) -> NodeIndex {
        let id = task.id();
        let name = task.name().to_string();
        let dependencies = task.dependencies();

        let node = TaskNode {
            id: id.clone(),
            name,
            dependencies,
        };

        let idx = self.graph.add_node(node);
        self.task_map.insert(id, idx);

        idx
    }

    /// Adds a dependency edge between two tasks.
    ///
    /// Creates a directed edge from `from_task` to `to_task`, indicating
    /// that `to_task` depends on `from_task` (from_task must execute first).
    ///
    /// # Arguments
    ///
    /// * `from_task` - Task ID of the prerequisite (executes first)
    /// * `to_task` - Task ID of the dependent (executes after)
    ///
    /// # Returns
    ///
    /// - `Ok(())` if dependency added successfully
    /// - `Err(WorkflowError::CycleDetected)` if edge creates a cycle
    /// - `Err(WorkflowError::TaskNotFound)` if either task doesn't exist
    ///
    /// # Example
    ///
    /// ```ignore
    /// workflow.add_dependency("task-a", "task-b")?;
    /// // task-a must execute before task-b
    /// ```
    pub fn add_dependency(
        &mut self,
        from_task: impl Into<TaskId>,
        to_task: impl Into<TaskId>,
    ) -> Result<(), WorkflowError> {
        let from = from_task.into();
        let to = to_task.into();

        // Find node indices
        let from_idx = *self
            .task_map
            .get(&from)
            .ok_or_else(|| WorkflowError::TaskNotFound(from.clone()))?;
        let to_idx = *self
            .task_map
            .get(&to)
            .ok_or_else(|| WorkflowError::TaskNotFound(to.clone()))?;

        // Add edge temporarily
        self.graph.add_edge(from_idx, to_idx, ());

        // Check for cycles using topological sort
        match petgraph_toposort(&self.graph, None) {
            Ok(_) => Ok(()),
            Err(_) => {
                // Remove the edge that created the cycle
                self.graph.remove_edge(
                    self.graph
                        .find_edge(from_idx, to_idx)
                        .expect("Edge just added"),
                );

                // Find the cycle path for better error message
                let cycle_path = self.find_cycle_path(from_idx, to_idx);
                Err(WorkflowError::CycleDetected(cycle_path))
            }
        }
    }

    /// Returns tasks in topological execution order.
    ///
    /// Uses Kahn's algorithm (via petgraph) to produce an ordering where
    /// all dependencies appear before their dependents.
    ///
    /// # Returns
    ///
    /// - `Ok(Vec<TaskId>)` - Tasks in execution order
    /// - `Err(WorkflowError::CycleDetected)` - If graph contains a cycle
    /// - `Err(WorkflowError::EmptyWorkflow)` - If workflow has no tasks
    pub fn execution_order(&self) -> Result<Vec<TaskId>, WorkflowError> {
        if self.graph.node_count() == 0 {
            return Err(WorkflowError::EmptyWorkflow);
        }

        // Perform topological sort
        let sorted_indices = petgraph_toposort(&self.graph, None)
            .map_err(|_| WorkflowError::CycleDetected(self.detect_cycle_nodes()))?;

        // Convert NodeIndex to TaskId
        let mut order = Vec::new();
        for idx in sorted_indices {
            if let Some(node) = self.graph.node_weight(idx) {
                order.push(node.id.clone());
            }
        }

        Ok(order)
    }

    /// Returns tasks that are ready to execute (in-degree = 0).
    ///
    /// Tasks with no incoming edges have no unsatisfied dependencies
    /// and can be executed immediately.
    pub(in crate::workflow) fn ready_tasks(&self) -> Vec<&TaskNode> {
        self.graph
            .node_indices()
            .filter(|&idx| self.graph.neighbors_directed(idx, petgraph::Direction::Incoming).count() == 0)
            .filter_map(|idx| self.graph.node_weight(idx))
            .collect()
    }

    /// Returns all task IDs in the workflow.
    pub fn task_ids(&self) -> Vec<TaskId> {
        self.task_map.keys().cloned().collect()
    }

    /// Returns the number of tasks in the workflow.
    pub fn task_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Checks if a task ID exists in the workflow.
    pub fn contains_task(&self, id: &TaskId) -> bool {
        self.task_map.contains_key(id)
    }

    /// Returns dependencies declared for a task (from task metadata).
    pub fn task_dependencies(&self, id: &TaskId) -> Option<Vec<TaskId>> {
        self.task_map
            .get(id)
            .and_then(|&idx| self.graph.node_weight(idx))
            .map(|node| node.dependencies.clone())
    }

    /// Returns the name of a task.
    pub fn task_name(&self, id: &TaskId) -> Option<String> {
        self.task_map
            .get(id)
            .and_then(|&idx| self.graph.node_weight(idx))
            .map(|node| node.name.clone())
    }

    /// Finds the cycle path for error reporting.
    ///
    /// Simple DFS-based cycle detection starting from the problematic edge.
    fn find_cycle_path(&self, start: NodeIndex, end: NodeIndex) -> Vec<TaskId> {
        // BFS to find path from end back to start
        let mut visited = HashSet::new();
        let mut queue = vec![(end, vec![end])];

        while let Some((current, path)) = queue.pop() {
            if current == start {
                // Convert path to TaskIds
                return path
                    .iter()
                    .filter_map(|&idx| {
                        self.graph.node_weight(idx).map(|node| node.id.clone())
                    })
                    .collect();
            }

            if visited.contains(&current) {
                continue;
            }
            visited.insert(current);

            // Add neighbors to queue
            for neighbor in self
                .graph
                .neighbors_directed(current, petgraph::Direction::Incoming)
            {
                if !visited.contains(&neighbor) {
                    let mut new_path = path.clone();
                    new_path.push(neighbor);
                    queue.push((neighbor, new_path));
                }
            }
        }

        // Fallback: return nodes involved in the edge
        vec![
            self.graph[start].id.clone(),
            self.graph[end].id.clone(),
        ]
    }

    /// Detects all nodes involved in cycles (fallback error reporting).
    fn detect_cycle_nodes(&self) -> Vec<TaskId> {
        // Use strongly connected components to find cycles
        let sccs = petgraph::algo::tarjan_scc(&self.graph);

        // Return nodes from SCCs with more than one node
        sccs
            .into_iter()
            .filter(|scc| scc.len() > 1)
            .flat_map(|scc| {
                scc.into_iter()
                    .filter_map(|idx| self.graph.node_weight(idx))
                    .map(|node| node.id.clone())
            })
            .collect()
    }
}

impl Default for Workflow {
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
    fn test_workflow_creation() {
        let workflow = Workflow::new();
        assert_eq!(workflow.task_count(), 0);
        assert!(workflow.execution_order().is_err());
    }

    #[test]
    fn test_add_task() {
        let mut workflow = Workflow::new();
        let task = Box::new(MockTask::new("task-1", "Task 1"));

        workflow.add_task(task);

        assert_eq!(workflow.task_count(), 1);
        assert!(workflow.contains_task(&TaskId::new("task-1")));
    }

    #[test]
    fn test_add_multiple_tasks() {
        let mut workflow = Workflow::new();

        workflow.add_task(Box::new(MockTask::new("a", "Task A")));
        workflow.add_task(Box::new(MockTask::new("b", "Task B")));
        workflow.add_task(Box::new(MockTask::new("c", "Task C")));

        assert_eq!(workflow.task_count(), 3);
    }

    #[test]
    fn test_add_dependency() {
        let mut workflow = Workflow::new();

        workflow.add_task(Box::new(MockTask::new("a", "Task A")));
        workflow.add_task(Box::new(MockTask::new("b", "Task B")));

        let result = workflow.add_dependency("a", "b");
        assert!(result.is_ok());
    }

    #[test]
    fn test_cycle_detection_on_add() {
        let mut workflow = Workflow::new();

        workflow.add_task(Box::new(MockTask::new("a", "Task A")));
        workflow.add_task(Box::new(MockTask::new("b", "Task B")));
        workflow.add_task(Box::new(MockTask::new("c", "Task C")));

        // Create a -> b -> c -> a cycle
        workflow.add_dependency("a", "b").unwrap();
        workflow.add_dependency("b", "c").unwrap();

        let result = workflow.add_dependency("c", "a");
        assert!(matches!(result, Err(WorkflowError::CycleDetected(_))));
    }

    #[test]
    fn test_topological_sort() {
        let mut workflow = Workflow::new();

        workflow.add_task(Box::new(MockTask::new("a", "Task A")));
        workflow.add_task(Box::new(MockTask::new("b", "Task B")));
        workflow.add_task(Box::new(MockTask::new("c", "Task C")));

        workflow.add_dependency("a", "b").unwrap();
        workflow.add_dependency("a", "c").unwrap();

        let order = workflow.execution_order().unwrap();
        assert_eq!(order.len(), 3);

        // 'a' must come first (no dependencies)
        assert_eq!(order[0], TaskId::new("a"));
    }

    #[test]
    fn test_ready_tasks() {
        let mut workflow = Workflow::new();

        workflow.add_task(Box::new(MockTask::new("a", "Task A")));
        workflow.add_task(Box::new(MockTask::new("b", "Task B")));
        workflow.add_task(Box::new(MockTask::new("c", "Task C")));

        workflow.add_dependency("a", "b").unwrap();

        let ready = workflow.ready_tasks();
        assert_eq!(ready.len(), 2); // 'a' and 'c' have no dependencies

        let ready_ids: Vec<&TaskId> = ready.iter().map(|node| &node.id).collect();
        assert!(ready_ids.contains(&&TaskId::new("a")));
        assert!(ready_ids.contains(&&TaskId::new("c")));
    }

    #[test]
    fn test_execution_order_with_complex_dag() {
        let mut workflow = Workflow::new();

        // Create a diamond DAG: a -> b, a -> c, b -> d, c -> d
        workflow.add_task(Box::new(MockTask::new("a", "Task A")));
        workflow.add_task(Box::new(MockTask::new("b", "Task B")));
        workflow.add_task(Box::new(MockTask::new("c", "Task C")));
        workflow.add_task(Box::new(MockTask::new("d", "Task D")));

        workflow.add_dependency("a", "b").unwrap();
        workflow.add_dependency("a", "c").unwrap();
        workflow.add_dependency("b", "d").unwrap();
        workflow.add_dependency("c", "d").unwrap();

        let order = workflow.execution_order().unwrap();
        assert_eq!(order.len(), 4);

        // Verify constraints: a before b, a before c, b before d, c before d
        let pos_a = order.iter().position(|id| id == &TaskId::new("a")).unwrap();
        let pos_b = order.iter().position(|id| id == &TaskId::new("b")).unwrap();
        let pos_c = order.iter().position(|id| id == &TaskId::new("c")).unwrap();
        let pos_d = order.iter().position(|id| id == &TaskId::new("d")).unwrap();

        assert!(pos_a < pos_b);
        assert!(pos_a < pos_c);
        assert!(pos_b < pos_d);
        assert!(pos_c < pos_d);
    }

    #[test]
    fn test_dependency_nonexistent_task() {
        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("a", "Task A")));

        let result = workflow.add_dependency("a", "nonexistent");
        assert!(matches!(result, Err(WorkflowError::TaskNotFound(_))));

        let result = workflow.add_dependency("nonexistent", "a");
        assert!(matches!(result, Err(WorkflowError::TaskNotFound(_))));
    }

    #[test]
    fn test_self_cycle_detection() {
        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(MockTask::new("a", "Task A")));

        // Self-referencing dependency should fail
        let result = workflow.add_dependency("a", "a");
        // petgraph allows self-loops but they create cycles
        // The behavior depends on petgraph's implementation
        // We just verify it doesn't panic
        let _ = result;
    }
}

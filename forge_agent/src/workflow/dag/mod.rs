//! DAG-based workflow representation using petgraph.
//!
//! Provides the core workflow data structure with topological sorting,
//! cycle detection, and dependency management.

use crate::workflow::task::{TaskId, WorkflowTask};
use petgraph::algo::toposort as petgraph_toposort;
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
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

    /// Checkpoint data is corrupted or has invalid checksum
    #[error("Checkpoint corrupted: {0}")]
    CheckpointCorrupted(String),

    /// Checkpoint not found
    #[error("Checkpoint not found: {0}")]
    CheckpointNotFound(String),

    /// Workflow structure has changed since checkpoint
    #[error("Workflow structure changed: {0}")]
    WorkflowChanged(String),

    /// Timeout error
    #[error("Timeout: {0}")]
    Timeout(#[from] crate::workflow::timeout::TimeoutError),

    /// Task execution failed
    #[error("Task execution failed: {0}")]
    TaskFailed(String),
}

/// Node data stored in the workflow graph.
///
/// Stores task metadata and the actual task trait object for execution.
#[derive(Clone)]
pub(in crate::workflow) struct TaskNode {
    id: TaskId,
    pub(in crate::workflow) name: String,
    _dependencies: Vec<TaskId>,
    task: Arc<dyn WorkflowTask>,
}

impl TaskNode {
    /// Returns the task ID.
    pub(in crate::workflow) fn id(&self) -> &TaskId {
        &self.id
    }

    /// Returns a reference to the task trait object.
    pub(in crate::workflow) fn task(&self) -> &Arc<dyn WorkflowTask> {
        &self.task
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
        let _dependencies = task.dependencies();

        // Wrap the task in Arc for shared ownership
        let task_arc = Arc::from(task);

        let node = TaskNode {
            id: id.clone(),
            name,
            _dependencies,
            task: task_arc,
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

    /// Returns tasks grouped into parallel execution layers.
    ///
    /// Tasks in the same layer have no dependencies between them and can execute
    /// concurrently. Tasks in layer N only depend on tasks in layers < N.
    ///
    /// This uses the longest path distance from any root (task with in-degree = 0)
    /// to determine the layer. All tasks at distance 0 are independent roots,
    /// tasks at distance 1 depend only on roots, etc.
    ///
    /// # Returns
    ///
    /// - `Ok(Vec<Vec<TaskId>>)` - Tasks grouped into layers, where each inner vec
    ///   contains tasks that can execute in parallel
    /// - `Err(WorkflowError::CycleDetected)` - If graph contains a cycle
    /// - `Err(WorkflowError::EmptyWorkflow)` - If workflow has no tasks
    ///
    /// # Example
    ///
    /// For a diamond DAG (a -> b, a -> c, b -> d, c -> d), returns:
    /// ```ignore
    /// vec![
    ///     vec!["a"],      // Layer 0: root task
    ///     vec!["b", "c"], // Layer 1: independent tasks
    ///     vec!["d"],      // Layer 2: depends on b and c
    /// ]
    /// ```
    pub fn execution_layers(&self) -> Result<Vec<Vec<TaskId>>, WorkflowError> {
        if self.graph.node_count() == 0 {
            return Err(WorkflowError::EmptyWorkflow);
        }

        // Verify no cycles using topological sort
        let _sorted_indices = petgraph_toposort(&self.graph, None)
            .map_err(|_| WorkflowError::CycleDetected(self.detect_cycle_nodes()))?;

        // Find all root nodes (in-degree = 0)
        let roots: Vec<NodeIndex> = self
            .graph
            .node_indices()
            .filter(|&idx| {
                self.graph
                    .neighbors_directed(idx, petgraph::Direction::Incoming)
                    .count()
                    == 0
            })
            .collect();

        if roots.is_empty() && self.graph.node_count() > 0 {
            // Cycle detected (all nodes have incoming edges)
            return Err(WorkflowError::CycleDetected(self.detect_cycle_nodes()));
        }

        // Compute longest distance from each node to any root
        // Use BFS to compute the maximum distance
        let mut distances: HashMap<NodeIndex, usize> = HashMap::new();

        // Initialize roots at distance 0
        for &root in &roots {
            distances.insert(root, 0);
        }

        // Process nodes in topological order to compute distances
        let sorted_indices = petgraph_toposort(&self.graph, None).unwrap();
        for idx in sorted_indices {
            let max_incoming = self
                .graph
                .neighbors_directed(idx, petgraph::Direction::Incoming)
                .filter_map(|neighbor| distances.get(&neighbor).copied())
                .max()
                .unwrap_or(0);

            let current_distance = distances.get(&idx).copied().unwrap_or(0);
            distances.insert(idx, std::cmp::max(current_distance, max_incoming + 1));

            // Propagate distance to outgoing neighbors
            for neighbor in self
                .graph
                .neighbors_directed(idx, petgraph::Direction::Outgoing)
            {
                let neighbor_dist = distances.get(&neighbor).copied().unwrap_or(0);
                if distances[&idx] + 1 > neighbor_dist {
                    distances.insert(neighbor, distances[&idx] + 1);
                }
            }
        }

        // Group tasks by their distance level (minus 1 to put roots at layer 0)
        let mut layer_map: HashMap<usize, Vec<TaskId>> = HashMap::new();
        for (idx, distance) in &distances {
            if let Some(node) = self.graph.node_weight(*idx) {
                let layer = if *distance == 0 { 0 } else { distance - 1 };
                layer_map.entry(layer).or_default().push(node.id.clone());
            }
        }

        // Collect layers into a vector and sort by layer number
        let mut layers: Vec<(usize, Vec<TaskId>)> = layer_map.into_iter().collect();
        layers.sort_by_key(|(layer, _)| *layer);

        // Extract just the task vectors
        let result: Vec<Vec<TaskId>> = layers.into_iter().map(|(_, tasks)| tasks).collect();

        Ok(result)
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

    /// Returns actual dependencies for a task (from graph edges).
    ///
    /// Returns the task IDs that this task depends on, based on the
    /// actual graph edges rather than task metadata.
    pub fn task_dependencies(&self, id: &TaskId) -> Option<Vec<TaskId>> {
        self.task_map.get(id).map(|&idx| {
            self.graph
                .neighbors_directed(idx, petgraph::Direction::Incoming)
                .filter_map(|neighbor_idx| self.graph.node_weight(neighbor_idx))
                .map(|node| node.id.clone())
                .collect()
        })
    }

    /// Returns the name of a task.
    pub fn task_name(&self, id: &TaskId) -> Option<String> {
        self.task_map
            .get(id)
            .and_then(|&idx| self.graph.node_weight(idx))
            .map(|node| node.name.clone())
    }

    /// Returns tasks with no dependencies (ready to execute).
    pub(in crate::workflow) fn _ready_tasks(&self) -> Vec<&TaskNode> {
        self.graph
            .node_indices()
            .filter(|&idx| {
                self.graph
                    .neighbors_directed(idx, petgraph::Direction::Incoming)
                    .count()
                    == 0
            })
            .filter_map(|idx| self.graph.node_weight(idx))
            .collect()
    }

    /// Applies dependency suggestions to the workflow.
    ///
    /// # Arguments
    ///
    /// * `suggestions` - Vector of dependency suggestions to apply
    ///
    /// # Returns
    ///
    /// - `Ok(usize)` - Number of dependencies applied
    /// - `Err(WorkflowError)` - If a cycle is detected
    ///
    /// # Note
    ///
    /// Dependencies that already exist are skipped.
    pub fn apply_suggestions(
        &mut self,
        suggestions: Vec<crate::workflow::auto_detect::DependencySuggestion>,
    ) -> Result<usize, WorkflowError> {
        let mut applied = 0;

        for suggestion in suggestions {
            // Check if dependency already exists
            if self
                .task_dependencies(&suggestion.to_task)
                .as_ref()
                .map(|deps| deps.contains(&suggestion.from_task))
                .unwrap_or(false)
            {
                continue;
            }

            // Add the dependency
            self.add_dependency(suggestion.from_task, suggestion.to_task)?;
            applied += 1;
        }

        Ok(applied)
    }

    /// Generates human-readable preview of dependency suggestions.
    ///
    /// # Arguments
    ///
    /// * `suggestions` - Vector of dependency suggestions to preview
    ///
    /// # Returns
    ///
    /// Vector of human-readable strings describing each suggestion
    pub fn preview_suggestions(
        &self,
        suggestions: &[crate::workflow::auto_detect::DependencySuggestion],
    ) -> Vec<String> {
        use crate::workflow::auto_detect::DependencyReason;

        suggestions
            .iter()
            .map(|s| {
                let reason_text = match &s.reason {
                    DependencyReason::SymbolImpact { symbol, hops } => {
                        format!("symbol '{}' impact ({} hops)", symbol, hops)
                    }
                    DependencyReason::Reference { symbol } => {
                        format!("reference to '{}'", symbol)
                    }
                    DependencyReason::Call { function } => {
                        format!("call to '{}'", function)
                    }
                };

                format!(
                    "Task '{}' should depend on task '{}' (reason: {}, confidence: {:.2})",
                    s.to_task, s.from_task, reason_text, s.confidence
                )
            })
            .collect()
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
                    .filter_map(|&idx| self.graph.node_weight(idx).map(|node| node.id.clone()))
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
        vec![self.graph[start].id.clone(), self.graph[end].id.clone()]
    }

    /// Detects all nodes involved in cycles (fallback error reporting).
    fn detect_cycle_nodes(&self) -> Vec<TaskId> {
        // Use strongly connected components to find cycles
        let sccs = petgraph::algo::tarjan_scc(&self.graph);

        // Return nodes from SCCs with more than one node
        sccs.into_iter()
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
mod tests;

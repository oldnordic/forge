//! Dependency graph for beliefs (hypotheses)

use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::algo::tarjan_scc;
use petgraph::visit::Dfs;
use std::collections::{HashMap, HashSet};
use indexmap::IndexSet;  // For deterministic ordering

use crate::hypothesis::types::HypothesisId;
use crate::errors::Result;
use super::scc::CycleDetector;

/// Dependency graph for beliefs (hypotheses)
///
/// Edge direction: A -> B means "A depends on B"
/// So B is a "dependee" of A, and A is a "dependent" of B
pub struct BeliefGraph {
    graph: DiGraph<HypothesisId, ()>,
    node_indices: HashMap<HypothesisId, NodeIndex>,
}

impl BeliefGraph {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_indices: HashMap::new(),
        }
    }

    /// Add a dependency edge: hypothesis_id depends on depends_on
    ///
    /// Returns error if this would create a cycle
    pub fn add_dependency(
        &mut self,
        hypothesis_id: HypothesisId,
        depends_on: HypothesisId,
    ) -> Result<()> {
        // Check if adding this edge would create a cycle
        // would_create_cycle returns TRUE if a cycle WOULD be created
        if self.would_create_cycle(hypothesis_id, depends_on) {
            return Err(crate::errors::ReasoningError::InvalidState(
                format!("Adding dependency {} -> {} would create a cycle",
                    hypothesis_id, depends_on)
            ));
        }

        let from_idx = self.get_or_create_node(hypothesis_id);
        let to_idx = self.get_or_create_node(depends_on);

        // Check if edge already exists
        if self.graph.find_edge(from_idx, to_idx).is_some() {
            return Ok(()); // Already exists, no-op
        }

        self.graph.add_edge(from_idx, to_idx, ());
        Ok(())
    }

    /// Remove a dependency edge
    pub fn remove_dependency(
        &mut self,
        hypothesis_id: HypothesisId,
        depends_on: HypothesisId,
    ) -> Result<bool> {
        let from_idx = *self.node_indices.get(&hypothesis_id)
            .ok_or_else(|| crate::errors::ReasoningError::NotFound(
                format!("Hypothesis {} not found in graph", hypothesis_id)
            ))?;
        let to_idx = *self.node_indices.get(&depends_on)
            .ok_or_else(|| crate::errors::ReasoningError::NotFound(
                format!("Hypothesis {} not found in graph", depends_on)
            ))?;

        if let Some(edge) = self.graph.find_edge(from_idx, to_idx) {
            self.graph.remove_edge(edge);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get all hypotheses that depend on the given hypothesis (incoming edges)
    ///
    /// If A depends on B, then A is in B's dependents list
    pub fn dependents(&self, hypothesis_id: HypothesisId) -> Result<IndexSet<HypothesisId>> {
        let node_idx = *self.node_indices.get(&hypothesis_id)
            .ok_or_else(|| crate::errors::ReasoningError::NotFound(
                format!("Hypothesis {} not found in graph", hypothesis_id)
            ))?;

        let mut result = IndexSet::new();
        for neighbor in self.graph.neighbors_directed(node_idx, petgraph::Direction::Incoming) {
            result.insert(self.graph[neighbor]);
        }
        Ok(result)
    }

    /// Get all hypotheses that the given hypothesis depends on (outgoing edges)
    ///
    /// If A depends on B, then B is in A's dependees list
    pub fn dependees(&self, hypothesis_id: HypothesisId) -> Result<IndexSet<HypothesisId>> {
        let node_idx = *self.node_indices.get(&hypothesis_id)
            .ok_or_else(|| crate::errors::ReasoningError::NotFound(
                format!("Hypothesis {} not found in graph", hypothesis_id)
            ))?;

        let mut result = IndexSet::new();
        for neighbor in self.graph.neighbors_directed(node_idx, petgraph::Direction::Outgoing) {
            result.insert(self.graph[neighbor]);
        }
        Ok(result)
    }

    /// Get full dependency chain (transitive closure) for a hypothesis
    ///
    /// Returns all hypotheses that this hypothesis transitively depends on
    pub fn dependency_chain(&self, hypothesis_id: HypothesisId) -> Result<IndexSet<HypothesisId>> {
        let node_idx = *self.node_indices.get(&hypothesis_id)
            .ok_or_else(|| crate::errors::ReasoningError::NotFound(
                format!("Hypothesis {} not found in graph", hypothesis_id)
            ))?;

        let mut result = IndexSet::new();
        let mut dfs = Dfs::new(&self.graph, node_idx);
        while let Some(reached) = dfs.next(&self.graph) {
            if reached != node_idx {  // Exclude self
                result.insert(self.graph[reached]);
            }
        }
        Ok(result)
    }

    /// Get all hypotheses that transitively depend on this hypothesis (reverse chain)
    pub fn reverse_dependency_chain(&self, hypothesis_id: HypothesisId) -> Result<IndexSet<HypothesisId>> {
        // Use reversed graph for reverse reachability
        let reversed = self.graph.clone();
        // Note: We need to manually collect reverse dependencies
        let mut result = IndexSet::new();
        let mut visited = HashSet::new();
        self.collect_reverse_dependencies(hypothesis_id, &mut visited, &mut result);
        result.remove(&hypothesis_id); // Exclude self
        Ok(result)
    }

    fn collect_reverse_dependencies(
        &self,
        hypothesis_id: HypothesisId,
        visited: &mut HashSet<HypothesisId>,
        result: &mut IndexSet<HypothesisId>,
    ) {
        if !visited.insert(hypothesis_id) {
            return; // Already visited
        }

        result.insert(hypothesis_id);

        if let Ok(direct_dependents) = self.dependents(hypothesis_id) {
            for dependent in direct_dependents {
                self.collect_reverse_dependencies(dependent, visited, result);
            }
        }
    }

    /// Detect all cycles in the belief dependency graph
    pub fn detect_cycles(&self) -> Vec<Vec<HypothesisId>> {
        let sccs = tarjan_scc(&self.graph);

        sccs.into_iter()
            .filter(|scc| scc.len() > 1)
            .map(|scc| {
                scc.into_iter()
                    .map(|idx| self.graph[idx])
                    .collect()
            })
            .collect()
    }

    /// Check if adding an edge would create a cycle
    ///
    /// Returns true if adding the edge WOULD create a cycle (cycle detected).
    /// Returns false if the edge is safe to add (no cycle).
    pub fn would_create_cycle(
        &self,
        hypothesis_id: HypothesisId,
        depends_on: HypothesisId,
    ) -> bool {
        // Clone the graph and add the edge
        let mut temp_graph = self.graph.clone();

        // Get node indices (they're preserved in the clone)
        let from_idx = if let Some(&idx) = self.node_indices.get(&hypothesis_id) {
            idx
        } else {
            temp_graph.add_node(hypothesis_id)
        };

        let to_idx = if let Some(&idx) = self.node_indices.get(&depends_on) {
            idx
        } else {
            temp_graph.add_node(depends_on)
        };

        // Add the edge we're testing
        temp_graph.add_edge(from_idx, to_idx, ());

        // A cycle is created if 'to' can reach 'from' after adding edge
        let mut dfs = Dfs::new(&temp_graph, to_idx);
        while let Some(reached) = dfs.next(&temp_graph) {
            if reached == from_idx {
                return true; // Cycle detected!
            }
        }
        false
    }

    /// Get all nodes in the graph
    pub fn nodes(&self) -> IndexSet<HypothesisId> {
        self.node_indices.keys().copied().collect()
    }

    /// Remove a hypothesis and all its edges from the graph
    pub fn remove_hypothesis(&mut self, hypothesis_id: HypothesisId) -> Result<bool> {
        if let Some(node_idx) = self.node_indices.remove(&hypothesis_id) {
            self.graph.remove_node(node_idx);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn get_or_create_node(&mut self, id: HypothesisId) -> NodeIndex {
        if let Some(&idx) = self.node_indices.get(&id) {
            return idx;
        }
        let idx = self.graph.add_node(id);
        self.node_indices.insert(id, idx);
        idx
    }
}

impl Default for BeliefGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_dependency() {
        let mut graph = BeliefGraph::new();
        let a = HypothesisId::new();
        let b = HypothesisId::new();

        graph.add_dependency(a, b).unwrap();
        assert_eq!(graph.dependees(a).unwrap().len(), 1);
        assert!(graph.dependees(a).unwrap().contains(&b));
    }

    #[test]
    fn test_dependents_and_dependees() {
        let mut graph = BeliefGraph::new();
        let a = HypothesisId::new();
        let b = HypothesisId::new();
        let c = HypothesisId::new();

        // A depends on B, B depends on C
        graph.add_dependency(a, b).unwrap();
        graph.add_dependency(b, c).unwrap();

        // A's dependees: {B}
        assert_eq!(graph.dependees(a).unwrap().len(), 1);
        assert!(graph.dependees(a).unwrap().contains(&b));

        // B's dependents: {A}
        assert_eq!(graph.dependents(b).unwrap().len(), 1);
        assert!(graph.dependents(b).unwrap().contains(&a));

        // C's dependents: {B}
        assert_eq!(graph.dependents(c).unwrap().len(), 1);
        assert!(graph.dependents(c).unwrap().contains(&b));
    }

    #[test]
    fn test_dependency_chain() {
        let mut graph = BeliefGraph::new();
        let a = HypothesisId::new();
        let b = HypothesisId::new();
        let c = HypothesisId::new();
        let d = HypothesisId::new();

        // A depends on B, B depends on C, C depends on D
        graph.add_dependency(a, b).unwrap();
        graph.add_dependency(b, c).unwrap();
        graph.add_dependency(c, d).unwrap();

        // A's dependency chain: {B, C, D}
        let chain = graph.dependency_chain(a).unwrap();
        assert_eq!(chain.len(), 3);
        assert!(chain.contains(&b));
        assert!(chain.contains(&c));
        assert!(chain.contains(&d));
    }

    #[test]
    fn test_cycle_detection_prevents_cycle() {
        let mut graph = BeliefGraph::new();
        let a = HypothesisId::new();
        let b = HypothesisId::new();
        let c = HypothesisId::new();

        graph.add_dependency(a, b).unwrap();
        graph.add_dependency(b, c).unwrap();

        // Adding C -> A would create a cycle
        let result = graph.add_dependency(c, a);
        assert!(result.is_err());
    }

    #[test]
    fn test_detect_existing_cycles() {
        let mut graph = BeliefGraph::new();
        let a = HypothesisId::new();
        let b = HypothesisId::new();

        // Create a cycle directly (bypassing add_dependency check for testing)
        graph.get_or_create_node(a);
        graph.get_or_create_node(b);
        let from_idx = *graph.node_indices.get(&a).unwrap();
        let to_idx = *graph.node_indices.get(&b).unwrap();
        graph.graph.add_edge(from_idx, to_idx, ());
        graph.graph.add_edge(to_idx, from_idx, ());

        let cycles = graph.detect_cycles();
        assert_eq!(cycles.len(), 1);
    }
}

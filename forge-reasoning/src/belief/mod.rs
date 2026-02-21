//! Belief dependency graph with cycle detection
//!
//! This module provides a directed graph model for dependencies between beliefs (hypotheses).
//! It enables automatic cycle detection using Tarjan's SCC algorithm and provides query APIs
//! for dependency chains.

mod graph;
mod scc;

pub use graph::BeliefGraph;

use crate::hypothesis::HypothesisBoard;
use crate::hypothesis::types::HypothesisId;
use crate::errors::Result;

/// Combined reasoning system with hypotheses and belief dependencies
pub struct ReasoningSystem {
    pub board: HypothesisBoard,
    pub graph: BeliefGraph,
}

impl ReasoningSystem {
    pub fn new(board: HypothesisBoard) -> Self {
        Self {
            board,
            graph: BeliefGraph::new(),
        }
    }

    pub fn in_memory() -> Self {
        Self::new(HypothesisBoard::in_memory())
    }

    /// Create a dependency edge between hypotheses
    pub async fn add_dependency(
        &mut self,
        hypothesis_id: HypothesisId,
        depends_on: HypothesisId,
    ) -> Result<()> {
        // Verify both hypotheses exist
        if self.board.get(hypothesis_id).await?.is_none() {
            return Err(crate::errors::ReasoningError::NotFound(
                format!("Hypothesis {} not found", hypothesis_id)
            ));
        }
        if self.board.get(depends_on).await?.is_none() {
            return Err(crate::errors::ReasoningError::NotFound(
                format!("Hypothesis {} not found", depends_on)
            ));
        }

        self.graph.add_dependency(hypothesis_id, depends_on)
    }

    /// Remove a dependency edge
    pub fn remove_dependency(
        &mut self,
        hypothesis_id: HypothesisId,
        depends_on: HypothesisId,
    ) -> Result<bool> {
        self.graph.remove_dependency(hypothesis_id, depends_on)
    }

    /// Get dependents (what depends on this hypothesis)
    pub fn dependents(&self, hypothesis_id: HypothesisId) -> Result<indexmap::IndexSet<HypothesisId>> {
        self.graph.dependents(hypothesis_id)
    }

    /// Get dependees (what this hypothesis depends on)
    pub fn dependees(&self, hypothesis_id: HypothesisId) -> Result<indexmap::IndexSet<HypothesisId>> {
        self.graph.dependees(hypothesis_id)
    }

    /// Get full dependency chain
    pub fn dependency_chain(&self, hypothesis_id: HypothesisId) -> Result<indexmap::IndexSet<HypothesisId>> {
        self.graph.dependency_chain(hypothesis_id)
    }

    /// Detect cycles in the belief graph
    pub fn detect_cycles(&self) -> Vec<Vec<HypothesisId>> {
        self.graph.detect_cycles()
    }

    /// Remove a hypothesis and its graph node
    pub async fn remove_hypothesis(&mut self, id: HypothesisId) -> Result<bool> {
        let removed_from_board = self.board.delete(id).await?;
        let removed_from_graph = self.graph.remove_hypothesis(id)?;
        Ok(removed_from_board || removed_from_graph)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hypothesis::confidence::Confidence;

    #[tokio::test]
    async fn test_reasoning_system_integration() {
        let mut system = ReasoningSystem::in_memory();

        // Create hypotheses
        let h1 = system.board.propose("H1", Confidence::new(0.5).unwrap()).await.unwrap();
        let h2 = system.board.propose("H2", Confidence::new(0.5).unwrap()).await.unwrap();
        let h3 = system.board.propose("H3", Confidence::new(0.5).unwrap()).await.unwrap();

        // Create dependencies: H1 depends on H2, H2 depends on H3
        system.add_dependency(h1, h2).await.unwrap();
        system.add_dependency(h2, h3).await.unwrap();

        // Query dependency chain
        let chain = system.dependency_chain(h1).unwrap();
        assert_eq!(chain.len(), 2);
        assert!(chain.contains(&h2));
        assert!(chain.contains(&h3));

        // Detect cycles (should be none)
        assert_eq!(system.detect_cycles().len(), 0);
    }

    #[tokio::test]
    async fn test_cycle_prevention() {
        let mut system = ReasoningSystem::in_memory();

        let h1 = system.board.propose("H1", Confidence::new(0.5).unwrap()).await.unwrap();
        let h2 = system.board.propose("H2", Confidence::new(0.5).unwrap()).await.unwrap();

        system.add_dependency(h1, h2).await.unwrap();

        // Try to create a cycle
        let result = system.add_dependency(h2, h1).await;
        assert!(result.is_err());
    }
}

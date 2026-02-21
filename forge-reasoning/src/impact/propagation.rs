//! Confidence propagation through dependency graphs

use crate::hypothesis::{Confidence, HypothesisBoard, HypothesisId};
use crate::belief::BeliefGraph;
use crate::errors::{Result, ReasoningError};

/// Configuration for confidence propagation
#[derive(Clone, Debug)]
pub struct PropagationConfig {
    /// Decay factor per dependency level (default: 0.95)
    pub decay_factor: f64,
    /// Minimum confidence floor (default: 0.1)
    pub min_confidence: f64,
    /// Maximum cascade size limit (default: 10000)
    pub max_cascade_size: usize,
}

impl Default for PropagationConfig {
    fn default() -> Self {
        Self {
            decay_factor: 0.95,
            min_confidence: 0.1,
            max_cascade_size: 10000,
        }
    }
}

/// Result of confidence propagation
#[derive(Clone, Debug)]
pub struct PropagationResult {
    pub changes: Vec<ConfidenceChange>,
    pub cycles_detected: bool,
    pub normalized_cycles: usize,
    pub total_affected: usize,
    pub max_depth: usize,
}

/// Represents a change in confidence for a single hypothesis
#[derive(Clone, Debug)]
pub struct ConfidenceChange {
    pub hypothesis_id: HypothesisId,
    pub hypothesis_name: String,
    pub old_confidence: Confidence,
    pub new_confidence: Confidence,
    pub delta: f64,
    pub depth: usize,
    pub propagation_path: Vec<HypothesisId>,
}

/// Error type for cascade operations
#[derive(Debug, thiserror::Error)]
pub enum CascadeError {
    #[error("Cascade too large: {size} hypotheses affected, limit is {limit}")]
    CascadeTooLarge { size: usize, limit: usize },
    #[error("Hypothesis not found: {0}")]
    HypothesisNotFound(HypothesisId),
    #[error("Confidence validation error: {0}")]
    ConfidenceError(#[from] crate::hypothesis::ConfidenceError),
    #[error("Cycle normalization failed")]
    CycleNormalizationFailed,
    #[error("Graph error: {0}")]
    GraphError(String),
}

/// Compute cascade impact with BFS traversal
pub async fn compute_cascade(
    start: HypothesisId,
    new_confidence: Confidence,
    board: &HypothesisBoard,
    graph: &BeliefGraph,
    config: &PropagationConfig,
) -> std::result::Result<PropagationResult, CascadeError> {
    use std::collections::{HashSet, VecDeque};

    // Verify start hypothesis exists
    let start_hypothesis = board.get(start).await
        .map_err(|e| CascadeError::GraphError(e.to_string()))?
        .ok_or(CascadeError::HypothesisNotFound(start))?;

    let mut changes = Vec::new();
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();

    // Start node with depth 0
    queue.push_back((start, vec![start], 0));
    visited.insert(start);

    // BFS traversal
    while let Some((current_id, path, depth)) = queue.pop_front() {
        // Check cascade size limit
        if visited.len() > config.max_cascade_size {
            return Err(CascadeError::CascadeTooLarge {
                size: visited.len(),
                limit: config.max_cascade_size,
            });
        }

        // Get current hypothesis
        let hypothesis = board.get(current_id).await
            .map_err(|e| CascadeError::GraphError(e.to_string()))?
            .ok_or(CascadeError::HypothesisNotFound(current_id))?;

        let old_confidence = hypothesis.current_confidence();
        let name = hypothesis.statement().to_string();

        // Compute decayed confidence
        let decay_factor = config.decay_factor.powi(depth as i32);
        let decayed_value = new_confidence.get() * decay_factor;
        let decayed_value = decayed_value.max(config.min_confidence);
        let new_conf_for_hyp = Confidence::new(decayed_value)?;

        // Create change record
        let delta = new_conf_for_hyp.get() - old_confidence.get();
        changes.push(ConfidenceChange {
            hypothesis_id: current_id,
            hypothesis_name: name,
            old_confidence,
            new_confidence: new_conf_for_hyp,
            delta,
            depth,
            propagation_path: path.clone(),
        });

        // Get dependents (what depends on current hypothesis)
        if let Ok(dependents) = graph.dependents(current_id) {
            for dependent in dependents {
                if !visited.contains(&dependent) {
                    visited.insert(dependent);
                    let mut new_path = path.clone();
                    new_path.push(dependent);
                    queue.push_back((dependent, new_path, depth + 1));
                }
            }
        }
    }

    // Detect cycles
    let cycles_detected = graph.detect_cycles();
    let has_cycles = !cycles_detected.is_empty();

    let max_depth = changes.iter()
        .map(|c| c.depth)
        .max()
        .unwrap_or(0);

    Ok(PropagationResult {
        changes,
        cycles_detected: has_cycles,
        normalized_cycles: 0,
        total_affected: visited.len(),
        max_depth,
    })
}

/// Normalize cycles to average confidence
pub fn normalize_cycles(
    changes: &mut [ConfidenceChange],
    graph: &BeliefGraph,
) -> std::result::Result<usize, CascadeError> {
    use std::collections::HashSet;

    let cycles = graph.detect_cycles();
    let mut normalized_count = 0;

    for cycle in cycles {
        if cycle.len() <= 1 {
            continue; // Not a real cycle
        }

        // Find changes belonging to this cycle
        let cycle_ids: HashSet<_> = cycle.iter().cloned().collect();

        // Compute average confidence for this cycle
        let cycle_changes: Vec<_> = changes.iter()
            .filter(|c| cycle_ids.contains(&c.hypothesis_id))
            .collect();

        if cycle_changes.is_empty() {
            continue;
        }

        let avg_confidence: f64 = cycle_changes.iter()
            .map(|c| c.new_confidence.get())
            .sum::<f64>() / cycle_changes.len() as f64;

        // Update all cycle members to average confidence
        for change in changes.iter_mut() {
            if cycle_ids.contains(&change.hypothesis_id) {
                let avg_conf = Confidence::new(avg_confidence)?;
                change.new_confidence = avg_conf;
                change.delta = avg_conf.get() - change.old_confidence.get();
            }
        }

        normalized_count += 1;
    }

    Ok(normalized_count)
}

/// Apply confidence propagation changes
pub async fn propagate_confidence(
    result: PropagationResult,
    board: &std::sync::Arc<HypothesisBoard>,
) -> Result<()> {
    for change in result.changes {
        // Skip if hypothesis was deleted
        if board.get(change.hypothesis_id).await?.is_none() {
            continue;
        }
        board.update_confidence_direct(change.hypothesis_id, change.new_confidence).await?;
    }
    Ok(())
}

/// Query impact radius without applying changes
pub async fn impact_radius(
    start: HypothesisId,
    graph: &BeliefGraph,
) -> Result<usize> {
    use std::collections::{HashSet, VecDeque};

    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();

    queue.push_back(start);
    visited.insert(start);

    while let Some(current) = queue.pop_front() {
        if let Ok(dependents) = graph.dependents(current) {
            for dependent in dependents {
                if !visited.contains(&dependent) {
                    visited.insert(dependent);
                    queue.push_back(dependent);
                }
            }
        }
    }

    Ok(visited.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_propagation_config_default() {
        let config = PropagationConfig::default();
        assert_eq!(config.decay_factor, 0.95);
        assert_eq!(config.min_confidence, 0.1);
        assert_eq!(config.max_cascade_size, 10000);
    }

    #[test]
    fn test_cascade_error_display() {
        let err = CascadeError::CascadeTooLarge { size: 100, limit: 50 };
        assert!(err.to_string().contains("100"));
        assert!(err.to_string().contains("50"));
    }

    #[tokio::test]
    async fn test_compute_cascade_linear_chain() {
        let board = HypothesisBoard::in_memory();
        let mut graph = BeliefGraph::new();

        // Create hypotheses: A -> B -> C (A depends on B, B depends on C)
        let h_c = board.propose("C", Confidence::new(0.5).unwrap()).await.unwrap();
        let h_b = board.propose("B", Confidence::new(0.5).unwrap()).await.unwrap();
        let h_a = board.propose("A", Confidence::new(0.5).unwrap()).await.unwrap();

        // Create dependencies
        graph.add_dependency(h_b, h_c).unwrap();
        graph.add_dependency(h_a, h_b).unwrap();

        // Compute cascade from C with new confidence 0.9
        let new_conf = Confidence::new(0.9).unwrap();
        let config = PropagationConfig::default();
        let result = compute_cascade(h_c, new_conf, &board, &graph, &config).await.unwrap();

        // Should affect all 3 hypotheses
        assert_eq!(result.total_affected, 3);
        assert_eq!(result.changes.len(), 3);

        // Check depth assignments
        let change_c = &result.changes[0];
        let change_b = result.changes.iter().find(|c| c.hypothesis_id == h_b).unwrap();
        let change_a = result.changes.iter().find(|c| c.hypothesis_id == h_a).unwrap();

        assert_eq!(change_c.depth, 0);
        assert_eq!(change_b.depth, 1);
        assert_eq!(change_a.depth, 2);

        // Check decay application (0.95^1 = 0.95, 0.95^2 = 0.9025)
        assert!((change_b.new_confidence.get() - 0.855).abs() < 0.01); // 0.9 * 0.95
        assert!((change_a.new_confidence.get() - 0.81225).abs() < 0.01); // 0.9 * 0.95^2
    }

    #[tokio::test]
    async fn test_compute_cascade_min_confidence_floor() {
        let board = HypothesisBoard::in_memory();
        let mut graph = BeliefGraph::new();

        // Create hypotheses with deep chain
        let h_c = board.propose("C", Confidence::new(0.5).unwrap()).await.unwrap();
        let h_b = board.propose("B", Confidence::new(0.5).unwrap()).await.unwrap();
        let h_a = board.propose("A", Confidence::new(0.5).unwrap()).await.unwrap();

        graph.add_dependency(h_b, h_c).unwrap();
        graph.add_dependency(h_a, h_b).unwrap();

        // Set low confidence and high decay
        let new_conf = Confidence::new(0.2).unwrap();
        let config = PropagationConfig {
            decay_factor: 0.5,
            min_confidence: 0.15,
            max_cascade_size: 1000,
        };

        let result = compute_cascade(h_c, new_conf, &board, &graph, &config).await.unwrap();

        // A at depth 2: 0.2 * 0.5^2 = 0.05, but should be floored to 0.15
        let change_a = result.changes.iter().find(|c| c.hypothesis_id == h_a).unwrap();
        assert!(change_a.new_confidence.get() >= 0.15);
    }

    #[tokio::test]
    async fn test_cascade_too_large_error() {
        let board = HypothesisBoard::in_memory();
        let mut graph = BeliefGraph::new();

        // Create a small chain
        let h_a = board.propose("A", Confidence::new(0.5).unwrap()).await.unwrap();
        let h_b = board.propose("B", Confidence::new(0.5).unwrap()).await.unwrap();
        graph.add_dependency(h_b, h_a).unwrap();

        // Set very small cascade limit
        let new_conf = Confidence::new(0.9).unwrap();
        let config = PropagationConfig {
            decay_factor: 0.95,
            min_confidence: 0.1,
            max_cascade_size: 1, // Only 1 hypothesis allowed
        };

        let result = compute_cascade(h_a, new_conf, &board, &graph, &config).await;
        assert!(matches!(result, Err(CascadeError::CascadeTooLarge { .. })));
    }

    #[tokio::test]
    async fn test_hypothesis_not_found_error() {
        let board = HypothesisBoard::in_memory();
        let graph = BeliefGraph::new();
        let non_existent = HypothesisId::new();

        let new_conf = Confidence::new(0.9).unwrap();
        let config = PropagationConfig::default();

        let result = compute_cascade(non_existent, new_conf, &board, &graph, &config).await;
        assert!(matches!(result, Err(CascadeError::HypothesisNotFound(_))));
    }

    #[tokio::test]
    async fn test_normalize_cycles() {
        let board = HypothesisBoard::in_memory();
        let graph = BeliefGraph::new();

        // Create changes for testing (no actual cycle in graph)
        let h_a = HypothesisId::new();
        let h_b = HypothesisId::new();

        let mut changes = vec![
            ConfidenceChange {
                hypothesis_id: h_a,
                hypothesis_name: "A".to_string(),
                old_confidence: Confidence::new(0.5).unwrap(),
                new_confidence: Confidence::new(0.8).unwrap(),
                delta: 0.3,
                depth: 0,
                propagation_path: vec![h_a],
            },
            ConfidenceChange {
                hypothesis_id: h_b,
                hypothesis_name: "B".to_string(),
                old_confidence: Confidence::new(0.5).unwrap(),
                new_confidence: Confidence::new(0.7).unwrap(),
                delta: 0.2,
                depth: 1,
                propagation_path: vec![h_a, h_b],
            },
        ];

        // Normalize cycles with empty graph (no cycles to normalize)
        let normalized = normalize_cycles(&mut changes, &graph).unwrap();
        assert_eq!(normalized, 0);

        // Changes should remain unchanged
        assert!((changes[0].new_confidence.get() - 0.8).abs() < 0.01);
        assert!((changes[1].new_confidence.get() - 0.7).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_propagate_confidence() {
        let board = std::sync::Arc::new(HypothesisBoard::in_memory());
        let mut graph = BeliefGraph::new();

        // Create hypotheses
        let h_a = board.propose("A", Confidence::new(0.5).unwrap()).await.unwrap();
        let h_b = board.propose("B", Confidence::new(0.5).unwrap()).await.unwrap();

        graph.add_dependency(h_b, h_a).unwrap();

        // Create propagation result
        let result = PropagationResult {
            changes: vec![
                ConfidenceChange {
                    hypothesis_id: h_a,
                    hypothesis_name: "A".to_string(),
                    old_confidence: Confidence::new(0.5).unwrap(),
                    new_confidence: Confidence::new(0.8).unwrap(),
                    delta: 0.3,
                    depth: 0,
                    propagation_path: vec![h_a],
                },
                ConfidenceChange {
                    hypothesis_id: h_b,
                    hypothesis_name: "B".to_string(),
                    old_confidence: Confidence::new(0.5).unwrap(),
                    new_confidence: Confidence::new(0.76).unwrap(),
                    delta: 0.26,
                    depth: 1,
                    propagation_path: vec![h_a, h_b],
                },
            ],
            cycles_detected: false,
            normalized_cycles: 0,
            total_affected: 2,
            max_depth: 1,
        };

        // Apply propagation
        propagate_confidence(result, &board).await.unwrap();

        // Verify confidences were updated
        let h_a_updated = board.get(h_a).await.unwrap().unwrap();
        let h_b_updated = board.get(h_b).await.unwrap().unwrap();

        assert!((h_a_updated.current_confidence().get() - 0.8).abs() < 0.01);
        assert!((h_b_updated.current_confidence().get() - 0.76).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_impact_radius() {
        let board = HypothesisBoard::in_memory();
        let mut graph = BeliefGraph::new();

        // Create chain: A depends on B, B depends on C
        let h_c = board.propose("C", Confidence::new(0.5).unwrap()).await.unwrap();
        let h_b = board.propose("B", Confidence::new(0.5).unwrap()).await.unwrap();
        let h_a = board.propose("A", Confidence::new(0.5).unwrap()).await.unwrap();

        graph.add_dependency(h_b, h_c).unwrap();
        graph.add_dependency(h_a, h_b).unwrap();

        // Impact radius from C should be 3 (C, B, A all affected)
        let radius = impact_radius(h_c, &graph).await.unwrap();
        assert_eq!(radius, 3);

        // Impact radius from A should be 1 (only A affected)
        let radius_a = impact_radius(h_a, &graph).await.unwrap();
        assert_eq!(radius_a, 1);
    }
}

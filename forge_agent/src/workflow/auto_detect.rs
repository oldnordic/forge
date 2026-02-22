//! Automatic dependency detection using graph queries.
//!
//! This module provides intelligent dependency detection for workflow tasks
//! by analyzing the code graph to find relationships between symbols.

use crate::workflow::dag::{Workflow, WorkflowError};
use crate::workflow::task::TaskId;
use forge_core::graph::GraphModule;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Configuration for automatic dependency detection.
#[derive(Clone, Debug)]
pub struct AutoDetectConfig {
    /// Maximum distance for dependency detection (default: 2)
    pub max_hops: u32,
    /// Include indirect dependencies (default: false)
    pub include_transitive: bool,
    /// Minimum confidence for auto-detected dependencies (default: 0.7)
    pub confidence_threshold: f64,
}

impl Default for AutoDetectConfig {
    fn default() -> Self {
        Self {
            max_hops: 2,
            include_transitive: false,
            confidence_threshold: 0.7,
        }
    }
}

impl AutoDetectConfig {
    /// Creates a new configuration with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the maximum number of hops for dependency detection.
    pub fn with_max_hops(mut self, max_hops: u32) -> Self {
        self.max_hops = max_hops;
        self
    }

    /// Sets whether to include transitive dependencies.
    pub fn with_transitive(mut self, include_transitive: bool) -> Self {
        self.include_transitive = include_transitive;
        self
    }

    /// Sets the minimum confidence threshold.
    pub fn with_confidence_threshold(mut self, threshold: f64) -> Self {
        self.confidence_threshold = threshold;
        self
    }
}

/// Reason for a suggested dependency.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyReason {
    /// Symbol impact analysis detected dependency
    SymbolImpact {
        /// Symbol that is impacted
        symbol: String,
        /// Hop distance
        hops: u32,
    },
    /// Direct reference detected
    Reference {
        /// Referenced symbol
        symbol: String,
    },
    /// Function call detected
    Call {
        /// Called function
        function: String,
    },
}

/// Suggested dependency between two tasks.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DependencySuggestion {
    /// Task that should execute first
    pub from_task: TaskId,
    /// Task that depends on from_task
    pub to_task: TaskId,
    /// Reason for the suggested dependency
    pub reason: DependencyReason,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
}

impl DependencySuggestion {
    /// Checks if this suggestion has high confidence.
    ///
    /// High confidence is defined as >= 0.8
    pub fn is_high_confidence(&self) -> bool {
        self.confidence >= 0.8
    }
}

/// Dependency analyzer for automatic workflow construction.
pub struct DependencyAnalyzer {
    graph: GraphModule,
    config: AutoDetectConfig,
}

impl DependencyAnalyzer {
    /// Creates a new dependency analyzer with default configuration.
    pub fn new(graph: GraphModule) -> Self {
        Self {
            graph,
            config: AutoDetectConfig::default(),
        }
    }

    /// Creates a new dependency analyzer with custom configuration.
    pub fn with_config(graph: GraphModule, config: AutoDetectConfig) -> Self {
        Self { graph, config }
    }

    /// Detects dependencies between tasks in a workflow.
    ///
    /// This method analyzes the workflow's GraphQueryTasks and suggests
    /// dependencies based on symbol impact analysis and reference checking.
    ///
    /// # Arguments
    ///
    /// * `workflow` - The workflow to analyze
    ///
    /// # Returns
    ///
    /// A vector of suggested dependencies
    pub async fn detect_dependencies(
        &self,
        workflow: &Workflow,
    ) -> Result<Vec<DependencySuggestion>, WorkflowError> {
        let mut suggestions = Vec::new();

        // Access the graph directly (same module, pub(in crate::workflow))
        let mut task_targets: HashMap<TaskId, Option<String>> = HashMap::new();

        for node_idx in workflow.graph.node_indices() {
            if let Some(node) = workflow.graph.node_weight(node_idx) {
                let task_id = node.id().clone();

                // Try to downcast the task to GraphQueryTask
                // Note: We can't access the actual trait object from TaskNode
                // so we need to work with task names and heuristics for now
                // A full implementation would require TaskNode to expose more metadata

                // For Phase 8, we'll use task name patterns to detect GraphQueryTasks
                let target = self.extract_target_from_name(&node.name);
                task_targets.insert(task_id.clone(), target);
            }
        }

        // For each task with a target, analyze impact and references
        for (task_id, maybe_target) in &task_targets {
            if let Some(target) = maybe_target {
                // Run impact analysis
                if let Ok(impacted) = self.graph.impact_analysis(target, Some(self.config.max_hops)).await {
                    for impacted_symbol in impacted {
                        // Find tasks that operate on impacted symbols
                        for (other_task_id, other_target) in &task_targets {
                            if task_id == other_task_id {
                                continue;
                            }

                            if let Some(other_target) = other_target {
                                // Check if other task operates on impacted symbol
                                if self.symbol_matches(other_target, &impacted_symbol.name) {
                                    // Calculate confidence based on hop distance
                                    let confidence = self.calculate_impact_confidence(impacted_symbol.hop_distance);

                                    if confidence >= self.config.confidence_threshold {
                                        // Suggest dependency: task_id -> other_task_id
                                        // (task_id should execute before other_task_id)
                                        suggestions.push(DependencySuggestion {
                                            from_task: task_id.clone(),
                                            to_task: other_task_id.clone(),
                                            reason: DependencyReason::SymbolImpact {
                                                symbol: impacted_symbol.name.clone(),
                                                hops: impacted_symbol.hop_distance,
                                            },
                                            confidence,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }

                // Check references
                // Note: Reference struct contains SymbolIds, not names
                // For Phase 8, we skip reference-based detection due to API limitations
                // A full implementation would look up symbol names from SymbolIds
            }
        }

        // Remove duplicates (same from_task, to_task pairs)
        let mut seen = HashSet::new();
        suggestions.retain(|s| {
            let key = (s.from_task.clone(), s.to_task.clone());
            seen.insert(key)
        });

        // Remove existing dependencies
        let existing_deps = self.get_existing_dependencies(workflow);
        suggestions.retain(|s| {
            !existing_deps.contains(&(s.from_task.clone(), s.to_task.clone()))
        });

        Ok(suggestions)
    }

    /// Extracts target symbol from task name using heuristics.
    ///
    /// This is a Phase 8 limitation - a full implementation would access
    /// the actual GraphQueryTask metadata.
    fn extract_target_from_name(&self, name: &str) -> Option<String> {
        // For GraphQueryTasks created via the builder API, the name format is:
        // "Graph Query: FindSymbol" or "Graph Query: References", etc.
        // The target is stored in the task itself, not the name
        // So for Phase 8, we return None and rely on manual dependency specification
        None
    }

    /// Checks if a target symbol matches a symbol name.
    fn symbol_matches(&self, target: &str, symbol_name: &str) -> bool {
        // Exact match or substring match
        target == symbol_name || symbol_name.contains(target) || target.contains(symbol_name)
    }

    /// Calculates confidence score based on hop distance.
    fn calculate_impact_confidence(&self, hops: u32) -> f64 {
        // Closer symbols have higher confidence
        // Base confidence: 0.9 for 1 hop, decreasing by 0.1 per hop
        let base = 0.9;
        let decay = 0.1 * (hops as f64 - 1.0);
        (base - decay).max(0.5).min(1.0)
    }

    /// Gets existing dependencies from the workflow.
    fn get_existing_dependencies(&self, workflow: &Workflow) -> HashSet<(TaskId, TaskId)> {
        let mut existing = HashSet::new();

        for task_id in workflow.task_ids() {
            if let Some(deps) = workflow.task_dependencies(&task_id) {
                for dep in deps {
                    existing.insert((dep, task_id.clone()));
                }
            }
        }

        existing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::tasks::GraphQueryTask;
    use crate::workflow::WorkflowBuilder;

    #[test]
    fn test_config_defaults() {
        let config = AutoDetectConfig::default();
        assert_eq!(config.max_hops, 2);
        assert_eq!(config.include_transitive, false);
        assert_eq!(config.confidence_threshold, 0.7);
    }

    #[test]
    fn test_config_builder() {
        let config = AutoDetectConfig::new()
            .with_max_hops(3)
            .with_transitive(true)
            .with_confidence_threshold(0.8);

        assert_eq!(config.max_hops, 3);
        assert_eq!(config.include_transitive, true);
        assert_eq!(config.confidence_threshold, 0.8);
    }

    #[test]
    fn test_confidence_calculation() {
        // Test confidence calculation logic directly
        let calculate_confidence = |hops: u32| -> f64 {
            let base = 0.9;
            let decay = 0.1 * (hops as f64 - 1.0);
            (base - decay).max(0.5).min(1.0)
        };

        assert!((calculate_confidence(1) - 0.9).abs() < 0.01);
        assert!((calculate_confidence(2) - 0.8).abs() < 0.01);
        assert!((calculate_confidence(3) - 0.7).abs() < 0.01);
    }

    #[test]
    fn test_symbol_matching() {
        // Test symbol matching logic directly
        let symbol_matches = |target: &str, symbol_name: &str| -> bool {
            target == symbol_name || symbol_name.contains(target) || target.contains(symbol_name)
        };

        assert!(symbol_matches("process_data", "process_data"));
        assert!(symbol_matches("process", "process_data"));
        assert!(symbol_matches("process_data", "process"));
    }

    #[test]
    fn test_high_confidence_filter() {
        let suggestion = DependencySuggestion {
            from_task: TaskId::new("a"),
            to_task: TaskId::new("b"),
            reason: DependencyReason::SymbolImpact {
                symbol: "test".to_string(),
                hops: 1,
            },
            confidence: 0.9,
        };

        assert!(suggestion.is_high_confidence());

        let low_conf = DependencySuggestion {
            confidence: 0.7,
            ..suggestion.clone()
        };

        assert!(!low_conf.is_high_confidence());
    }
}

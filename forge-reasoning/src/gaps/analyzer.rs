//! Knowledge gap analyzer core types
//!
//! Provides the main data structures for tracking knowledge gaps with multi-factor
//! priority scoring and auto-close capabilities.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::hypothesis::HypothesisBoard;
use crate::hypothesis::types::HypothesisId;
use crate::belief::BeliefGraph;

/// Unique identifier for a knowledge gap
///
/// UUID v4 wrapper following the same pattern as CheckpointId and HypothesisId.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GapId(uuid::Uuid);

impl GapId {
    /// Create a new random GapId
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    /// Create GapId from UUID bytes
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(uuid::Uuid::from_bytes(bytes))
    }

    /// Get GapId as UUID bytes
    pub fn as_bytes(&self) -> [u8; 16] {
        self.0.as_bytes().clone()
    }
}

impl Default for GapId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for GapId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Criticality level of a knowledge gap
///
/// Determines the base priority weight in scoring calculations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum GapCriticality {
    /// Low priority gap - nice to have but not blocking
    Low,
    /// Medium priority gap - important but not urgent
    Medium,
    /// High priority gap - blocking investigation
    High,
}

/// Type of knowledge gap
///
/// Categorizes the nature of the missing information for context-aware suggestions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GapType {
    /// Missing data or facts needed for investigation
    MissingInformation,
    /// Hypothesis or assumption not yet verified
    UntestedAssumption,
    /// Conflicting evidence or contradictory signals
    ContradictoryEvidence,
    /// Unknown relationship or dependency between hypotheses
    UnknownDependency,
    /// Flexible catch-all for other gap types
    Other(String),
}

/// A knowledge gap with scoring factors
///
/// Tracks missing information with computed priority score based on multiple factors:
/// - Criticality (High/Medium/Low)
/// - Dependency depth (deeper = higher priority)
/// - Evidence strength (less evidence = higher priority)
/// - Age (older gaps = higher priority, capped at 30 days)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KnowledgeGap {
    /// Unique identifier
    pub id: GapId,
    /// Human-readable description
    pub description: String,
    /// Links to hypothesis if applicable
    pub hypothesis_id: Option<HypothesisId>,
    /// Criticality level
    pub criticality: GapCriticality,
    /// Type of gap
    pub gap_type: GapType,
    /// When the gap was registered
    pub created_at: DateTime<Utc>,
    /// When the gap was filled (None if still open)
    pub filled_at: Option<DateTime<Utc>>,
    /// Resolution notes if filled
    pub resolution_notes: Option<String>,
    /// Computed multi-factor priority score (0.0 to 1.0)
    pub score: f64,
    /// Dependency depth (0 if no hypothesis)
    pub depth: usize,
    /// Average evidence strength at linked hypothesis
    pub evidence_strength: f64,
}

/// Scoring configuration for multi-factor gap priority
///
/// Weights for each factor in the priority score calculation.
/// All weights should sum to 1.0 for normalized scoring.
#[derive(Clone, Debug)]
pub struct ScoringConfig {
    /// Weight for criticality factor (default: 0.5)
    pub criticality_weight: f64,
    /// Weight for dependency depth factor (default: 0.3)
    pub depth_weight: f64,
    /// Weight for evidence strength factor (default: 0.15)
    pub evidence_weight: f64,
    /// Weight for age factor (default: 0.05)
    pub age_weight: f64,
    /// Auto-close threshold for hypothesis confidence (default: 0.9)
    pub auto_close_threshold: f64,
}

impl Default for ScoringConfig {
    fn default() -> Self {
        Self {
            criticality_weight: 0.5,
            depth_weight: 0.3,
            evidence_weight: 0.15,
            age_weight: 0.05,
            auto_close_threshold: 0.9,
        }
    }
}

/// Action suggestion for resolving a knowledge gap
///
/// Generated based on gap type, hypothesis context, and dependency relationships.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GapSuggestion {
    /// ID of the gap this suggestion addresses
    pub gap_id: GapId,
    /// Suggested action to take
    pub action: SuggestedAction,
    /// Human-readable rationale for this suggestion
    pub rationale: String,
    /// Priority score (copied from gap)
    pub priority: f64,
}

/// Suggested action for resolving a knowledge gap
///
/// Action types are context-aware based on gap type and linked hypothesis state.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SuggestedAction {
    /// Run a specific test to gather evidence
    RunTest { test_name: String },
    /// Investigate a specific area
    Investigate { area: String, details: String },
    /// Gather evidence for a hypothesis
    GatherEvidence { hypothesis_id: HypothesisId },
    /// Resolve a dependency between hypotheses
    ResolveDependency { dependent_id: HypothesisId, dependee_id: HypothesisId },
    /// Create a verification check for a hypothesis
    CreateVerificationCheck { command: String, hypothesis_id: HypothesisId },
    /// Research a specific topic
    Research { topic: String },
    /// Other action (flexible catch-all)
    Other { description: String },
}

/// Knowledge gap analyzer with multi-factor priority scoring
///
/// Main API for registering, tracking, and resolving knowledge gaps.
/// Automatically computes priority scores and provides action suggestions.
pub struct KnowledgeGapAnalyzer {
    /// Hypothesis board for confidence queries
    board: Arc<HypothesisBoard>,
    /// Belief dependency graph for depth queries
    graph: Arc<BeliefGraph>,
    /// All gaps (filled and unfilled)
    gaps: HashMap<GapId, KnowledgeGap>,
    /// Scoring configuration
    scoring_config: ScoringConfig,
}

impl KnowledgeGapAnalyzer {
    /// Create new analyzer with default scoring config
    pub fn new(board: Arc<HypothesisBoard>, graph: Arc<BeliefGraph>) -> Self {
        Self {
            board,
            graph,
            gaps: HashMap::new(),
            scoring_config: ScoringConfig::default(),
        }
    }

    /// Set custom scoring configuration (builder pattern)
    pub fn with_scoring_config(mut self, config: ScoringConfig) -> Self {
        self.scoring_config = config;
        self
    }

    /// Register a new knowledge gap
    ///
    /// Computes initial depth and evidence strength from linked hypothesis.
    pub async fn register_gap(
        &mut self,
        description: String,
        criticality: GapCriticality,
        gap_type: GapType,
        hypothesis_id: Option<HypothesisId>,
    ) -> crate::errors::Result<GapId> {
        let id = GapId::new();
        let created_at = Utc::now();

        // Compute depth from dependency graph
        let depth = if let Some(hid) = hypothesis_id {
            self.compute_depth(hid).await
        } else {
            0
        };

        // Compute evidence strength from hypothesis board
        let evidence_strength = if let Some(hid) = hypothesis_id {
            self.compute_evidence_strength(hid).await
        } else {
            0.0
        };

        // Create gap
        let gap = KnowledgeGap {
            id,
            description,
            hypothesis_id,
            criticality,
            gap_type,
            created_at,
            filled_at: None,
            resolution_notes: None,
            score: 0.0, // Will be computed below
            depth,
            evidence_strength,
        };

        // Compute initial score
        let score = super::scoring::compute_gap_score(&gap, &self.scoring_config);

        let mut gap = gap;
        gap.score = score;

        self.gaps.insert(id, gap);
        Ok(id)
    }

    /// Mark a gap as filled with resolution notes
    pub async fn fill_gap(
        &mut self,
        gap_id: GapId,
        resolution_notes: String,
    ) -> crate::errors::Result<()> {
        let gap = self.gaps.get_mut(&gap_id)
            .ok_or_else(|| crate::errors::ReasoningError::NotFound(
                format!("Gap {} not found", gap_id)
            ))?;

        gap.filled_at = Some(Utc::now());
        gap.resolution_notes = Some(resolution_notes);
        Ok(())
    }

    /// List all gaps, optionally filtering unfilled only
    ///
    /// Returns gaps sorted by score descending (highest priority first).
    pub async fn list_gaps(&self, unfilled_only: bool) -> Vec<KnowledgeGap> {
        let mut gaps: Vec<_> = self.gaps.values()
            .filter(|g| !unfilled_only || g.filled_at.is_none())
            .cloned()
            .collect();

        // Sort by score descending (highest priority first)
        gaps.sort_by(|a, b| {
            b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
        });

        gaps
    }

    /// Get a specific gap by ID
    pub fn get_gap(&self, gap_id: GapId) -> Option<&KnowledgeGap> {
        self.gaps.get(&gap_id)
    }

    /// Auto-close gaps where linked hypothesis reached high confidence
    ///
    /// Returns list of closed gap IDs.
    pub async fn auto_close_gaps(&mut self) -> Vec<GapId> {
        let mut closed = Vec::new();

        for gap in self.gaps.values_mut() {
            // Only consider unfilled gaps with linked hypothesis
            if gap.filled_at.is_some() {
                continue;
            }

            let hypothesis_id = match gap.hypothesis_id {
                Some(hid) => hid,
                None => continue,
            };

            // Query hypothesis confidence
            let hypothesis = match self.board.get(hypothesis_id).await {
                Ok(Some(h)) => h,
                _ => continue,
            };

            // Check if confidence exceeds threshold
            if hypothesis.current_confidence().get() > self.scoring_config.auto_close_threshold {
                gap.filled_at = Some(Utc::now());
                gap.resolution_notes = Some(
                    "Auto-closed: hypothesis reached high confidence".to_string()
                );
                closed.push(gap.id);
            }
        }

        closed
    }

    /// Get action suggestions for gaps
    ///
    /// Returns context-aware suggestions sorted by priority.
    pub async fn get_suggestions(&self, unfilled_only: bool) -> Vec<GapSuggestion> {
        let gaps = self.list_gaps(unfilled_only).await;
        super::suggestions::generate_all_suggestions(&gaps, &self.board, &self.graph)
    }

    /// Recompute all gap scores (useful after config changes)
    pub fn recompute_scores(&mut self) {
        super::scoring::recompute_all_scores(&mut self.gaps, &self.scoring_config);
    }

    /// Compute dependency depth for a hypothesis
    ///
    /// Returns maximum depth of dependency chain (longest path to root).
    async fn compute_depth(&self, hypothesis_id: HypothesisId) -> usize {
        // Get full dependency chain
        match self.graph.dependency_chain(hypothesis_id) {
            Ok(chain) => chain.len(),
            Err(_) => 0,
        }
    }

    /// Compute average evidence strength for a hypothesis
    async fn compute_evidence_strength(&self, hypothesis_id: HypothesisId) -> f64 {
        match self.board.list_evidence(hypothesis_id).await {
            Ok(evidence_list) => {
                if evidence_list.is_empty() {
                    0.0
                } else {
                    let total: f64 = evidence_list.iter()
                        .map(|e| e.strength().abs())
                        .sum();
                    total / evidence_list.len() as f64
                }
            }
            Err(_) => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hypothesis::confidence::Confidence;

    #[tokio::test]
    async fn test_gap_id_uniqueness() {
        let id1 = GapId::new();
        let id2 = GapId::new();
        assert_ne!(id1, id2);
    }

    #[tokio::test]
    async fn test_scoring_config_default() {
        let config = ScoringConfig::default();
        assert_eq!(config.criticality_weight, 0.5);
        assert_eq!(config.depth_weight, 0.3);
        assert_eq!(config.evidence_weight, 0.15);
        assert_eq!(config.age_weight, 0.05);
        assert_eq!(config.auto_close_threshold, 0.9);
    }

    #[tokio::test]
    async fn test_register_gap() {
        let board = Arc::new(HypothesisBoard::in_memory());
        let graph = Arc::new(BeliefGraph::new());
        let mut analyzer = KnowledgeGapAnalyzer::new(board, graph);

        let gap_id = analyzer.register_gap(
            "Test gap".to_string(),
            GapCriticality::Medium,
            GapType::MissingInformation,
            None,
        ).await.unwrap();

        let gap = analyzer.get_gap(gap_id);
        assert!(gap.is_some());
        assert_eq!(gap.unwrap().description, "Test gap");
    }

    #[tokio::test]
    async fn test_fill_gap() {
        let board = Arc::new(HypothesisBoard::in_memory());
        let graph = Arc::new(BeliefGraph::new());
        let mut analyzer = KnowledgeGapAnalyzer::new(board, graph);

        let gap_id = analyzer.register_gap(
            "Test gap".to_string(),
            GapCriticality::Low,
            GapType::UntestedAssumption,
            None,
        ).await.unwrap();

        analyzer.fill_gap(gap_id, "Resolved".to_string()).await.unwrap();

        let gap = analyzer.get_gap(gap_id).unwrap();
        assert!(gap.filled_at.is_some());
        assert_eq!(gap.resolution_notes, Some("Resolved".to_string()));
    }

    #[tokio::test]
    async fn test_list_gaps_sorts_by_priority() {
        let board = Arc::new(HypothesisBoard::in_memory());
        let graph = Arc::new(BeliefGraph::new());
        let mut analyzer = KnowledgeGapAnalyzer::new(board, graph);

        // Register gaps with different criticality
        analyzer.register_gap(
            "Low priority".to_string(),
            GapCriticality::Low,
            GapType::MissingInformation,
            None,
        ).await.unwrap();

        analyzer.register_gap(
            "High priority".to_string(),
            GapCriticality::High,
            GapType::MissingInformation,
            None,
        ).await.unwrap();

        let gaps = analyzer.list_gaps(false).await;
        // High priority should come first
        assert_eq!(gaps[0].criticality, GapCriticality::High);
        assert_eq!(gaps[1].criticality, GapCriticality::Low);
    }

    #[tokio::test]
    async fn test_register_gap_computes_score_correctly() {
        let board = Arc::new(HypothesisBoard::in_memory());
        let graph = Arc::new(BeliefGraph::new());
        let mut analyzer = KnowledgeGapAnalyzer::new(board, graph);

        let gap_id = analyzer.register_gap(
            "Test gap".to_string(),
            GapCriticality::High,
            GapType::MissingInformation,
            None,
        ).await.unwrap();

        let gap = analyzer.get_gap(gap_id).unwrap();
        // Score should be > 0 since High criticality
        assert!(gap.score > 0.0);
        assert!(gap.score <= 1.0);
    }

    #[tokio::test]
    async fn test_auto_close_gaps_closes_high_confidence_hypotheses() {
        let board = Arc::new(HypothesisBoard::in_memory());
        let graph = Arc::new(BeliefGraph::new());
        let mut analyzer = KnowledgeGapAnalyzer::new(board.clone(), graph);

        // Create hypothesis with high confidence
        let prior = Confidence::new(0.95).unwrap();
        let h_id = board.propose("High confidence hypothesis", prior).await.unwrap();

        // Register gap linked to this hypothesis
        let gap_id = analyzer.register_gap(
            "Test gap".to_string(),
            GapCriticality::Medium,
            GapType::UntestedAssumption,
            Some(h_id),
        ).await.unwrap();

        // Auto-close should close this gap
        let closed = analyzer.auto_close_gaps().await;
        assert_eq!(closed.len(), 1);
        assert_eq!(closed[0], gap_id);

        // Verify gap is filled
        let gap = analyzer.get_gap(gap_id).unwrap();
        assert!(gap.filled_at.is_some());
        assert!(gap.resolution_notes.is_some());
    }

    #[tokio::test]
    async fn test_get_suggestions_returns_sorted_list() {
        let board = Arc::new(HypothesisBoard::in_memory());
        let graph = Arc::new(BeliefGraph::new());
        let mut analyzer = KnowledgeGapAnalyzer::new(board, graph);

        // Register gaps
        analyzer.register_gap(
            "Low priority".to_string(),
            GapCriticality::Low,
            GapType::MissingInformation,
            None,
        ).await.unwrap();

        analyzer.register_gap(
            "High priority".to_string(),
            GapCriticality::High,
            GapType::UntestedAssumption,
            None,
        ).await.unwrap();

        // Get suggestions
        let suggestions = analyzer.get_suggestions(true).await;

        // Should have 2 suggestions
        assert_eq!(suggestions.len(), 2);

        // Should be sorted by priority
        assert!(suggestions[0].priority >= suggestions[1].priority);
    }

    #[tokio::test]
    async fn test_recompute_scores_updates_all_gaps() {
        let board = Arc::new(HypothesisBoard::in_memory());
        let graph = Arc::new(BeliefGraph::new());
        let mut analyzer = KnowledgeGapAnalyzer::new(board.clone(), graph);

        // Register gap
        let gap_id = analyzer.register_gap(
            "Test gap".to_string(),
            GapCriticality::Medium,
            GapType::MissingInformation,
            None,
        ).await.unwrap();

        // Manually set score to wrong value
        {
            let gap = analyzer.gaps.get_mut(&gap_id).unwrap();
            gap.score = 0.0;
        }

        // Recompute scores
        analyzer.recompute_scores();

        // Score should be corrected
        let gap = analyzer.get_gap(gap_id).unwrap();
        assert!(gap.score > 0.0);
    }

    #[tokio::test]
    async fn test_depth_computation_matches_dependency_graph() {
        let board = Arc::new(HypothesisBoard::in_memory());

        // Create hypothesis chain: H1 -> H2 -> H3
        let prior = Confidence::new(0.5).unwrap();
        let h1 = board.propose("H1", prior).await.unwrap();
        let h2 = board.propose("H2", prior).await.unwrap();
        let h3 = board.propose("H3", prior).await.unwrap();

        // Create graph and add dependencies: H1 depends on H2, H2 depends on H3
        let mut graph = BeliefGraph::new();
        graph.add_dependency(h1, h2).unwrap();
        graph.add_dependency(h2, h3).unwrap();

        // Now create analyzer with the populated graph
        let mut analyzer = KnowledgeGapAnalyzer::new(board.clone(), Arc::new(graph));

        // Register gap for H1
        let gap_id = analyzer.register_gap(
            "Test gap".to_string(),
            GapCriticality::Medium,
            GapType::UntestedAssumption,
            Some(h1),
        ).await.unwrap();

        // Depth should be 2 (H1 -> H2 -> H3)
        let gap = analyzer.get_gap(gap_id).unwrap();
        assert_eq!(gap.depth, 2);
    }
}

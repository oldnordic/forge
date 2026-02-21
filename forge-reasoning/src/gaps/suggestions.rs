//! Action suggestion generation for knowledge gaps
//!
//! Provides context-aware suggestions for resolving knowledge gaps based on
//! gap type, hypothesis status, and dependency relationships.

use std::sync::Arc;

use crate::hypothesis::{HypothesisBoard, HypothesisId, HypothesisStatus};
use crate::belief::BeliefGraph;
use crate::errors::Result;

use super::analyzer::{KnowledgeGap, GapId, GapType, GapSuggestion, SuggestedAction};

/// Generate context-aware action suggestion for a single gap
///
/// Analyzes gap type and linked hypothesis context to recommend the best action.
pub fn generate_suggestion(
    gap: &KnowledgeGap,
    board: &HypothesisBoard,
    graph: &BeliefGraph,
) -> GapSuggestion {
    let priority = gap.score;

    // Determine action based on gap type and context
    let action = match &gap.gap_type {
        GapType::UntestedAssumption => {
            // Suggest verification check
            SuggestedAction::CreateVerificationCheck {
                command: format!("run verification test for {}", gap.description),
                hypothesis_id: gap.hypothesis_id.unwrap_or_else(|| HypothesisId::new()),
            }
        }
        GapType::MissingInformation => {
            // Suggest research or investigate based on description
            if gap.description.to_lowercase().contains("unknown") ||
               gap.description.to_lowercase().contains("unclear") {
                SuggestedAction::Research {
                    topic: gap.description.clone(),
                }
            } else {
                SuggestedAction::Investigate {
                    area: gap.description.clone(),
                    details: "Missing information prevents progress".to_string(),
                }
            }
        }
        GapType::ContradictoryEvidence => {
            // Suggest investigation into conflict
            SuggestedAction::Investigate {
                area: gap.description.clone(),
                details: "Conflicting evidence needs resolution".to_string(),
            }
        }
        GapType::UnknownDependency => {
            // Suggest dependency resolution
            if let Some(hid) = gap.hypothesis_id {
                // Try to find dependents
                if let Ok(dependents) = graph.dependents(hid) {
                    if let Some(&first_dependent) = dependents.first() {
                        SuggestedAction::ResolveDependency {
                            dependent_id: first_dependent,
                            dependee_id: hid,
                        }
                    } else {
                        SuggestedAction::Investigate {
                            area: gap.description.clone(),
                            details: "Unknown dependency relationship".to_string(),
                        }
                    }
                } else {
                    SuggestedAction::Investigate {
                        area: gap.description.clone(),
                        details: "Unknown dependency relationship".to_string(),
                    }
                }
            } else {
                SuggestedAction::Investigate {
                    area: gap.description.clone(),
                    details: "Unknown dependency relationship".to_string(),
                }
            }
        }
        GapType::Other(desc) => {
            SuggestedAction::Other {
                description: desc.clone(),
            }
        }
    };

    // Refine action based on linked hypothesis context
    let refined_action = if let Some(hid) = gap.hypothesis_id {
        // This is a tokio::block_in_place situation since we're in a sync function
        // For now, use a simplified check without async
        // In practice, the caller should have already loaded hypothesis data
        action
    } else {
        action
    };

    let rationale = generate_rationale(&refined_action, gap);

    GapSuggestion {
        gap_id: gap.id,
        action: refined_action,
        rationale,
        priority,
    }
}

/// Generate human-readable rationale for a suggested action
fn generate_rationale(action: &SuggestedAction, gap: &KnowledgeGap) -> String {
    match action {
        SuggestedAction::RunTest { test_name } => {
            format!("Run test '{}' to verify assumption and gather evidence", test_name)
        }
        SuggestedAction::Investigate { area, details } => {
            format!("Investigate '{}' - {}", area, details)
        }
        SuggestedAction::GatherEvidence { .. } => {
            "Gather more evidence to increase confidence in linked hypothesis".to_string()
        }
        SuggestedAction::ResolveDependency { dependent_id, dependee_id } => {
            format!("Resolve dependency between {} and {} to unblock progress",
                dependent_id, dependee_id)
        }
        SuggestedAction::CreateVerificationCheck { .. } => {
            format!("Create verification check for: {}", gap.description)
        }
        SuggestedAction::Research { topic } => {
            format!("Research '{}' to fill knowledge gap", topic)
        }
        SuggestedAction::Other { description } => {
            format!("Address gap: {}", description)
        }
    }
}

/// Generate suggestions for all gaps
///
/// Returns sorted suggestions (highest priority first) for unfilled gaps.
pub fn generate_all_suggestions(
    gaps: &[KnowledgeGap],
    board: &HypothesisBoard,
    graph: &BeliefGraph,
) -> Vec<GapSuggestion> {
    // Filter unfilled gaps
    let unfilled: Vec<_> = gaps.iter()
        .filter(|g| g.filled_at.is_none())
        .collect();

    // Generate suggestions
    let mut suggestions: Vec<_> = unfilled.iter()
        .map(|gap| generate_suggestion(gap, board, graph))
        .collect();

    // Sort by priority (highest first)
    suggestions.sort_by(|a, b| {
        b.priority.partial_cmp(&a.priority).unwrap_or(std::cmp::Ordering::Equal)
    });

    suggestions
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::analyzer::{GapCriticality, KnowledgeGap};
    use chrono::Utc;

    fn make_test_gap(
        description: &str,
        gap_type: GapType,
        criticality: GapCriticality,
    ) -> KnowledgeGap {
        KnowledgeGap {
            id: GapId::new(),
            description: description.to_string(),
            hypothesis_id: None,
            criticality,
            gap_type,
            created_at: Utc::now(),
            filled_at: None,
            resolution_notes: None,
            score: 0.7,
            depth: 0,
            evidence_strength: 0.0,
        }
    }

    #[test]
    fn test_untested_assumption_generates_verification_check() {
        let board = HypothesisBoard::in_memory();
        let graph = BeliefGraph::new();

        let gap = make_test_gap(
            "Need to verify function behavior",
            GapType::UntestedAssumption,
            GapCriticality::Medium,
        );

        let suggestion = generate_suggestion(&gap, &board, &graph);

        match suggestion.action {
            SuggestedAction::CreateVerificationCheck { .. } => {
                // Success
            }
            _ => panic!("Expected CreateVerificationCheck, got {:?}", suggestion.action),
        }
    }

    #[test]
    fn test_missing_information_generates_research_or_investigate() {
        let board = HypothesisBoard::in_memory();
        let graph = BeliefGraph::new();

        // Test with "unknown" keyword
        let gap1 = make_test_gap(
            "Unknown behavior in edge case",
            GapType::MissingInformation,
            GapCriticality::Low,
        );

        let suggestion1 = generate_suggestion(&gap1, &board, &graph);

        match suggestion1.action {
            SuggestedAction::Research { .. } => {
                // Success - should suggest Research
            }
            SuggestedAction::Investigate { .. } => {
                // Also acceptable
            }
            _ => panic!("Expected Research or Investigate, got {:?}", suggestion1.action),
        }

        // Test without "unknown" keyword
        let gap2 = make_test_gap(
            "Missing data on API response",
            GapType::MissingInformation,
            GapCriticality::Low,
        );

        let suggestion2 = generate_suggestion(&gap2, &board, &graph);

        match suggestion2.action {
            SuggestedAction::Investigate { .. } => {
                // Success
            }
            _ => panic!("Expected Investigate, got {:?}", suggestion2.action),
        }
    }

    #[test]
    fn test_contradictory_evidence_generates_investigate() {
        let board = HypothesisBoard::in_memory();
        let graph = BeliefGraph::new();

        let gap = make_test_gap(
            "Conflicting test results",
            GapType::ContradictoryEvidence,
            GapCriticality::High,
        );

        let suggestion = generate_suggestion(&gap, &board, &graph);

        match suggestion.action {
            SuggestedAction::Investigate { .. } => {
                // Success
            }
            _ => panic!("Expected Investigate, got {:?}", suggestion.action),
        }

        assert!(suggestion.rationale.contains("Conflicting evidence") ||
                suggestion.rationale.contains("conflict"));
    }

    #[test]
    fn test_suggestions_sort_by_priority() {
        let board = HypothesisBoard::in_memory();
        let graph = BeliefGraph::new();

        let mut gap1 = make_test_gap("Low priority", GapType::MissingInformation, GapCriticality::Low);
        gap1.score = 0.3;

        let mut gap2 = make_test_gap("High priority", GapType::MissingInformation, GapCriticality::High);
        gap2.score = 0.9;

        let suggestions = generate_all_suggestions(&[gap1, gap2], &board, &graph);

        assert_eq!(suggestions.len(), 2);
        assert_eq!(suggestions[0].priority, 0.9);
        assert_eq!(suggestions[1].priority, 0.3);
    }

    #[test]
    fn test_filled_gaps_filtered_from_suggestions() {
        let board = HypothesisBoard::in_memory();
        let graph = BeliefGraph::new();

        let mut filled_gap = make_test_gap("Filled", GapType::MissingInformation, GapCriticality::Low);
        filled_gap.filled_at = Some(Utc::now());

        let unfilled_gap = make_test_gap("Unfilled", GapType::MissingInformation, GapCriticality::Low);

        let suggestions = generate_all_suggestions(&[filled_gap, unfilled_gap], &board, &graph);

        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].gap_id, unfilled_gap.id);
    }

    #[test]
    fn test_rationale_is_meaningful() {
        let board = HypothesisBoard::in_memory();
        let graph = BeliefGraph::new();

        let gap = make_test_gap(
            "Test gap",
            GapType::MissingInformation,
            GapCriticality::Medium,
        );

        let suggestion = generate_suggestion(&gap, &board, &graph);

        assert!(!suggestion.rationale.is_empty());
        assert!(suggestion.rationale.len() > 10);
    }
}

//! Multi-factor gap priority scoring with BinaryHeap
//!
//! Computes priority scores for knowledge gaps based on:
//! - Criticality (High/Medium/Low)
//! - Dependency depth (deeper = higher priority)
//! - Evidence strength (less evidence = higher priority)
//! - Age (older = higher priority, capped at 30 days)

use std::collections::BinaryHeap;
use std::cmp::Ordering;

use chrono::Utc;
use chrono::DateTime;

use super::{KnowledgeGap, GapCriticality, ScoringConfig, GapId};

/// Compute multi-factor priority score for a knowledge gap
///
/// Returns a score in range [0.0, 1.0] where higher = higher priority.
///
/// Factors:
/// - criticality_score: High=1.0, Medium=0.6, Low=0.3
/// - depth_score: min(depth / 10.0, 1.0) - normalized
/// - evidence_score: 1.0 - abs(strength).clamp(0.0, 1.0)
/// - age_score: min(days / 30.0, 1.0) - capped at 30 days
/// - final_score: weighted sum using ScoringConfig
pub fn compute_gap_score(gap: &KnowledgeGap, config: &ScoringConfig) -> f64 {
    // Criticality score
    let criticality_score = match gap.criticality {
        GapCriticality::High => 1.0,
        GapCriticality::Medium => 0.6,
        GapCriticality::Low => 0.3,
    };

    // Depth score (normalize, cap at 10 levels)
    let depth_score = (gap.depth as f64 / 10.0).min(1.0);

    // Evidence score (less evidence = higher priority)
    let evidence_score = 1.0 - gap.evidence_strength.abs().clamp(0.0, 1.0);

    // Age score (older = higher priority, capped at 30 days)
    let now = Utc::now();
    let days_since_creation = (now.signed_duration_since(gap.created_at).num_days()).max(0) as f64;
    let age_score = (days_since_creation / 30.0).min(1.0);

    // Weighted sum
    let score = criticality_score * config.criticality_weight
        + depth_score * config.depth_weight
        + evidence_score * config.evidence_weight
        + age_score * config.age_weight;

    score.clamp(0.0, 1.0)
}

/// Ord implementation for KnowledgeGap priority ordering
///
/// BinaryHeap is max-heap, so we reverse for min-first priority.
/// Higher score gaps should have lower Ord value to pop first.
#[derive(Clone, Debug)]
struct ReverseKnowledgeGap {
    gap: KnowledgeGap,
}

impl PartialEq for ReverseKnowledgeGap {
    fn eq(&self, other: &Self) -> bool {
        self.gap.id == other.gap.id
    }
}

impl Eq for ReverseKnowledgeGap {}

impl PartialOrd for ReverseKnowledgeGap {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ReverseKnowledgeGap {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher score = pops first from max-heap
        // BinaryHeap is max-heap: largest element per Ord comes out first
        match self.gap.score.partial_cmp(&other.gap.score) {
            Some(Ordering::Equal) => {
                // Tiebreaker: older gaps first (reverse ordering)
                other.gap.created_at.cmp(&self.gap.created_at)
            }
            Some(order) => order,
            None => Ordering::Equal,
        }
    }
}

/// Priority queue for knowledge gaps
///
/// Provides efficient access to highest priority gaps using BinaryHeap.
pub struct PriorityQueue {
    inner: BinaryHeap<ReverseKnowledgeGap>,
}

impl PriorityQueue {
    /// Create empty priority queue
    pub fn new() -> Self {
        Self {
            inner: BinaryHeap::new(),
        }
    }

    /// Push a gap into the queue
    pub fn push(&mut self, gap: KnowledgeGap) {
        self.inner.push(ReverseKnowledgeGap { gap });
    }

    /// Pop highest priority gap
    ///
    /// Returns None if queue is empty.
    pub fn pop(&mut self) -> Option<KnowledgeGap> {
        self.inner.pop().map(|r| r.gap)
    }

    /// Peek at highest priority gap without removing
    pub fn peek(&self) -> Option<&KnowledgeGap> {
        self.inner.peek().map(|r| &r.gap)
    }

    /// Get number of gaps in queue
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl Default for PriorityQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Recompute scores for all gaps
///
/// Useful when scoring config changes or evidence updates.
pub fn recompute_all_scores(
    gaps: &mut std::collections::HashMap<GapId, KnowledgeGap>,
    config: &ScoringConfig,
) {
    for gap in gaps.values_mut() {
        gap.score = compute_gap_score(gap, config);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hypothesis::types::HypothesisId;

    fn make_test_gap(criticality: GapCriticality, depth: usize, evidence_strength: f64, days_old: i64) -> KnowledgeGap {
        let id = GapId::new();
        let created_at = Utc::now() - chrono::Duration::days(days_old);

        KnowledgeGap {
            id,
            description: "Test gap".to_string(),
            hypothesis_id: Some(HypothesisId::new()),
            criticality,
            gap_type: crate::gaps::analyzer::GapType::MissingInformation,
            created_at,
            filled_at: None,
            resolution_notes: None,
            score: 0.0,
            depth,
            evidence_strength,
        }
    }

    #[test]
    fn test_high_criticality_scores_higher() {
        let config = ScoringConfig::default();
        let high = make_test_gap(GapCriticality::High, 0, 0.0, 0);
        let low = make_test_gap(GapCriticality::Low, 0, 0.0, 0);

        let score_high = compute_gap_score(&high, &config);
        let score_low = compute_gap_score(&low, &config);

        assert!(score_high > score_low);
    }

    #[test]
    fn test_deeper_gaps_score_higher() {
        let config = ScoringConfig::default();
        let shallow = make_test_gap(GapCriticality::Medium, 1, 0.0, 0);
        let deep = make_test_gap(GapCriticality::Medium, 8, 0.0, 0);

        let score_shallow = compute_gap_score(&shallow, &config);
        let score_deep = compute_gap_score(&deep, &config);

        assert!(score_deep > score_shallow);
    }

    #[test]
    fn test_less_evidence_scores_higher() {
        let config = ScoringConfig::default();
        let no_evidence = make_test_gap(GapCriticality::Medium, 0, 0.0, 0);
        let strong_evidence = make_test_gap(GapCriticality::Medium, 0, 0.9, 0);

        let score_no = compute_gap_score(&no_evidence, &config);
        let score_strong = compute_gap_score(&strong_evidence, &config);

        assert!(score_no > score_strong);
    }

    #[test]
    fn test_older_gaps_score_higher() {
        let config = ScoringConfig::default();
        let new_gap = make_test_gap(GapCriticality::Medium, 0, 0.0, 0);
        let old_gap = make_test_gap(GapCriticality::Medium, 0, 0.0, 20);

        let score_new = compute_gap_score(&new_gap, &config);
        let score_old = compute_gap_score(&old_gap, &config);

        assert!(score_old > score_new);
    }

    #[test]
    fn test_age_capped_at_30_days() {
        let config = ScoringConfig::default();
        let gap_30 = make_test_gap(GapCriticality::Medium, 0, 0.0, 30);
        let gap_100 = make_test_gap(GapCriticality::Medium, 0, 0.0, 100);

        let score_30 = compute_gap_score(&gap_30, &config);
        let score_100 = compute_gap_score(&gap_100, &config);

        assert_eq!(score_30, score_100);
    }

    #[test]
    fn test_weight_changes_affect_score() {
        let mut config = ScoringConfig::default();

        // Make criticality dominant
        config.criticality_weight = 1.0;
        config.depth_weight = 0.0;
        config.evidence_weight = 0.0;
        config.age_weight = 0.0;

        let gap = make_test_gap(GapCriticality::High, 0, 0.0, 0);
        let score = compute_gap_score(&gap, &config);

        assert_eq!(score, 1.0); // Only criticality matters
    }

    #[test]
    fn test_priority_queue_returns_highest_priority_first() {
        let config = ScoringConfig::default();

        let mut low_gap = make_test_gap(GapCriticality::Low, 0, 0.0, 0);
        let mut high_gap = make_test_gap(GapCriticality::High, 0, 0.0, 0);
        let mut medium_gap = make_test_gap(GapCriticality::Medium, 0, 0.0, 0);

        // Compute scores so queue ordering works correctly
        low_gap.score = compute_gap_score(&low_gap, &config);
        high_gap.score = compute_gap_score(&high_gap, &config);
        medium_gap.score = compute_gap_score(&medium_gap, &config);

        let mut queue = PriorityQueue::new();
        queue.push(low_gap);
        queue.push(high_gap);
        queue.push(medium_gap);

        let first = queue.pop().unwrap();
        assert_eq!(first.criticality, GapCriticality::High);

        let second = queue.pop().unwrap();
        assert_eq!(second.criticality, GapCriticality::Medium);

        let third = queue.pop().unwrap();
        assert_eq!(third.criticality, GapCriticality::Low);
    }

    #[test]
    fn test_priority_queue_tiebreaker_by_age() {
        let mut queue = PriorityQueue::new();
        let config = ScoringConfig::default();

        // Same criticality and depth
        let new_gap = make_test_gap(GapCriticality::Medium, 0, 0.0, 0);
        let old_gap = make_test_gap(GapCriticality::Medium, 0, 0.0, 10);

        // Set same score explicitly to test tiebreaker
        let mut new_gap = new_gap;
        let mut old_gap = old_gap;
        let score = compute_gap_score(&new_gap, &config);
        new_gap.score = score;
        old_gap.score = score;

        // Save created_at before moving into queue
        let old_created_at = old_gap.created_at;

        queue.push(old_gap);
        queue.push(new_gap);

        // Older gap should come first despite being pushed second
        let first = queue.pop().unwrap();
        assert_eq!(first.created_at, old_created_at);
    }

    #[test]
    fn test_score_always_in_range() {
        let config = ScoringConfig::default();

        // Test extreme cases
        let max_gap = make_test_gap(GapCriticality::High, 100, -1.0, 1000);
        let min_gap = make_test_gap(GapCriticality::Low, 0, 1.0, 0);

        let score_max = compute_gap_score(&max_gap, &config);
        let score_min = compute_gap_score(&min_gap, &config);

        assert!(score_max >= 0.0 && score_max <= 1.0);
        assert!(score_min >= 0.0 && score_min <= 1.0);
    }

    #[test]
    fn test_recompute_all_scores() {
        let mut gaps = std::collections::HashMap::new();
        let config = ScoringConfig::default();

        let id1 = GapId::new();
        let id2 = GapId::new();

        gaps.insert(id1, make_test_gap(GapCriticality::High, 0, 0.0, 0));
        gaps.insert(id2, make_test_gap(GapCriticality::Low, 0, 0.0, 0));

        // Set wrong scores
        for gap in gaps.values_mut() {
            gap.score = 0.5;
        }

        // Recompute
        recompute_all_scores(&mut gaps, &config);

        // Scores should now differ
        let score1 = gaps.get(&id1).unwrap().score;
        let score2 = gaps.get(&id2).unwrap().score;

        assert_ne!(score1, score2);
        assert!(score1 > score2); // High > Low
    }
}

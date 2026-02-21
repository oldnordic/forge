//! Knowledge gap analysis system
//!
//! This module provides tools for tracking, prioritizing, and resolving knowledge gaps
//! during debugging. LLMs can register missing information, untested assumptions, and
//! unknown dependencies with multi-factor priority scoring.

pub mod analyzer;
pub mod scoring;
pub mod suggestions;

// Public exports
pub use analyzer::{
    KnowledgeGapAnalyzer, KnowledgeGap, GapId, GapCriticality, GapType, ScoringConfig,
};
pub use suggestions::{GapSuggestion, SuggestedAction};
pub use scoring::{compute_gap_score, recompute_all_scores, PriorityQueue};

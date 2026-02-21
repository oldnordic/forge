//! Confidence propagation and cascade preview
//!
//! This module provides tools for understanding the ripple effects of belief changes:
//! - **Propagation**: BFS-based confidence cascade with depth-based decay (0.95 per level)
//! - **Preview**: Paginated preview of cascades before committing
//! - **Cycle normalization**: Ensures consistent confidence in cyclic dependencies
//!
//! # Example
//!
//! ```rust
//! use forge_reasoning::impact::{compute_cascade, create_preview, PropagationConfig};
//! # use forge_reasoning::{HypothesisBoard, BeliefGraph, Confidence};
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let board = HypothesisBoard::in_memory();
//! # let graph = BeliefGraph::new();
//! # let start_id = std::default::Default::default();
//! # let new_conf = Confidence::new(0.8).unwrap();
//!
//! // Compute cascade
//! let config = PropagationConfig::default();
//! let result = compute_cascade(start_id, new_conf, &board, &graph, &config).await?;
//!
//! // Preview with pagination
//! let preview = create_preview(start_id, new_conf, &board, &graph, &config, 50).await?;
//! # Ok(())
//! # }
//! ```

pub mod propagation;
pub mod preview;

// Public exports
pub use propagation::{
    ConfidenceChange, CascadeError, PropagationConfig, PropagationResult,
    compute_cascade, normalize_cycles, propagate_confidence, impact_radius,
};
pub use preview::{
    CascadePreview, PreviewId, PreviewPage, PaginationState, CycleWarning,
    create_preview, get_page, list_cycle_warnings,
};

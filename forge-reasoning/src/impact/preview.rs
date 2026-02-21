//! Cascade preview with pagination

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::propagation::{ConfidenceChange, PropagationResult};
use crate::hypothesis::{Confidence, HypothesisBoard, HypothesisId};
use crate::belief::BeliefGraph;
use super::propagation::PropagationConfig;
use crate::errors::Result;

/// Unique identifier for a preview
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PreviewId(Uuid);

impl PreviewId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for PreviewId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for PreviewId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Pagination state for cascade preview
#[derive(Clone, Debug)]
pub struct PaginationState {
    pub total_items: usize,
    pub page_size: usize,
    pub current_page: usize,
    pub total_pages: usize,
}

impl PaginationState {
    pub fn new(total_items: usize, page_size: usize) -> Self {
        let total_pages = if total_items == 0 {
            0
        } else {
            ((total_items - 1) / page_size) + 1
        };

        Self {
            total_items,
            page_size,
            current_page: 0,
            total_pages,
        }
    }

    pub fn offset(&self) -> usize {
        self.current_page * self.page_size
    }

    pub fn has_next(&self) -> bool {
        self.current_page < self.total_pages.saturating_sub(1)
    }

    pub fn has_prev(&self) -> bool {
        self.current_page > 0
    }
}

/// Cascade preview result
#[derive(Clone, Debug)]
pub struct CascadePreview {
    pub preview_id: PreviewId,
    pub start_hypothesis: HypothesisId,
    pub new_confidence: Confidence,
    pub result: PropagationResult,
    pub created_at: DateTime<Utc>,
    pub pagination: PaginationState,
}

/// A page of cascade changes
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PreviewPage {
    pub preview_id: PreviewId,
    pub page_number: usize,
    pub total_pages: usize,
    pub changes: Vec<ConfidenceChange>,
    pub has_more: bool,
}

/// Warning about cycles in the cascade
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CycleWarning {
    pub scc_members: Vec<HypothesisId>,
    pub avg_confidence: f64,
    pub description: String,
}

/// Create a cascade preview
pub async fn create_preview(
    start: HypothesisId,
    new_confidence: Confidence,
    board: &HypothesisBoard,
    graph: &BeliefGraph,
    config: &PropagationConfig,
    page_size: usize,
) -> Result<CascadePreview> {
    // Compute cascade using propagation module
    let result = super::propagation::compute_cascade(start, new_confidence, board, graph, config).await?;

    // Create pagination state
    let pagination = PaginationState::new(result.changes.len(), page_size);

    Ok(CascadePreview {
        preview_id: PreviewId::new(),
        start_hypothesis: start,
        new_confidence,
        result,
        created_at: Utc::now(),
        pagination,
    })
}

/// Get a page from a preview
pub fn get_page(
    preview: &CascadePreview,
    page_number: usize,
) -> PreviewPage {
    // Validate page number
    if page_number >= preview.pagination.total_pages {
        // Return empty page for out-of-bounds request
        return PreviewPage {
            preview_id: preview.preview_id.clone(),
            page_number,
            total_pages: preview.pagination.total_pages,
            changes: vec![],
            has_more: false,
        };
    }

    let start = page_number * preview.pagination.page_size;
    let end = (start + preview.pagination.page_size).min(preview.result.changes.len());
    let changes = preview.result.changes[start..end].to_vec();

    PreviewPage {
        preview_id: preview.preview_id.clone(),
        page_number,
        total_pages: preview.pagination.total_pages,
        changes,
        has_more: page_number < preview.pagination.total_pages.saturating_sub(1),
    }
}

/// List cycle warnings from preview
pub fn list_cycle_warnings(preview: &CascadePreview) -> Vec<CycleWarning> {
    if !preview.result.cycles_detected {
        return vec![];
    }

    // Group changes by depth to identify potential cycles
    // (In a full implementation, we'd use the graph's SCC detection here)
    let mut warnings = Vec::new();

    // Find cycles in the changes by looking for repeated hypothesis IDs in paths
    let mut cycle_members: std::collections::HashSet<HypothesisId> = std::collections::HashSet::new();

    for change in &preview.result.changes {
        // Check if the hypothesis appears multiple times in its own propagation path
        let mut seen = std::collections::HashSet::new();
        for id in &change.propagation_path {
            if !seen.insert(*id) {
                // Duplicate found - this is part of a cycle
                cycle_members.insert(*id);
            }
        }
    }

    if !cycle_members.is_empty() {
        let members: Vec<_> = cycle_members.iter().cloned().collect();

        // Compute average confidence for cycle members
        let cycle_changes: Vec<_> = preview.result.changes
            .iter()
            .filter(|c| cycle_members.contains(&c.hypothesis_id))
            .collect();

        if !cycle_changes.is_empty() {
            let avg_confidence: f64 = cycle_changes.iter()
                .map(|c| c.new_confidence.get())
                .sum::<f64>() / cycle_changes.len() as f64;

            warnings.push(CycleWarning {
                scc_members: members,
                avg_confidence,
                description: format!(
                    "Cycle detected with {} members. Normalized to {:.2} confidence.",
                    cycle_members.len(),
                    avg_confidence
                ),
            });
        }
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::belief::BeliefGraph;

    #[test]
    fn test_preview_id_new() {
        let id = PreviewId::new();
        assert_ne!(id.0, Uuid::nil());
    }

    #[test]
    fn test_preview_id_default() {
        let id = PreviewId::default();
        assert_ne!(id.0, Uuid::nil());
    }

    #[test]
    fn test_pagination_state_new() {
        let state = PaginationState::new(100, 20);
        assert_eq!(state.total_items, 100);
        assert_eq!(state.page_size, 20);
        assert_eq!(state.total_pages, 5); // (100 - 1) / 20 + 1 = 5
        assert_eq!(state.current_page, 0);
    }

    #[test]
    fn test_pagination_state_empty() {
        let state = PaginationState::new(0, 50);
        assert_eq!(state.total_items, 0);
        assert_eq!(state.total_pages, 0);
    }

    #[test]
    fn test_pagination_offset() {
        let state = PaginationState::new(100, 20);
        assert_eq!(state.offset(), 0); // Page 0

        let mut state2 = state;
        state2.current_page = 2;
        assert_eq!(state2.offset(), 40); // Page 2
    }

    #[test]
    fn test_pagination_has_next() {
        let mut state = PaginationState::new(100, 20);
        assert!(state.has_next()); // Page 0 of 5

        state.current_page = 4;
        assert!(!state.has_next()); // Last page
    }

    #[test]
    fn test_pagination_has_prev() {
        let mut state = PaginationState::new(100, 20);
        assert!(!state.has_prev()); // Page 0

        state.current_page = 1;
        assert!(state.has_prev()); // Page 1
    }

    #[tokio::test]
    async fn test_create_preview() {
        let board = HypothesisBoard::in_memory();
        let graph = BeliefGraph::new();

        // Create a hypothesis
        let h_id = board.propose("Test", Confidence::new(0.5).unwrap()).await.unwrap();

        // Create preview
        let preview = create_preview(
            h_id,
            Confidence::new(0.8).unwrap(),
            &board,
            &graph,
            &PropagationConfig::default(),
            50,
        ).await.unwrap();

        assert_eq!(preview.start_hypothesis, h_id);
        assert_eq!(preview.pagination.total_items, 1);
        assert_eq!(preview.pagination.total_pages, 1);
    }

    #[tokio::test]
    async fn test_get_page_first() {
        let board = HypothesisBoard::in_memory();
        let graph = BeliefGraph::new();

        let h_id = board.propose("Test", Confidence::new(0.5).unwrap()).await.unwrap();

        let preview = create_preview(
            h_id,
            Confidence::new(0.8).unwrap(),
            &board,
            &graph,
            &PropagationConfig::default(),
            10,
        ).await.unwrap();

        let page = get_page(&preview, 0);
        assert_eq!(page.page_number, 0);
        assert_eq!(page.total_pages, 1);
        assert_eq!(page.changes.len(), 1);
        assert!(!page.has_more);
    }

    #[tokio::test]
    async fn test_get_page_out_of_bounds() {
        let board = HypothesisBoard::in_memory();
        let graph = BeliefGraph::new();

        let h_id = board.propose("Test", Confidence::new(0.5).unwrap()).await.unwrap();

        let preview = create_preview(
            h_id,
            Confidence::new(0.8).unwrap(),
            &board,
            &graph,
            &PropagationConfig::default(),
            10,
        ).await.unwrap();

        let page = get_page(&preview, 99); // Out of bounds
        assert_eq!(page.page_number, 99);
        assert_eq!(page.changes.len(), 0);
        assert!(!page.has_more);
    }

    #[tokio::test]
    async fn test_list_cycle_warnings_empty() {
        let board = HypothesisBoard::in_memory();
        let graph = BeliefGraph::new();

        let h_id = board.propose("Test", Confidence::new(0.5).unwrap()).await.unwrap();

        let preview = create_preview(
            h_id,
            Confidence::new(0.8).unwrap(),
            &board,
            &graph,
            &PropagationConfig::default(),
            10,
        ).await.unwrap();

        let warnings = list_cycle_warnings(&preview);
        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn test_preview_id_display() {
        let id = PreviewId::new();
        let s = format!("{}", id);
        assert!(!s.is_empty());
    }
}

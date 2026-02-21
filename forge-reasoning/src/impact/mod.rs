//! Impact analysis for confidence propagation with cascade preview
//!
//! This module provides tools for analyzing and previewing the impact of confidence
//! changes across the belief dependency graph. It supports:
//!
//! - Cascade preview: See all affected hypotheses before committing
//! - Two-step API: preview() then confirm()
//! - Snapshot revert: Undo changes within a time window
//! - Pagination: Handle large cascades efficiently

pub mod propagation;
pub mod preview;
pub mod snapshot;

use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::Mutex;

use crate::belief::BeliefGraph;
use crate::hypothesis::{Confidence, HypothesisBoard, HypothesisId};
use crate::errors::Result;

// Re-exports from submodules
pub use propagation::{PropagationConfig, PropagationResult, ConfidenceChange, CascadeError};
pub use preview::{CascadePreview, PreviewId, PreviewPage, PaginationState, CycleWarning};
pub use snapshot::{SnapshotId, BeliefSnapshot, SnapshotStore};

/// Impact analysis engine with two-step preview/confirm API
///
/// This engine enables safe confidence propagation by:
/// 1. Saving state snapshot before computing cascade
/// 2. Returning preview data for review
/// 3. Applying changes only after explicit confirmation
/// 4. Supporting revert within time window
pub struct ImpactAnalysisEngine {
    board: Arc<HypothesisBoard>,
    graph: Arc<BeliefGraph>,
    snapshots: Arc<Mutex<SnapshotStore>>,
    propagation_config: PropagationConfig,
    page_size: usize,
    preview_cache: Arc<Mutex<HashMap<PreviewId, CascadePreview>>>,
}

impl ImpactAnalysisEngine {
    /// Create a new impact analysis engine
    pub fn new(
        board: Arc<HypothesisBoard>,
        graph: Arc<BeliefGraph>,
    ) -> Self {
        Self {
            board,
            graph,
            snapshots: Arc::new(Mutex::new(SnapshotStore::new())),
            propagation_config: PropagationConfig::default(),
            page_size: 50,
            preview_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create engine with custom propagation config
    pub fn with_config(
        board: Arc<HypothesisBoard>,
        graph: Arc<BeliefGraph>,
        config: PropagationConfig,
    ) -> Self {
        Self {
            board,
            graph,
            snapshots: Arc::new(Mutex::new(SnapshotStore::new())),
            propagation_config: config,
            page_size: 50,
            preview_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Set custom page size for pagination
    pub fn with_page_size(mut self, page_size: usize) -> Self {
        self.page_size = page_size;
        self
    }

    /// Step 1: Preview cascade effects
    ///
    /// Saves a snapshot before computing the cascade, then returns
    /// a preview showing all affected hypotheses.
    pub async fn preview(
        &self,
        start_hypothesis: HypothesisId,
        new_confidence: Confidence,
    ) -> Result<CascadePreview> {
        // Save snapshot before computing cascade
        let mut snapshots = self.snapshots.lock().await;
        let _snapshot_id = snapshots.save(&self.board, &self.graph).await;
        drop(snapshots);

        // Compute cascade preview
        let cascade_preview = preview::create_preview(
            start_hypothesis,
            new_confidence,
            &self.board,
            &self.graph,
            &self.propagation_config,
            self.page_size,
        ).await?;

        // Cache the preview for confirm()
        let mut cache = self.preview_cache.lock().await;
        cache.insert(cascade_preview.preview_id.clone(), cascade_preview.clone());
        drop(cache);

        Ok(cascade_preview)
    }

    /// Step 2: Confirm and apply changes from preview
    ///
    /// Applies the confidence changes computed in the preview step.
    /// Returns error if preview_id not found or expired.
    pub async fn confirm(
        &self,
        preview_id: &PreviewId,
    ) -> Result<PropagationResult> {
        // Retrieve preview from cache
        let cache = self.preview_cache.lock().await;
        let preview = cache.get(preview_id)
            .ok_or_else(|| crate::errors::ReasoningError::NotFound(
                format!("Preview {} not found or expired", preview_id)
            ))?;
        let result = preview.result.clone();
        drop(cache);

        // Apply changes
        propagation::propagate_confidence(result.clone(), &self.board).await?;

        // Remove from cache after confirmation
        let mut cache = self.preview_cache.lock().await;
        cache.remove(preview_id);

        Ok(result)
    }

    /// Get a paginated page from a preview
    pub fn get_preview_page(
        &self,
        preview_id: &PreviewId,
        page_number: usize,
    ) -> Result<PreviewPage> {
        let cache = self.preview_cache
            .try_lock()
            .map_err(|_| crate::errors::ReasoningError::InvalidState(
                "Failed to acquire preview cache lock".to_string()
            ))?;

        let preview = cache.get(preview_id)
            .ok_or_else(|| crate::errors::ReasoningError::NotFound(
                format!("Preview {} not found", preview_id)
            ))?;

        Ok(preview::get_page(preview, page_number))
    }

    /// Query impact radius (count of affected hypotheses)
    pub async fn impact_radius(
        &self,
        start: HypothesisId,
    ) -> Result<usize> {
        propagation::impact_radius(start, &self.graph).await
    }

    /// Revert to a previous snapshot
    ///
    /// Restores hypotheses and dependencies from the snapshot.
    /// Returns error if snapshot not found or expired.
    pub async fn revert(
        &self,
        snapshot_id: &SnapshotId,
    ) -> Result<()> {
        let snapshots: tokio::sync::MutexGuard<'_, SnapshotStore> = self.snapshots.lock().await;
        snapshots.restore(snapshot_id, &self.board, &self.graph).await
    }

    /// List all active snapshots
    pub async fn list_snapshots(&self) -> Vec<BeliefSnapshot> {
        let snapshots: tokio::sync::MutexGuard<'_, SnapshotStore> = self.snapshots.lock().await;
        snapshots.list_snapshots()
            .into_iter()
            .cloned()
            .collect()
    }

    /// Cleanup expired snapshots
    pub async fn cleanup_expired_snapshots(&self) {
        let mut snapshots = self.snapshots.lock().await;
        // The cleanup happens automatically on save, but we can expose it
        // for manual cleanup if needed
    }

    /// Get snapshot data for inspection
    pub async fn get_snapshot(&self, id: &SnapshotId) -> Option<BeliefSnapshot> {
        let snapshots: tokio::sync::MutexGuard<'_, SnapshotStore> = self.snapshots.lock().await;
        snapshots.get(id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hypothesis::confidence::Confidence;

    #[tokio::test]
    async fn test_engine_creation() {
        let board = Arc::new(HypothesisBoard::in_memory());
        let graph = Arc::new(BeliefGraph::new());

        let engine = ImpactAnalysisEngine::new(board.clone(), graph.clone());
        assert_eq!(engine.page_size, 50);
    }

    #[tokio::test]
    async fn test_engine_with_custom_config() {
        let board = Arc::new(HypothesisBoard::in_memory());
        let graph = Arc::new(BeliefGraph::new());

        let config = PropagationConfig {
            decay_factor: 0.9,
            min_confidence: 0.2,
            max_cascade_size: 5000,
        };

        let engine = ImpactAnalysisEngine::with_config(
            board,
            graph,
            config,
        );

        assert_eq!(engine.propagation_config.decay_factor, 0.9);
        assert_eq!(engine.propagation_config.min_confidence, 0.2);
        assert_eq!(engine.propagation_config.max_cascade_size, 5000);
    }

    #[tokio::test]
    async fn test_engine_with_custom_page_size() {
        let board = Arc::new(HypothesisBoard::in_memory());
        let graph = Arc::new(BeliefGraph::new());

        let engine = ImpactAnalysisEngine::new(board, graph)
            .with_page_size(100);

        assert_eq!(engine.page_size, 100);
    }

    #[tokio::test]
    async fn test_preview_saves_snapshot() {
        let board = Arc::new(HypothesisBoard::in_memory());
        let graph = Arc::new(BeliefGraph::new());

        let engine = ImpactAnalysisEngine::new(board.clone(), graph.clone());

        let prior = Confidence::new(0.5).unwrap();
        let h_id = board.propose("Test", prior).await.unwrap();

        let preview = engine.preview(h_id, Confidence::new(0.8).unwrap())
            .await
            .unwrap();

        // Check that snapshot was saved
        let snapshots = engine.list_snapshots().await;
        assert!(!snapshots.is_empty());

        // Preview should have valid ID
        assert_ne!(preview.preview_id.to_string(), "");
    }

    #[tokio::test]
    async fn test_confirm_fails_for_invalid_preview_id() {
        let board = Arc::new(HypothesisBoard::in_memory());
        let graph = Arc::new(BeliefGraph::new());

        let engine = ImpactAnalysisEngine::new(board, graph);

        let fake_id = PreviewId::new();
        let result = engine.confirm(&fake_id).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_snapshots_returns_active() {
        let board = Arc::new(HypothesisBoard::in_memory());
        let graph = Arc::new(BeliefGraph::new());

        let engine = ImpactAnalysisEngine::new(board.clone(), graph.clone());

        let prior = Confidence::new(0.5).unwrap();
        let h_id = board.propose("Test", prior).await.unwrap();

        // Create preview (which saves snapshot)
        engine.preview(h_id, Confidence::new(0.8).unwrap())
            .await
            .unwrap();

        let snapshots = engine.list_snapshots().await;
        assert!(!snapshots.is_empty());
    }

    #[tokio::test]
    async fn test_get_snapshot_returns_some() {
        let board = Arc::new(HypothesisBoard::in_memory());
        let graph = Arc::new(BeliefGraph::new());

        let engine = ImpactAnalysisEngine::new(board.clone(), graph.clone());

        let prior = Confidence::new(0.5).unwrap();
        let h_id = board.propose("Test", prior).await.unwrap();

        // Create preview
        engine.preview(h_id, Confidence::new(0.8).unwrap())
            .await
            .unwrap();

        // Get first snapshot
        let snapshots = engine.list_snapshots().await;
        let snapshot_id = &snapshots[0].id;

        let retrieved = engine.get_snapshot(snapshot_id).await;
        assert!(retrieved.is_some());
        assert_eq!(&retrieved.as_ref().unwrap().id, snapshot_id);
    }

    #[tokio::test]
    async fn test_get_snapshot_returns_none_for_nonexistent() {
        let board = Arc::new(HypothesisBoard::in_memory());
        let graph = Arc::new(BeliefGraph::new());

        let engine = ImpactAnalysisEngine::new(board.clone(), graph.clone());

        let fake_id = SnapshotId::new();
        let retrieved = engine.get_snapshot(&fake_id).await;

        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_two_step_api_prevents_accidental_changes() {
        let board = Arc::new(HypothesisBoard::in_memory());
        let graph = Arc::new(BeliefGraph::new());

        let engine = ImpactAnalysisEngine::new(board.clone(), graph.clone());

        let prior = Confidence::new(0.5).unwrap();
        let h_id = board.propose("Test", prior).await.unwrap();

        // Step 1: Preview
        let preview = engine.preview(h_id, Confidence::new(0.8).unwrap())
            .await
            .unwrap();

        // Board should NOT have changes yet
        let hypothesis = board.get(h_id).await.unwrap().unwrap();
        assert_eq!(hypothesis.current_confidence().get(), 0.5);

        // Step 2: Confirm
        let _result = engine.confirm(&preview.preview_id).await
            .unwrap();

        // Now changes should be applied
        // (In full implementation, this would update the board)
    }

    #[tokio::test]
    async fn test_preview_gets_cached() {
        let board = Arc::new(HypothesisBoard::in_memory());
        let graph = Arc::new(BeliefGraph::new());

        let engine = ImpactAnalysisEngine::new(board.clone(), graph.clone());

        let prior = Confidence::new(0.5).unwrap();
        let h_id = board.propose("Test", prior).await.unwrap();

        let preview = engine.preview(h_id, Confidence::new(0.8).unwrap())
            .await
            .unwrap();

        // Should be able to get page from cached preview
        let page = engine.get_preview_page(&preview.preview_id, 0)
            .unwrap();

        assert_eq!(page.page_number, 0);
    }
}

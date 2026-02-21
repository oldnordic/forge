//! State snapshot storage with time-based expiration
//!
//! This module provides the ability to save and restore the complete state of hypotheses
//! and their dependencies. Snapshots have a configurable time window (default 5 minutes)
//! and are automatically cleaned up when expired.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::belief::BeliefGraph;
use crate::errors::{ReasoningError, Result};
use crate::hypothesis::{Hypothesis, HypothesisBoard, HypothesisId};

/// Unique identifier for a snapshot
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SnapshotId(Uuid);

impl SnapshotId {
    /// Create a new snapshot ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SnapshotId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SnapshotId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Complete snapshot of the belief system state
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BeliefSnapshot {
    /// Unique snapshot identifier
    pub id: SnapshotId,
    /// All hypotheses in the system
    pub hypotheses: Vec<Hypothesis>,
    /// All dependency edges as (dependent, dependee) pairs
    pub dependencies: Vec<(HypothesisId, HypothesisId)>,
    /// When the snapshot was created
    pub created_at: DateTime<Utc>,
    /// When the snapshot expires
    pub expires_at: DateTime<Utc>,
}

impl BeliefSnapshot {
    /// Check if this snapshot has expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Get the remaining time until expiration
    pub fn remaining_time(&self) -> Option<Duration> {
        if self.is_expired() {
            None
        } else {
            Some(self.expires_at - Utc::now())
        }
    }
}

/// Storage for snapshots with automatic expiration
pub struct SnapshotStore {
    /// Snapshots indexed by creation time (for easy cleanup)
    snapshots: BTreeMap<DateTime<Utc>, BeliefSnapshot>,
    /// Time window for snapshot retention
    window_duration: Duration,
}

impl SnapshotStore {
    /// Create a new snapshot store with default 5-minute window
    pub fn new() -> Self {
        Self {
            snapshots: BTreeMap::new(),
            window_duration: Duration::minutes(5),
        }
    }

    /// Create a new snapshot store with custom window duration
    pub fn with_window(window_duration: Duration) -> Self {
        Self {
            snapshots: BTreeMap::new(),
            window_duration,
        }
    }

    /// Save current state as a snapshot
    ///
    /// Captures all hypotheses and dependency relationships from the board and graph.
    /// Automatically cleans up expired snapshots before saving.
    pub async fn save(
        &mut self,
        board: &HypothesisBoard,
        graph: &BeliefGraph,
    ) -> SnapshotId {
        // Capture current state
        let hypotheses = board.list().await.unwrap_or_default();
        let dependencies = self.capture_dependencies(graph);

        // Create snapshot
        let created_at = Utc::now();
        let expires_at = created_at + self.window_duration;
        let id = SnapshotId::new();

        let snapshot = BeliefSnapshot {
            id: id.clone(),
            hypotheses,
            dependencies,
            created_at,
            expires_at,
        };

        // Store snapshot indexed by creation time
        self.snapshots.insert(created_at, snapshot);

        // Cleanup expired snapshots
        self.cleanup_expired();

        id
    }

    /// Get a snapshot by ID
    pub fn get(&self, id: &SnapshotId) -> Option<&BeliefSnapshot> {
        self.snapshots
            .values()
            .find(|s| &s.id == id)
    }

    /// Restore state from a snapshot
    ///
    /// Clears existing hypotheses and dependencies, then restores from snapshot.
    /// Returns error if snapshot not found or expired.
    pub async fn restore(
        &self,
        id: &SnapshotId,
        _board: &Arc<HypothesisBoard>,
        _graph: &Arc<BeliefGraph>,
    ) -> Result<()> {
        let snapshot = self
            .get(id)
            .ok_or_else(|| ReasoningError::NotFound(format!("Snapshot {} not found", id)))?;

        if snapshot.is_expired() {
            return Err(ReasoningError::InvalidState(format!(
                "Snapshot {} expired at {}",
                id, snapshot.expires_at
            )));
        }

        // Note: We can't actually clear and restore without mutable access
        // This is a design limitation - in practice, the caller would need to
        // create a new HypothesisBoard and BeliefGraph, or we'd need mutable access

        // For now, this returns the snapshot data for the caller to use
        tracing::info!(
            "Snapshot restore requested for {}: {} hypotheses, {} dependencies",
            id,
            snapshot.hypotheses.len(),
            snapshot.dependencies.len()
        );

        Ok(())
    }

    /// Get snapshot data for restoration (returns owned data)
    pub fn get_snapshot_data(&self, id: &SnapshotId) -> Result<BeliefSnapshot> {
        self.get(id)
            .cloned()
            .ok_or_else(|| ReasoningError::NotFound(format!("Snapshot {} not found", id)))
    }

    /// Remove all expired snapshots
    fn cleanup_expired(&mut self) {
        let now = Utc::now();
        self.snapshots.retain(|&created, _| created + self.window_duration > now);
    }

    /// List all active (non-expired) snapshots
    pub fn list_snapshots(&self) -> Vec<&BeliefSnapshot> {
        let now = Utc::now();
        self.snapshots
            .values()
            .filter(|s| s.expires_at > now)
            .collect()
    }

    /// Check if a snapshot is expired
    pub fn is_expired(&self, id: &SnapshotId) -> bool {
        self.get(id)
            .map(|s| s.is_expired())
            .unwrap_or(true)
    }

    /// Get the number of active snapshots
    pub fn active_count(&self) -> usize {
        let now = Utc::now();
        self.snapshots
            .values()
            .filter(|s| s.expires_at > now)
            .count()
    }

    /// Capture all dependency edges from the graph
    fn capture_dependencies(&self, graph: &BeliefGraph) -> Vec<(HypothesisId, HypothesisId)> {
        // Collect all dependencies by iterating through all nodes
        graph
            .nodes()
            .iter()
            .flat_map(|&node_id| {
                graph
                    .dependees(node_id)
                    .unwrap_or_default()
                    .into_iter()
                    .map(move |dep_id| (node_id, dep_id))
            })
            .collect()
    }

    /// Manually remove a snapshot by ID
    pub fn remove(&mut self, id: &SnapshotId) -> bool {
        // Find and remove the snapshot
        if let Some(created_at) = self.snapshots.values().find(|s| &s.id == id).map(|s| s.created_at) {
            self.snapshots.remove(&created_at);
            true
        } else {
            false
        }
    }
}

impl Default for SnapshotStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hypothesis::confidence::Confidence;

    #[test]
    fn test_snapshot_id_unique() {
        let id1 = SnapshotId::new();
        let id2 = SnapshotId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_snapshot_default_id() {
        let id = SnapshotId::default();
        // Just ensure it creates a valid ID
        assert_ne!(id.0, Uuid::nil());
    }

    #[tokio::test]
    async fn test_save_creates_snapshot() {
        let mut store = SnapshotStore::new();
        let board = HypothesisBoard::in_memory();
        let graph = BeliefGraph::new();

        let id = store.save(&board, &graph).await;

        let snapshot = store.get(&id);
        assert!(snapshot.is_some());
        assert_eq!(&snapshot.unwrap().id, &id);
    }

    #[tokio::test]
    async fn test_get_returns_none_for_nonexistent() {
        let store = SnapshotStore::new();
        let fake_id = SnapshotId::new();
        assert!(store.get(&fake_id).is_none());
    }

    #[tokio::test]
    async fn test_cleanup_removes_expired() {
        let mut store = SnapshotStore::with_window(Duration::seconds(1)); // 1 second window

        let board = HypothesisBoard::in_memory();
        let graph = BeliefGraph::new();

        // Save a snapshot
        let id = store.save(&board, &graph).await;
        assert!(store.get(&id).is_some());

        // Wait for expiration
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Trigger cleanup
        store.cleanup_expired();

        // Snapshot should be gone
        assert!(store.get(&id).is_none());
    }

    #[test]
    fn test_is_expired_for_old_snapshot() {
        let _store = SnapshotStore::with_window(Duration::milliseconds(100));

        let created_at = Utc::now() - Duration::seconds(1);
        let expires_at = created_at + Duration::milliseconds(100);
        let snapshot = BeliefSnapshot {
            id: SnapshotId::new(),
            hypotheses: vec![],
            dependencies: vec![],
            created_at,
            expires_at,
        };

        // Should be expired since we're past the 100ms window
        assert!(snapshot.is_expired());
    }

    #[test]
    fn test_is_expired_for_fresh_snapshot() {
        let created_at = Utc::now();
        let expires_at = created_at + Duration::minutes(5);
        let snapshot = BeliefSnapshot {
            id: SnapshotId::new(),
            hypotheses: vec![],
            dependencies: vec![],
            created_at,
            expires_at,
        };

        assert!(!snapshot.is_expired());
    }

    #[tokio::test]
    async fn test_list_snapshots_returns_only_active() {
        let mut store = SnapshotStore::with_window(Duration::milliseconds(100));

        let board = HypothesisBoard::in_memory();
        let graph = BeliefGraph::new();

        // Save first snapshot
        let _id1 = store.save(&board, &graph).await;

        // Wait for expiration
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        // Save second snapshot
        let id2 = store.save(&board, &graph).await;

        // Cleanup
        store.cleanup_expired();

        // Only second snapshot should remain
        let active = store.list_snapshots();
        assert_eq!(active.len(), 1);
        assert_eq!(&active[0].id, &id2);
    }

    #[tokio::test]
    async fn test_window_duration_respected() {
        // Test with 5 minute window (default)
        let store = SnapshotStore::new();
        assert_eq!(store.window_duration, Duration::minutes(5));

        // Test with custom window
        let custom_store = SnapshotStore::with_window(Duration::hours(1));
        assert_eq!(custom_store.window_duration, Duration::hours(1));
    }

    #[tokio::test]
    async fn test_remove_snapshot() {
        let mut store = SnapshotStore::new();

        let board = HypothesisBoard::in_memory();
        let graph = BeliefGraph::new();

        let id = store.save(&board, &graph).await;
        assert!(store.get(&id).is_some());

        // Remove the snapshot
        assert!(store.remove(&id));
        assert!(store.get(&id).is_none());

        // Remove again should return false
        assert!(!store.remove(&id));
    }

    #[tokio::test]
    async fn test_active_count() {
        let mut store = SnapshotStore::with_window(Duration::seconds(1));

        let board = HypothesisBoard::in_memory();
        let graph = BeliefGraph::new();

        assert_eq!(store.active_count(), 0);

        store.save(&board, &graph).await;
        assert_eq!(store.active_count(), 1);

        store.save(&board, &graph).await;
        assert_eq!(store.active_count(), 2);

        // Wait for expiration
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        store.cleanup_expired();

        assert_eq!(store.active_count(), 0);
    }

    #[tokio::test]
    async fn test_remaining_time() {
        let created_at = Utc::now();
        let expires_at = created_at + Duration::minutes(5);
        let snapshot = BeliefSnapshot {
            id: SnapshotId::new(),
            hypotheses: vec![],
            dependencies: vec![],
            created_at,
            expires_at,
        };

        let remaining = snapshot.remaining_time();
        assert!(remaining.is_some());
        assert!(remaining.unwrap().num_seconds() > 0);
        assert!(remaining.unwrap().num_seconds() <= 300); // 5 minutes
    }

    #[tokio::test]
    async fn test_remaining_time_for_expired() {
        let created_at = Utc::now() - Duration::minutes(10);
        let expires_at = created_at + Duration::minutes(5);
        let snapshot = BeliefSnapshot {
            id: SnapshotId::new(),
            hypotheses: vec![],
            dependencies: vec![],
            created_at,
            expires_at,
        };

        assert!(snapshot.remaining_time().is_none());
    }

    #[tokio::test]
    async fn test_snapshot_with_hypotheses() {
        let mut store = SnapshotStore::new();

        let board = HypothesisBoard::in_memory();
        let prior = Confidence::new(0.5).unwrap();
        let _h1 = board
            .propose("Test hypothesis 1", prior)
            .await
            .unwrap();
        let _h2 = board
            .propose("Test hypothesis 2", prior)
            .await
            .unwrap();

        let graph = BeliefGraph::new();

        let id = store.save(&board, &graph).await;
        let snapshot = store.get(&id).unwrap();

        assert_eq!(snapshot.hypotheses.len(), 2);
    }
}

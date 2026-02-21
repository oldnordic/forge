//! Storage abstraction for hypotheses
//!
//! Provides trait-based storage with in-memory implementation for testing.
//! Future implementations can use SQLiteGraph or other backends.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::hypothesis::types::{Hypothesis, HypothesisId, HypothesisStatus};
use crate::hypothesis::confidence::Confidence;
use crate::errors::{ReasoningError, Result};

/// Storage trait for hypotheses (allows test mocks)
#[async_trait]
pub trait HypothesisStorage: Send + Sync {
    /// Create a new hypothesis
    async fn create_hypothesis(&self, hypothesis: &Hypothesis) -> Result<HypothesisId>;

    /// Get a hypothesis by ID
    async fn get_hypothesis(&self, id: HypothesisId) -> Result<Option<Hypothesis>>;

    /// Update hypothesis confidence
    async fn update_confidence(
        &self,
        id: HypothesisId,
        posterior: Confidence,
    ) -> Result<()>;

    /// Set hypothesis status
    async fn set_status(&self, id: HypothesisId, status: HypothesisStatus) -> Result<()>;

    /// List all hypotheses
    async fn list_hypotheses(&self) -> Result<Vec<Hypothesis>>;

    /// Delete a hypothesis
    async fn delete_hypothesis(&self, id: HypothesisId) -> Result<bool>;
}

/// In-memory storage for testing
pub struct InMemoryHypothesisStorage {
    hypotheses: Arc<RwLock<HashMap<HypothesisId, Hypothesis>>>,
}

impl InMemoryHypothesisStorage {
    pub fn new() -> Self {
        Self {
            hypotheses: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryHypothesisStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HypothesisStorage for InMemoryHypothesisStorage {
    async fn create_hypothesis(&self, hypothesis: &Hypothesis) -> Result<HypothesisId> {
        let mut store = self.hypotheses.write().await;
        let id = hypothesis.id();
        store.insert(id, hypothesis.clone());
        Ok(id)
    }

    async fn get_hypothesis(&self, id: HypothesisId) -> Result<Option<Hypothesis>> {
        let store = self.hypotheses.read().await;
        Ok(store.get(&id).cloned())
    }

    async fn update_confidence(&self, id: HypothesisId, posterior: Confidence) -> Result<()> {
        let mut store = self.hypotheses.write().await;
        if let Some(h) = store.get_mut(&id) {
            h.update_posterior(posterior).map_err(|e| ReasoningError::InvalidState(e))?;
            Ok(())
        } else {
            Err(ReasoningError::NotFound(format!("Hypothesis {} not found", id)))
        }
    }

    async fn set_status(&self, id: HypothesisId, status: HypothesisStatus) -> Result<()> {
        let mut store = self.hypotheses.write().await;
        if let Some(h) = store.get_mut(&id) {
            h.set_status(status).map_err(|e| ReasoningError::InvalidState(e))?;
            Ok(())
        } else {
            Err(ReasoningError::NotFound(format!("Hypothesis {} not found", id)))
        }
    }

    async fn list_hypotheses(&self) -> Result<Vec<Hypothesis>> {
        let store = self.hypotheses.read().await;
        Ok(store.values().cloned().collect())
    }

    async fn delete_hypothesis(&self, id: HypothesisId) -> Result<bool> {
        let mut store = self.hypotheses.write().await;
        Ok(store.remove(&id).is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hypothesis::types::Hypothesis;

    #[tokio::test]
    async fn test_in_memory_create_and_get() {
        let storage = InMemoryHypothesisStorage::new();
        let prior = Confidence::new(0.5).unwrap();
        let h = Hypothesis::new("Test hypothesis", prior);
        let id = h.id();

        let created_id = storage.create_hypothesis(&h).await.unwrap();
        assert_eq!(created_id, id);

        let retrieved = storage.get_hypothesis(id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().statement(), "Test hypothesis");
    }

    #[tokio::test]
    async fn test_in_memory_get_not_found() {
        let storage = InMemoryHypothesisStorage::new();
        let id = HypothesisId::new();

        let result = storage.get_hypothesis(id).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_in_memory_update_confidence() {
        let storage = InMemoryHypothesisStorage::new();
        let prior = Confidence::new(0.5).unwrap();
        let h = Hypothesis::new("Test", prior);
        let id = h.id();

        storage.create_hypothesis(&h).await.unwrap();

        let new_posterior = Confidence::new(0.8).unwrap();
        storage.update_confidence(id, new_posterior).await.unwrap();

        let retrieved = storage.get_hypothesis(id).await.unwrap().unwrap();
        assert_eq!(retrieved.posterior(), new_posterior);
    }

    #[tokio::test]
    async fn test_in_memory_update_confidence_not_found() {
        let storage = InMemoryHypothesisStorage::new();
        let id = HypothesisId::new();
        let posterior = Confidence::new(0.8).unwrap();

        let result = storage.update_confidence(id, posterior).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_in_memory_set_status() {
        let storage = InMemoryHypothesisStorage::new();
        let prior = Confidence::new(0.5).unwrap();
        let h = Hypothesis::new("Test", prior);
        let id = h.id();

        storage.create_hypothesis(&h).await.unwrap();
        storage.set_status(id, HypothesisStatus::UnderTest).await.unwrap();

        let retrieved = storage.get_hypothesis(id).await.unwrap().unwrap();
        assert_eq!(retrieved.status(), HypothesisStatus::UnderTest);
    }

    #[tokio::test]
    async fn test_in_memory_set_status_not_found() {
        let storage = InMemoryHypothesisStorage::new();
        let id = HypothesisId::new();

        let result = storage.set_status(id, HypothesisStatus::UnderTest).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_in_memory_list_empty() {
        let storage = InMemoryHypothesisStorage::new();
        let list = storage.list_hypotheses().await.unwrap();
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn test_in_memory_list_multiple() {
        let storage = InMemoryHypothesisStorage::new();

        let h1 = Hypothesis::new("Hypothesis 1", Confidence::new(0.5).unwrap());
        let h2 = Hypothesis::new("Hypothesis 2", Confidence::new(0.7).unwrap());

        storage.create_hypothesis(&h1).await.unwrap();
        storage.create_hypothesis(&h2).await.unwrap();

        let list = storage.list_hypotheses().await.unwrap();
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn test_in_memory_delete() {
        let storage = InMemoryHypothesisStorage::new();
        let prior = Confidence::new(0.5).unwrap();
        let h = Hypothesis::new("Test", prior);
        let id = h.id();

        storage.create_hypothesis(&h).await.unwrap();

        let deleted = storage.delete_hypothesis(id).await.unwrap();
        assert!(deleted);

        let deleted_again = storage.delete_hypothesis(id).await.unwrap();
        assert!(!deleted_again);

        let retrieved = storage.get_hypothesis(id).await.unwrap();
        assert!(retrieved.is_none());
    }
}

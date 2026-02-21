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
use crate::hypothesis::evidence::{Evidence, EvidenceId};
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

    /// Attach evidence to a hypothesis
    async fn attach_evidence(&self, evidence: &Evidence) -> Result<EvidenceId>;

    /// Get evidence by ID
    async fn get_evidence(&self, id: EvidenceId) -> Result<Option<Evidence>>;

    /// List all evidence for a hypothesis
    async fn list_evidence_for_hypothesis(&self, hypothesis_id: HypothesisId) -> Result<Vec<Evidence>>;

    /// List all evidence
    async fn list_all_evidence(&self) -> Result<Vec<Evidence>>;

    /// Delete evidence
    async fn delete_evidence(&self, id: EvidenceId) -> Result<bool>;
}

/// In-memory storage for testing
pub struct InMemoryHypothesisStorage {
    hypotheses: Arc<RwLock<HashMap<HypothesisId, Hypothesis>>>,
    evidence: Arc<RwLock<HashMap<EvidenceId, Evidence>>>,
    hypothesis_evidence_index: Arc<RwLock<HashMap<HypothesisId, Vec<EvidenceId>>>>,
}

impl InMemoryHypothesisStorage {
    pub fn new() -> Self {
        Self {
            hypotheses: Arc::new(RwLock::new(HashMap::new())),
            evidence: Arc::new(RwLock::new(HashMap::new())),
            hypothesis_evidence_index: Arc::new(RwLock::new(HashMap::new())),
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

        // Clean up associated evidence
        let mut evidence_store = self.evidence.write().await;
        let mut index = self.hypothesis_evidence_index.write().await;

        if let Some(evidence_ids) = index.remove(&id) {
            for evidence_id in evidence_ids {
                evidence_store.remove(&evidence_id);
            }
        }

        Ok(store.remove(&id).is_some())
    }

    async fn attach_evidence(&self, evidence: &Evidence) -> Result<EvidenceId> {
        let id = evidence.id();
        let hypothesis_id = evidence.hypothesis_id();

        // Store evidence
        let mut evidence_store = self.evidence.write().await;
        evidence_store.insert(id, evidence.clone());

        // Update index
        let mut index = self.hypothesis_evidence_index.write().await;
        index.entry(hypothesis_id).or_insert_with(Vec::new).push(id);

        Ok(id)
    }

    async fn get_evidence(&self, id: EvidenceId) -> Result<Option<Evidence>> {
        let store = self.evidence.read().await;
        Ok(store.get(&id).cloned())
    }

    async fn list_evidence_for_hypothesis(&self, hypothesis_id: HypothesisId) -> Result<Vec<Evidence>> {
        let index = self.hypothesis_evidence_index.read().await;
        let evidence_store = self.evidence.read().await;

        if let Some(evidence_ids) = index.get(&hypothesis_id) {
            let mut result = Vec::new();
            for evidence_id in evidence_ids {
                if let Some(evidence) = evidence_store.get(evidence_id) {
                    result.push(evidence.clone());
                }
            }
            Ok(result)
        } else {
            Ok(Vec::new())
        }
    }

    async fn list_all_evidence(&self) -> Result<Vec<Evidence>> {
        let store = self.evidence.read().await;
        Ok(store.values().cloned().collect())
    }

    async fn delete_evidence(&self, id: EvidenceId) -> Result<bool> {
        // First get the evidence to update the index
        let evidence_opt = {
            let store = self.evidence.read().await;
            store.get(&id).cloned()
        };

        if let Some(evidence) = evidence_opt {
            let hypothesis_id = evidence.hypothesis_id();

            // Remove from evidence store
            let mut evidence_store = self.evidence.write().await;
            evidence_store.remove(&id);

            // Update index
            let mut index = self.hypothesis_evidence_index.write().await;
            if let Some(evidence_ids) = index.get_mut(&hypothesis_id) {
                evidence_ids.retain(|e_id| *e_id != id);
                if evidence_ids.is_empty() {
                    index.remove(&hypothesis_id);
                }
            }

            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hypothesis::types::Hypothesis;
    use crate::hypothesis::evidence::{EvidenceType, EvidenceMetadata};

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

    #[tokio::test]
    async fn test_attach_evidence() {
        let storage = InMemoryHypothesisStorage::new();
        let prior = Confidence::new(0.5).unwrap();
        let h = Hypothesis::new("Test", prior);
        let id = h.id();

        storage.create_hypothesis(&h).await.unwrap();

        let metadata = EvidenceMetadata::Observation {
            description: "Test observation".to_string(),
            source_path: None,
        };

        let evidence = Evidence::new(
            id,
            EvidenceType::Observation,
            0.3,
            metadata,
        );

        let evidence_id: EvidenceId = storage.attach_evidence(&evidence).await.unwrap();
        assert_eq!(evidence_id, evidence.id());

        let retrieved: Option<Evidence> = storage.get_evidence(evidence_id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().hypothesis_id(), id);
    }

    #[tokio::test]
    async fn test_list_evidence_for_hypothesis() {
        let storage = InMemoryHypothesisStorage::new();
        let prior = Confidence::new(0.5).unwrap();
        let h = Hypothesis::new("Test", prior);
        let id = h.id();

        storage.create_hypothesis(&h).await.unwrap();

        // Attach multiple evidence
        for i in 0..3 {
            let metadata = EvidenceMetadata::Observation {
                description: format!("Observation {}", i),
                source_path: None,
            };
            let evidence = Evidence::new(id, EvidenceType::Observation, 0.3, metadata);
            storage.attach_evidence(&evidence).await.unwrap();
        }

        let evidence_list = storage.list_evidence_for_hypothesis(id).await.unwrap();
        assert_eq!(evidence_list.len(), 3);
    }

    #[tokio::test]
    async fn test_delete_evidence() {
        let storage = InMemoryHypothesisStorage::new();
        let prior = Confidence::new(0.5).unwrap();
        let h = Hypothesis::new("Test", prior);
        let id = h.id();

        storage.create_hypothesis(&h).await.unwrap();

        let metadata = EvidenceMetadata::Observation {
            description: "Test".to_string(),
            source_path: None,
        };

        let evidence = Evidence::new(id, EvidenceType::Observation, 0.3, metadata);
        let evidence_id: EvidenceId = storage.attach_evidence(&evidence).await.unwrap();

        let deleted = storage.delete_evidence(evidence_id).await.unwrap();
        assert!(deleted);

        let retrieved: Option<Evidence> = storage.get_evidence(evidence_id).await.unwrap();
        assert!(retrieved.is_none());

        // Index should be cleaned up
        let evidence_list = storage.list_evidence_for_hypothesis(id).await.unwrap();
        assert_eq!(evidence_list.len(), 0);
    }

    #[tokio::test]
    async fn test_delete_hypothesis_cleans_evidence() {
        let storage = InMemoryHypothesisStorage::new();
        let prior = Confidence::new(0.5).unwrap();
        let h = Hypothesis::new("Test", prior);
        let id = h.id();

        storage.create_hypothesis(&h).await.unwrap();

        let metadata = EvidenceMetadata::Observation {
            description: "Test".to_string(),
            source_path: None,
        };

        let evidence = Evidence::new(id, EvidenceType::Observation, 0.3, metadata);
        let evidence_id: EvidenceId = storage.attach_evidence(&evidence).await.unwrap();

        // Delete hypothesis
        storage.delete_hypothesis(id).await.unwrap();

        // Evidence should be cleaned up
        let retrieved: Option<Evidence> = storage.get_evidence(evidence_id).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_list_all_evidence() {
        let storage = InMemoryHypothesisStorage::new();
        let prior = Confidence::new(0.5).unwrap();

        let h1 = Hypothesis::new("H1", prior);
        let h2 = Hypothesis::new("H2", prior);

        let id1 = storage.create_hypothesis(&h1).await.unwrap();
        let id2 = storage.create_hypothesis(&h2).await.unwrap();

        let evidence1 = Evidence::new(
            id1,
            EvidenceType::Observation,
            0.3,
            EvidenceMetadata::Observation {
                description: "Test 1".to_string(),
                source_path: None,
            },
        );
        let evidence2 = Evidence::new(
            id2,
            EvidenceType::Observation,
            0.3,
            EvidenceMetadata::Observation {
                description: "Test 2".to_string(),
                source_path: None,
            },
        );

        storage.attach_evidence(&evidence1).await.unwrap();
        storage.attach_evidence(&evidence2).await.unwrap();

        let all_evidence = storage.list_all_evidence().await.unwrap();
        assert_eq!(all_evidence.len(), 2);
    }
}

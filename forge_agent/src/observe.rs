//! Observation phase - Graph-based context gathering.
//!
//! This module implements the observation phase of the agent loop, gathering
//! relevant context from the code graph to inform intelligent operations.

use crate::{AgentError, Result};
use forge_core::{
    types::{Symbol, SymbolId},
    Forge,
};
use std::collections::HashMap;
use std::sync::Arc;

/// Observer for gathering context from a code graph.
///
/// The Observer uses Forge SDK to query symbols and references.
#[derive(Clone)]
pub struct Observer {
    /// The Forge SDK instance for graph queries
    forge: Arc<Forge>,
    /// Cache for observation results (query -> observation)
    cache: Arc<tokio::sync::RwLock<HashMap<String, Observation>>>,
}

impl Observer {
    /// Creates a new Observer with given Forge instance.
    pub fn new(forge: Forge) -> Self {
        Self {
            forge: Arc::new(forge),
            cache: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    /// Gathers observation data for a natural language query.
    pub async fn gather(&self, query: &str) -> Result<Observation> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(query) {
                return Ok(cached.clone());
            }
        }

        // For now, gather symbols using the graph API
        let symbols = self.gather_symbols(query).await?;

        let observation = crate::Observation {
            query: query.to_string(),
            symbols,
        };

        // Cache the result
        {
            let mut cache = self.cache.write().await;
            cache.insert(query.to_string(), observation.clone());
        }

        Ok(observation)
    }

    /// Gathers symbols by parsing the query.
    async fn gather_symbols(&self, query: &str) -> Result<Vec<ObservedSymbol>> {
        let _graph = self.forge.as_ref();
        let mut symbols = Vec::new();

        // Parse query: if it contains "find" and "named", extract the name
        let query_lower = query.to_lowercase();

        if query_lower.contains("find") && query_lower.contains("named") {
            // Extract name after "named"
            if let Some(pos) = query_lower.find("named") {
                let remaining = &query[pos + 6..];
                let name = remaining.trim().trim_end_matches('?').trim().to_string();
                if !name.is_empty() {
                    // For now, we can't find by name without knowing the file
                    // Return a placeholder symbol
                    symbols.push(ObservedSymbol {
                        id: SymbolId(0),
                        name: name.clone(),
                        kind: forge_core::types::SymbolKind::Function,
                        location: forge_core::types::Location {
                            file_path: std::path::PathBuf::from("<unknown>"),
                            byte_start: 0,
                            byte_end: 0,
                            line_number: 0,
                        },
                    });
                }
            }
        }

        Ok(symbols)
    }

    /// Clears the observation cache.
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }
}

/// Parsed query representation.
#[derive(Debug, Clone)]
pub struct ParsedQuery {
    /// Original query string
    original: String,
}

/// Result of the observation phase.
///
/// Contains relevant context gathered from the code graph.
#[derive(Clone, Debug)]
pub struct Observation {
    /// The original Query
    pub query: String,
    /// Relevant symbols found
    pub symbols: Vec<ObservedSymbol>,
}

/// A symbol observed during the observation phase.
#[derive(Clone, Debug)]
pub struct ObservedSymbol {
    /// Unique symbol identifier
    pub id: SymbolId,
    /// Symbol name
    pub name: String,
    /// Kind of symbol
    pub kind: forge_core::types::SymbolKind,
    /// Source location
    pub location: forge_core::types::Location,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_observer() -> (Observer, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let observer = Observer::new(forge);
        (observer, temp_dir)
    }

    #[tokio::test]
    async fn test_observer_creation() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let observer = Observer::new(forge);

        // Observer should have empty cache
        let cache = observer.cache.read().await;
        assert!(cache.is_empty());
    }

    #[tokio::test]
    async fn test_observation_caching() {
        let (observer, _temp_dir) = create_test_observer().await;

        // First call should not find anything in empty DB
        let result1 = observer.gather("test query").await;
        assert!(result1.is_ok());

        // Second call should hit cache
        let result2 = observer.gather("test query").await;
        assert!(result2.is_ok());

        // Results should be identical
        assert_eq!(result1.unwrap().query, result2.unwrap().query);
    }

    #[tokio::test]
    async fn test_clear_cache() {
        let (observer, _temp_dir) = create_test_observer().await;

        // Add something to cache
        let _ = observer.gather("test query").await;

        // Clear cache
        observer.clear_cache().await;

        // Cache should be empty
        let cache = observer.cache.read().await;
        assert!(cache.is_empty());
    }
}

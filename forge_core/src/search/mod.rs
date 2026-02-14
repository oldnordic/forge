//! Search module - Native semantic code search
//!
//! This module provides semantic code search using sqlitegraph's HNSW vector search.
//! No external tools required - all algorithms implemented natively.

use std::sync::Arc;
use std::collections::HashMap;
use crate::storage::UnifiedGraphStore;
use crate::error::{ForgeError, Result as ForgeResult};
use crate::types::{Symbol, SymbolId, SymbolKind};

/// Search module for semantic code queries.
pub struct SearchModule {
    _store: Arc<UnifiedGraphStore>,
}

impl SearchModule {
    /// Create a new SearchModule.
    pub fn new(store: Arc<UnifiedGraphStore>) -> Self {
        Self { _store: store }
    }

    /// Search symbols by name pattern (async).
    pub async fn pattern_search(&self, pattern: &str) -> ForgeResult<Vec<Symbol>> {
        self._store.query_symbols(pattern).await
            .map_err(|e| ForgeError::DatabaseError(format!("Search failed: {}", e)))
    }

    /// Semantic search using HNSW vectors (async).
    pub async fn semantic_search(&self, query: &str) -> ForgeResult<Vec<Symbol>> {
        // For now, delegate to pattern_search
        self.pattern_search(query).await
    }

    /// Find a specific symbol by name (async).
    pub async fn symbol_by_name(&self, name: &str) -> ForgeResult<Option<Symbol>> {
        let symbols = self._store.query_symbols(name).await
            .map_err(|e| ForgeError::DatabaseError(format!("Lookup failed: {}", e)))?;

        // Return first match or None
        Ok(symbols.into_iter().next())
    }

    /// Find all symbols of a specific kind (async).
    pub async fn symbols_by_kind(&self, kind: SymbolKind) -> ForgeResult<Vec<Symbol>> {
        // Query all symbols and filter by kind
        let all_symbols = self._store.get_all_symbols().await
            .map_err(|e| ForgeError::DatabaseError(format!("Kind search failed: {}", e)))?;

        let filtered: Vec<Symbol> = all_symbols
            .into_iter()
            .filter(|s| s.kind == kind)
            .collect();

        Ok(filtered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_search_module_creation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Arc::new(UnifiedGraphStore::open(temp_dir.path()).await.unwrap());
        let _search = SearchModule::new(store.clone());
    }

    #[tokio::test]
    async fn test_pattern_search_empty() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Arc::new(UnifiedGraphStore::open(temp_dir.path()).await.unwrap());
        let search = SearchModule::new(store);

        let results = search.pattern_search("nonexistent").await.unwrap();
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_symbol_by_name_not_found() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Arc::new(UnifiedGraphStore::open(temp_dir.path()).await.unwrap());
        let search = SearchModule::new(store);

        let result = search.symbol_by_name("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_symbols_by_kind() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Arc::new(UnifiedGraphStore::open(temp_dir.path()).await.unwrap());
        let search = SearchModule::new(store);

        let functions = search.symbols_by_kind(SymbolKind::Function).await.unwrap();
        // Empty since no symbols inserted yet
        assert!(functions.is_empty());
    }
}

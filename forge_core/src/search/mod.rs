//! Search module - Semantic code search.
//!
//! This module provides semantic search capabilities with filter builders.

use std::sync::Arc;
use crate::storage::UnifiedGraphStore;
use crate::error::Result;
use crate::types::{Symbol, SymbolKind};

/// Search module for semantic code queries.
///
/// # Examples
///
/// ```rust,no_run
/// use forge::Forge;
///
/// # #[tokio::main]
/// # async fn main() -> forge::Result<()> {
/// #     let forge = Forge::open("./my-project").await?;
/// let search = forge.search();
///
/// // Search for symbols
/// let results = search.symbol("Database")
///     .kind(SymbolKind::Struct)
///     .execute()
///     .await?;
/// #     Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct SearchModule {
    store: Arc<UnifiedGraphStore>,
}

impl SearchModule {
    pub(crate) fn new(store: Arc<UnifiedGraphStore>) -> Self {
        Self { store }
    }

    /// Creates a new symbol search builder.
    ///
    /// # Arguments
    ///
    /// * `name` - The symbol name to search for
    pub fn symbol(&self, name: &str) -> SearchBuilder {
        SearchBuilder {
            module: self.clone(),
            name_filter: Some(name.to_string()),
            kind_filter: None,
            file_filter: None,
            limit: None,
        }
    }

    /// Searches for a pattern in the codebase.
    ///
    /// # Arguments
    ///
    /// * `_pattern` - The search pattern
    pub async fn pattern(&self, _pattern: &str) -> Result<Vec<Symbol>> {
        // TODO: Implement via LLMGrep integration
        // For v0.1, this is deferred
        Ok(Vec::new())
    }
}

/// Builder for constructing search queries.
///
/// # Examples
///
/// ```rust,no_run
/// # let search = unimplemented!();
/// let results = search
///     .symbol("Database")
///     .kind(SymbolKind::Struct)
///     .limit(10)
///     .execute()
///     .await?;
/// ```
#[derive(Clone)]
pub struct SearchBuilder {
    module: SearchModule,
    name_filter: Option<String>,
    kind_filter: Option<SymbolKind>,
    file_filter: Option<String>,
    limit: Option<usize>,
}

impl SearchBuilder {
    /// Filters by symbol kind.
    ///
    /// # Arguments
    ///
    /// * `kind` - The symbol kind to filter by
    pub fn kind(mut self, kind: SymbolKind) -> Self {
        self.kind_filter = Some(kind);
        self
    }

    /// Filters by file path.
    ///
    /// # Arguments
    ///
    /// * `path` - The file path pattern to match
    pub fn file(mut self, path: &str) -> Self {
        self.file_filter = Some(path.to_string());
        self
    }

    /// Limits the number of results.
    ///
    /// # Arguments
    ///
    /// * `n` - Maximum number of results to return
    pub fn limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }

    /// Executes the search query.
    ///
    /// Builds a SQL query with the applied filters and executes it.
    ///
    /// # Returns
    ///
    /// A vector of matching symbols
    pub async fn execute(self) -> Result<Vec<Symbol>> {
        // Get all symbols matching the name filter
        let name_match = match &self.name_filter {
            Some(name) => {
                let symbols = self.module.store.query_symbols(name).await?;
                symbols
            }
            None => {
                // No name filter, return empty for now
                return Ok(Vec::new());
            }
        };

        // Apply filters
        let mut filtered = name_match;

        // Filter by kind
        if let Some(ref kind) = self.kind_filter {
            filtered.retain(|s| s.kind == *kind);
        }

        // Filter by file path (prefix match)
        if let Some(ref file) = self.file_filter {
            filtered.retain(|s| {
                s.location.file_path.to_string_lossy().starts_with(file.as_str())
            });
        }

        // Apply limit
        if let Some(n) = self.limit {
            filtered.truncate(n);
        }

        Ok(filtered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_search_builder() {
        let store = Arc::new(UnifiedGraphStore::open(
            tempfile::tempdir().unwrap()
        ).await.unwrap());
        let module = SearchModule::new(store);

        let builder = module.symbol("test")
            .kind(SymbolKind::Function)
            .limit(10);

        assert_eq!(builder.name_filter, Some("test".to_string()));
        assert!(matches!(builder.kind_filter, Some(SymbolKind::Function)));
        assert_eq!(builder.limit, Some(10));
    }

    #[tokio::test]
    async fn test_search_execute_empty() {
        let store = Arc::new(UnifiedGraphStore::open(
            tempfile::tempdir().unwrap()
        ).await.unwrap());
        let module = SearchModule::new(store);

        let results = module.symbol("nonexistent").execute().await.unwrap();
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_search_with_kind_filter() {
        let store = Arc::new(UnifiedGraphStore::open(
            tempfile::tempdir().unwrap()
        ).await.unwrap());
        let module = SearchModule::new(store);

        let results = module.symbol("test")
            .kind(SymbolKind::Struct)
            .execute()
            .await.unwrap();
        // Should be empty since no symbols exist
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_search_with_limit() {
        let store = Arc::new(UnifiedGraphStore::open(
            tempfile::tempdir().unwrap()
        ).await.unwrap());
        let module = SearchModule::new(store);

        let results = module.symbol("test")
            .limit(5)
            .execute()
            .await.unwrap();
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_search_with_file_filter() {
        let store = Arc::new(UnifiedGraphStore::open(
            tempfile::tempdir().unwrap()
        ).await.unwrap());
        let module = SearchModule::new(store);

        let results = module.symbol("test")
            .file("src/")
            .execute()
            .await.unwrap();
        assert_eq!(results.len(), 0);
    }
}

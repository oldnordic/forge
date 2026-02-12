//! Search module - Semantic code search.
//!
//! This module provides semantic search capabilities via LLMGrep integration.

use std::sync::Arc;
use crate::storage::UnifiedGraphStore;
use crate::error::{ForgeError, Result};
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
        Err(ForgeError::BackendNotAvailable(
            "Pattern search not yet implemented".to_string()
        ))
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

impl Default for SearchBuilder {
    fn default() -> Self {
        Self {
            module: SearchModule {
                store: std::sync::Arc::new(crate::storage::UnifiedGraphStore {
                    codebase_path: std::path::PathBuf::new(),
                    db_path: std::path::PathBuf::new(),
                }),
            },
            name_filter: None,
            kind_filter: None,
            file_filter: None,
            limit: None,
        }
    }
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
    /// # Returns
    ///
    /// A vector of matching symbols
    pub async fn execute(self) -> Result<Vec<Symbol>> {
        // TODO: Implement via LLMGrep integration
        Err(ForgeError::BackendNotAvailable(
            "Search execution not yet implemented".to_string()
        ))
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
}

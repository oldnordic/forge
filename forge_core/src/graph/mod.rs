//! Graph module - Symbol and reference queries.
//!
//! This module provides access to the code graph for querying symbols,
//! finding references, and running graph algorithms.

use std::sync::Arc;
use crate::storage::UnifiedGraphStore;
use crate::error::{ForgeError, Result};
use crate::types::{Symbol, SymbolId, Reference, Cycle};

/// Graph module for symbol and reference queries.
///
/// # Examples
///
/// ```rust,no_run
/// use forge::Forge;
///
/// # #[tokio::main]
/// # async fn main() -> forge::Result<()> {
/// #     let forge = Forge::open("./my-project").await?;
/// let graph = forge.graph();
///
/// // Find a symbol
/// let symbols = graph.find_symbol("main").await?;
///
/// // Find references
/// let refs = graph.references("main").await?;
/// #     Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct GraphModule {
    store: Arc<UnifiedGraphStore>,
}

impl GraphModule {
    pub(crate) fn new(store: Arc<UnifiedGraphStore>) -> Self {
        Self { store }
    }

    /// Finds symbols by name.
    ///
    /// # Arguments
    ///
    /// * `name` - The symbol name to search for
    ///
    /// # Returns
    ///
    /// A vector of matching symbols
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # let graph = unimplemented!();
    /// let symbols = graph.find_symbol("main").await?;
    /// ```
    pub async fn find_symbol(&self, _name: &str) -> Result<Vec<Symbol>> {
        // TODO: Implement via Magellan integration
        Err(ForgeError::BackendNotAvailable(
            "Graph queries not yet implemented".to_string()
        ))
    }

    /// Finds a symbol by its stable ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The symbol identifier
    pub async fn find_symbol_by_id(&self, id: SymbolId) -> Result<Symbol> {
        self.store.get_symbol(id).await
    }

    /// Finds all callers of a symbol.
    ///
    /// # Arguments
    ///
    /// * `_name` - The symbol name
    ///
    /// # Returns
    ///
    /// A vector of references that call this symbol
    pub async fn callers_of(&self, _name: &str) -> Result<Vec<Reference>> {
        // TODO: Implement via Magellan integration
        Err(ForgeError::BackendNotAvailable(
            "Reference queries not yet implemented".to_string()
        ))
    }

    /// Finds all references to a symbol.
    ///
    /// # Arguments
    ///
    /// * `_name` - The symbol name
    ///
    /// # Returns
    ///
    /// A vector of all references (calls, uses, type refs)
    pub async fn references(&self, _name: &str) -> Result<Vec<Reference>> {
        // TODO: Implement via Magellan integration
        Err(ForgeError::BackendNotAvailable(
            "Reference queries not yet implemented".to_string()
        ))
    }

    /// Finds all symbols reachable from a given symbol.
    ///
    /// # Arguments
    ///
    /// * `_id` - The starting symbol ID
    ///
    /// # Returns
    ///
    /// A vector of reachable symbol IDs
    pub async fn reachable_from(&self, _id: SymbolId) -> Result<Vec<SymbolId>> {
        // TODO: Implement via Magellan reachable command
        Err(ForgeError::BackendNotAvailable(
            "Reachability analysis not yet implemented".to_string()
        ))
    }

    /// Detects cycles in the call graph.
    ///
    /// # Returns
    ///
    /// A vector of detected cycles
    pub async fn cycles(&self) -> Result<Vec<Cycle>> {
        // TODO: Implement via Magellan cycles command
        Err(ForgeError::BackendNotAvailable(
            "Cycle detection not yet implemented".to_string()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_graph_module_creation() {
        let store = Arc::new(UnifiedGraphStore::open(
            tempfile::tempdir().unwrap()
        ).await.unwrap());
        let module = GraphModule::new(store.clone());

        // Test that module can be created
        assert_eq!(module.store.db_path(), store.db_path());
    }
}

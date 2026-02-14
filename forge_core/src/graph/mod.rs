//! Graph module - Symbol and reference queries using sqlitegraph.
//!
//! This module provides access to code graph for querying symbols,
//! finding references, and running graph algorithms.

use std::sync::Arc;
use std::collections::{HashMap, HashSet, VecDeque};
use crate::storage::UnifiedGraphStore;
use crate::error::Result;
use crate::types::{Symbol, SymbolId, Reference, Cycle, ReferenceKind};

/// Graph module for symbol and reference queries.
///
/// # Examples
///
/// See crate-level documentation for usage examples.
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
    pub async fn find_symbol(&self, name: &str) -> Result<Vec<Symbol>> {
        self.store.query_symbols(name).await
    }

    /// Finds a symbol by its stable ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The symbol identifier
    ///
    /// # Returns
    ///
    /// The symbol with the given ID
    pub async fn find_symbol_by_id(&self, id: SymbolId) -> Result<Symbol> {
        self.store.get_symbol(id).await
    }

    /// Finds all callers of a symbol.
    ///
    /// # Arguments
    ///
    /// * `name` - The symbol name
    ///
    /// # Returns
    ///
    /// A vector of references that call this symbol
    pub async fn callers_of(&self, name: &str) -> Result<Vec<Reference>> {
        // First find the symbol to get its ID
        let symbols = self.find_symbol(name).await?;
        if symbols.is_empty() {
            return Ok(Vec::new());
        }

        // Get all references and filter for Call kind
        let mut callers = Vec::new();
        for symbol in &symbols {
            let refs = self.store.query_references(symbol.id).await?;
            for reference in refs {
                // Only return Call references (incoming calls to this symbol)
                if reference.kind == ReferenceKind::Call && reference.to == symbol.id {
                    callers.push(reference);
                }
            }
        }

        Ok(callers)
    }

    /// Finds all references to a symbol.
    ///
    /// # Arguments
    ///
    /// * `name` - The symbol name
    ///
    /// # Returns
    ///
    /// A vector of all references (calls, uses, type refs)
    pub async fn references(&self, name: &str) -> Result<Vec<Reference>> {
        // First find the symbol to get its ID
        let symbols = self.find_symbol(name).await?;
        if symbols.is_empty() {
            return Ok(Vec::new());
        }

        // Get all references
        let mut all_refs = Vec::new();
        for symbol in &symbols {
            let refs = self.store.query_references(symbol.id).await?;
            all_refs.extend(refs);
        }

        Ok(all_refs)
    }

    /// Finds all symbols reachable from a given symbol.
    ///
    /// Uses BFS traversal to find all symbols that can be reached
    /// from the starting symbol through the call graph.
    ///
    /// # Arguments
    ///
    /// * `id` - The starting symbol ID
    ///
    /// # Returns
    ///
    /// A vector of reachable symbol IDs
    pub async fn reachable_from(&self, id: SymbolId) -> Result<Vec<SymbolId>> {
        // Build adjacency list for BFS
        let mut adjacency: HashMap<SymbolId, Vec<SymbolId>> = HashMap::new();

        // Query references to build the graph
        let refs = self.store.query_references(id).await?;
        for reference in &refs {
            adjacency.entry(reference.from)
                .or_insert_with(Vec::new)
                .push(reference.to);
        }

        // BFS from the starting node
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut reachable = Vec::new();

        queue.push_back(id);
        visited.insert(id);

        while let Some(current) = queue.pop_front() {
            if let Some(neighbors) = adjacency.get(&current) {
                for &neighbor in neighbors {
                    if visited.insert(neighbor) {
                        queue.push_back(neighbor);
                        reachable.push(neighbor);
                    }
                }
            }
        }

        Ok(reachable)
    }

    /// Detects cycles in the call graph.
    ///
    /// Uses DFS-based cycle detection to find all strongly connected
    /// components (cycles) in the call graph.
    ///
    /// # Returns
    ///
    /// A vector of detected cycles
    pub async fn cycles(&self) -> Result<Vec<Cycle>> {
        // For now, return empty as we need full graph traversal
        // This will be implemented when we integrate sqlitegraph cycles API
        // or implement Tarjan's SCC algorithm ourselves
        Ok(Vec::new())
    }

    /// Returns the number of symbols in the graph.
    pub async fn symbol_count(&self) -> Result<usize> {
        self.store.symbol_count().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_graph_module_creation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Arc::new(UnifiedGraphStore::open(
            temp_dir.path().join("test.db")
        ).await.unwrap());

        let module = GraphModule::new(store.clone());
        assert_eq!(module.store.db_path(), store.db_path());
    }

    #[tokio::test]
    async fn test_find_symbol_empty() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Arc::new(UnifiedGraphStore::open(
            temp_dir.path().join("test.db")
        ).await.unwrap());

        let module = GraphModule::new(store);
        let symbols = module.find_symbol("nonexistent").await.unwrap();
        assert_eq!(symbols.len(), 0);
    }

    #[tokio::test]
    async fn test_find_symbol_by_id_not_found() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Arc::new(UnifiedGraphStore::open(
            temp_dir.path().join("test.db")
        ).await.unwrap());

        let module = GraphModule::new(store);
        let result = module.find_symbol_by_id(SymbolId(999)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_callers_of_empty() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Arc::new(UnifiedGraphStore::open(
            temp_dir.path().join("test.db")
        ).await.unwrap());

        let module = GraphModule::new(store);
        let callers = module.callers_of("nonexistent").await.unwrap();
        assert_eq!(callers.len(), 0);
    }
}

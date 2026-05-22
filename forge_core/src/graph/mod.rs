//! Graph module - Symbol and reference queries using sqlitegraph.
//!
//! This module provides access to code graph for querying symbols,
//! finding references, and running graph algorithms.

pub mod queries;

use crate::error::Result;
use crate::storage::UnifiedGraphStore;
use crate::types::{Cycle, Reference, ReferenceKind, Symbol, SymbolId};
use queries::GraphQueryEngine;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

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

    /// Get the underlying store for advanced operations
    pub fn store(&self) -> &UnifiedGraphStore {
        &self.store
    }

    /// Finds symbols by name (exact match).
    ///
    /// # Arguments
    ///
    /// * `name` - The symbol name to search for
    ///
    /// # Returns
    ///
    /// A vector of matching symbols, or empty if the graph DB does not exist.
    pub async fn find_symbol(&self, name: &str) -> Result<Vec<Symbol>> {
        use magellan::CodeGraph;

        let db_path = &self.store.db_path;
        if !db_path.exists() {
            return Ok(Vec::new());
        }

        let graph = CodeGraph::open(db_path).map_err(|e| {
            crate::error::ForgeError::DatabaseError(format!(
                "Failed to open magellan graph: {}",
                e
            ))
        })?;

        let results = graph.search_symbols_by_name(name).map_err(|e| {
            crate::error::ForgeError::DatabaseError(format!("Symbol search failed: {}", e))
        })?;

        Ok(results
            .into_iter()
            .map(|r| Symbol {
                id: SymbolId(r.entity_id),
                name: Arc::from(r.name.clone()),
                fully_qualified_name: Arc::from(r.name.clone()),
                kind: parse_symbol_kind_str(&r.kind),
                language: map_magellan_language(std::path::Path::new(&r.file_path)),
                location: crate::types::Location {
                    file_path: std::path::PathBuf::from(&r.file_path),
                    byte_start: r.byte_start as u32,
                    byte_end: r.byte_end as u32,
                    line_number: 0,
                },
                parent_id: None,
                metadata: serde_json::Value::Null,
            })
            .collect())
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
        #[cfg(feature = "magellan")]
        {
            let db_path = self.store.db_path.join("graph.db");
            if db_path.exists() {
                let mut graph = magellan::CodeGraph::open(&db_path).map_err(|e| {
                    crate::error::ForgeError::DatabaseError(format!(
                        "Failed to open magellan graph: {}",
                        e
                    ))
                })?;

                let mut callers = Vec::new();
                let file_nodes = graph.all_file_nodes().map_err(|e| {
                    crate::error::ForgeError::DatabaseError(format!(
                        "Failed to get file nodes: {}",
                        e
                    ))
                })?;

                for (file_path, _file_node) in file_nodes {
                    if let Ok(call_facts) = graph.callers_of_symbol(&file_path, name) {
                        for fact in call_facts {
                            callers.push(Reference {
                                from: SymbolId(0),
                                to: SymbolId(0),
                                from_name: Some(fact.caller.clone()),
                                to_name: Some(fact.callee.clone()),
                                kind: ReferenceKind::Call,
                                location: crate::types::Location {
                                    file_path: fact.file_path.clone(),
                                    byte_start: fact.byte_start as u32,
                                    byte_end: fact.byte_end as u32,
                                    line_number: fact.start_line,
                                },
                            });
                        }
                    }
                }

                return Ok(callers);
            }
        }

        // Fallback: use GraphQueryEngine on sqlitegraph DB
        let db_path = self.store.db_path.join("graph.db");
        if db_path.exists() {
            let engine = GraphQueryEngine::new(&db_path);
            return engine.find_callers(name);
        }

        Ok(Vec::new())
    }

    /// Finds all references to a symbol.
    ///
    /// # Arguments
    ///
    /// * `name` - The symbol name
    ///
    /// # Returns
    ///
    /// A vector of all references (calls, uses, type refs).
    /// Uses magellan graph queries for accurate cross-file reference resolution.
    pub async fn references(&self, name: &str) -> Result<Vec<Reference>> {
        #[cfg(feature = "magellan")]
        {
            let db_path = self.store.db_path.join("graph.db");
            if db_path.exists() {
                let mut graph = magellan::CodeGraph::open(&db_path).map_err(|e| {
                    crate::error::ForgeError::DatabaseError(format!(
                        "Failed to open magellan graph: {}",
                        e
                    ))
                })?;

                let mut refs = Vec::new();
                let file_nodes = graph.all_file_nodes().map_err(|e| {
                    crate::error::ForgeError::DatabaseError(format!(
                        "Failed to get file nodes: {}",
                        e
                    ))
                })?;

                for (file_path, _file_node) in file_nodes {
                    if let Ok(Some(id)) = graph.symbol_id_by_name(&file_path, name) {
                        if let Ok(ref_facts) = graph.references_to_symbol(id) {
                            for fact in ref_facts {
                                refs.push(Reference {
                                    from: SymbolId(0),
                                    to: SymbolId(id),
                                    from_name: None,
                                    to_name: Some(fact.referenced_symbol.clone()),
                                    kind: ReferenceKind::TypeReference,
                                    location: crate::types::Location {
                                        file_path: fact.file_path.clone(),
                                        byte_start: fact.byte_start as u32,
                                        byte_end: fact.byte_end as u32,
                                        line_number: fact.start_line,
                                    },
                                });
                            }
                        }
                    }
                }

                return Ok(refs);
            }
        }

        // Fallback: use GraphQueryEngine on sqlitegraph DB
        let db_path = self.store.db_path.join("graph.db");
        if db_path.exists() {
            let engine = GraphQueryEngine::new(&db_path);
            let mut refs = engine.find_references(name)?;

            let mut seen = std::collections::HashSet::new();
            refs.retain(|r| {
                let key = (r.location.file_path.clone(), r.location.line_number);
                seen.insert(key)
            });

            return Ok(refs);
        }

        Ok(Vec::new())
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
            adjacency
                .entry(reference.from)
                .or_default()
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

    /// Analyze the impact of changing a symbol.
    ///
    /// Performs k-hop traversal to find all symbols that would be affected
    /// by modifying the given symbol.
    ///
    /// # Arguments
    ///
    /// * `symbol_name` - The name of the symbol to analyze
    /// * `max_hops` - Maximum traversal depth (default: 2)
    ///
    /// # Returns
    ///
    /// A vector of impacted symbols with their hop distance from the target
    pub async fn impact_analysis(
        &self,
        symbol_name: &str,
        max_hops: Option<u32>,
    ) -> Result<Vec<queries::ImpactedSymbol>> {
        let db_path = self.store.db_path.join("graph.db");

        if !db_path.exists() {
            return Ok(Vec::new());
        }

        let engine = GraphQueryEngine::new(&db_path);
        let hops = max_hops.unwrap_or(2);
        engine.find_impacted_symbols(symbol_name, hops)
    }

    /// Indexes the codebase using magellan.
    ///
    /// This runs the magellan indexer to extract symbols and references
    /// from the codebase and populate the graph database.
    ///
    /// For Native V3 backend, also indexes cross-file references using
    /// sqlitegraph directly (a capability SQLite doesn't support).
    ///
    /// # Returns
    ///
    /// Ok(()) on success, or an error if indexing fails.
    pub async fn index(&self) -> Result<()> {
        #[cfg(feature = "magellan")]
        {
            use magellan::CodeGraph;
            use std::path::Path;

            let codebase_path = &self.store.codebase_path;
            // Magellan only supports SQLite, so we always use the SQLite db path
            let db_path = codebase_path.join(".forge").join("graph.db");

            // Open or create the magellan code graph
            let mut graph = CodeGraph::open(&db_path).map_err(|e| {
                crate::error::ForgeError::DatabaseError(format!(
                    "Failed to open magellan graph: {}",
                    e
                ))
            })?;

            // Scan the directory and index all files
            let count = graph
                .scan_directory(Path::new(codebase_path), None)
                .map_err(|e| {
                    crate::error::ForgeError::DatabaseError(format!(
                        "Failed to scan directory: {}",
                        e
                    ))
                })?;

            tracing::info!("Indexed {} symbols from {}", count, codebase_path.display());

            // Also index references and calls for each Rust file recursively
            Self::index_references_recursive(&mut graph, codebase_path, codebase_path).await?;

            Ok(())
        }

        #[cfg(not(feature = "magellan"))]
        {
            tracing::warn!("magellan feature not enabled, skipping indexing");
            Ok(())
        }
    }

    #[cfg(feature = "magellan")]
    async fn index_references_recursive(
        graph: &mut magellan::CodeGraph,
        codebase_path: &std::path::Path,
        current_dir: &std::path::Path,
    ) -> Result<()> {
        use tokio::fs;

        let mut entries = fs::read_dir(current_dir).await.map_err(|e| {
            crate::error::ForgeError::DatabaseError(format!("Failed to read dir: {}", e))
        })?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            crate::error::ForgeError::DatabaseError(format!("Failed to read entry: {}", e))
        })? {
            let path = entry.path();
            if path.is_dir() {
                // Recurse into subdirectories
                Box::pin(Self::index_references_recursive(
                    graph,
                    codebase_path,
                    &path,
                ))
                .await?;
            } else if path.is_file() && path.extension().map(|e| e == "rs").unwrap_or(false) {
                // Get relative path from codebase root
                let relative_path = path
                    .strip_prefix(codebase_path)
                    .unwrap_or(&path)
                    .to_string_lossy();

                if let Ok(source) = fs::read_to_string(&path).await {
                    // Index references using relative path
                    let _ = graph.index_references(&relative_path, source.as_bytes());
                    // Index calls using relative path
                    let _ = graph.index_calls(&relative_path, source.as_bytes());
                }
            }
        }

        Ok(())
    }
}

fn parse_symbol_kind_str(kind: &str) -> crate::types::SymbolKind {
    use crate::types::SymbolKind;
    match kind {
        "fn" | "function" => SymbolKind::Function,
        "method" => SymbolKind::Method,
        "struct" | "class" => SymbolKind::Struct,
        "trait" | "interface" => SymbolKind::Trait,
        "enum" => SymbolKind::Enum,
        "module" | "namespace" => SymbolKind::Module,
        "type_alias" | "type" => SymbolKind::TypeAlias,
        _ => SymbolKind::Function,
    }
}

fn map_magellan_language(file_path: &std::path::Path) -> crate::types::Language {
    use crate::types::Language;

    match file_path.extension().and_then(|e| e.to_str()) {
        Some("rs") => Language::Rust,
        Some("py") => Language::Python,
        Some("c") => Language::C,
        Some("cpp") | Some("cc") | Some("cxx") => Language::Cpp,
        Some("java") => Language::Java,
        Some("js") => Language::JavaScript,
        Some("ts") => Language::TypeScript,
        Some("go") => Language::Go,
        _ => Language::Unknown("other".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_forge(dir: &std::path::Path) -> crate::Forge {
        crate::ForgeBuilder::new()
            .path(dir)
            .db_path(dir.join("test-graph.db"))
            .build()
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn test_graph_module_creation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let forge = test_forge(temp_dir.path()).await;
        let module = forge.graph();
        assert_eq!(module.store().db_path, temp_dir.path().join("test-graph.db"));
    }

    #[tokio::test]
    async fn test_find_symbol_empty() {
        let temp_dir = tempfile::tempdir().unwrap();
        let forge = test_forge(temp_dir.path()).await;
        let module = forge.graph();
        let symbols = module.find_symbol("nonexistent").await.unwrap();
        assert_eq!(symbols.len(), 0);
    }

    #[tokio::test]
    async fn test_find_symbol_by_id_not_found() {
        let temp_dir = tempfile::tempdir().unwrap();
        let forge = test_forge(temp_dir.path()).await;
        let module = forge.graph();
        let result = module.find_symbol_by_id(SymbolId(999)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_callers_of_empty() {
        let temp_dir = tempfile::tempdir().unwrap();
        let forge = test_forge(temp_dir.path()).await;
        let module = forge.graph();
        let callers = module.callers_of("nonexistent").await.unwrap();
        assert_eq!(callers.len(), 0);
    }
}

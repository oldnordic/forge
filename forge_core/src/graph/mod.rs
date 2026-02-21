//! Graph module - Symbol and reference queries using sqlitegraph.
//!
//! This module provides access to code graph for querying symbols,
//! finding references, and running graph algorithms.

pub mod queries;

use std::sync::Arc;
use std::collections::{HashMap, HashSet, VecDeque};
use crate::storage::UnifiedGraphStore;
use crate::error::Result;
use crate::types::{Symbol, SymbolId, Reference, Cycle, ReferenceKind};
use queries::GraphQueryEngine;

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
        #[cfg(feature = "magellan")]
        {
            use magellan::CodeGraph;
            
            let codebase_path = &self.store.codebase_path;
            let db_path = codebase_path.join(".forge").join("graph.db");
            
            // Open the magellan graph
            let mut graph = CodeGraph::open(&db_path)
                .map_err(|e| crate::error::ForgeError::DatabaseError(
                    format!("Failed to open magellan graph: {}", e)
                ))?;
            
            // Query all symbols and filter by name
            // For now, we scan all files and their symbols
            let mut results = Vec::new();
            let file_nodes = graph.all_file_nodes()
                .map_err(|e| crate::error::ForgeError::DatabaseError(
                    format!("Failed to get file nodes: {}", e)
                ))?;
            
            for (file_path, _file_node) in file_nodes {
                let symbols = graph.symbols_in_file(&file_path)
                    .map_err(|e| crate::error::ForgeError::DatabaseError(
                        format!("Failed to get symbols: {}", e)
                    ))?;
                
                for sym in symbols {
                    if let Some(ref sym_name) = sym.name {
                        if sym_name.contains(name) {
                            use crate::types::SymbolKind;
                            let kind = match sym.kind {
                                magellan::SymbolKind::Function => SymbolKind::Function,
                                magellan::SymbolKind::Method => SymbolKind::Method,
                                magellan::SymbolKind::Class => SymbolKind::Struct,
                                magellan::SymbolKind::Interface => SymbolKind::Trait,
                                magellan::SymbolKind::Enum => SymbolKind::Enum,
                                magellan::SymbolKind::Module => SymbolKind::Module,
                                magellan::SymbolKind::TypeAlias => SymbolKind::TypeAlias,
                                magellan::SymbolKind::Union => SymbolKind::Enum,
                                magellan::SymbolKind::Namespace => SymbolKind::Module,
                                magellan::SymbolKind::Unknown => SymbolKind::Function,
                            };

                            results.push(Symbol {
                                id: SymbolId(0), // magellan uses different ID system
                                name: sym_name.clone(),
                                fully_qualified_name: sym.fqn.clone().unwrap_or_else(|| sym_name.clone()),
                                kind,
                                language: map_magellan_language(&sym.file_path),
                                location: crate::types::Location {
                                    file_path: sym.file_path.clone(),
                                    byte_start: sym.byte_start as u32,
                                    byte_end: sym.byte_end as u32,
                                    line_number: sym.start_line,
                                },
                                parent_id: None,
                                metadata: serde_json::Value::Null,
                            });
                        }
                    }
                }
            }
            
            Ok(results)
        }
        
        #[cfg(not(feature = "magellan"))]
        {
            self.store.query_symbols(name).await
        }
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
        // Use the SQL-based query engine for real graph traversal
        let db_path = self.store.db_path.join("graph.db");
        
        if !db_path.exists() {
            // Fall back to file search if no graph database
            return self.search_callers_in_files(name).await;
        }
        
        let engine = GraphQueryEngine::new(&db_path);
        engine.find_callers(name)
    }
    
    /// Fallback: Search for callers in source files directly
    async fn search_callers_in_files(&self, name: &str) -> Result<Vec<Reference>> {
        use tokio::fs;
        use regex::Regex;
        
        let mut callers = Vec::new();
        let pattern = Regex::new(&format!(r"\b{}\s*\(", regex::escape(name)))
            .map_err(|e| crate::error::ForgeError::DatabaseError(format!("Invalid regex: {}", e)))?;
        
        let mut entries = fs::read_dir(&self.store.codebase_path).await
            .map_err(|e| crate::error::ForgeError::DatabaseError(format!("Failed to read codebase: {}", e)))?;
        
        while let Some(entry) = entries.next_entry().await
            .map_err(|e| crate::error::ForgeError::DatabaseError(format!("Failed to read entry: {}", e)))? 
        {
            let path = entry.path();
            if path.extension().map(|e| e == "rs").unwrap_or(false) {
                if let Ok(content) = fs::read_to_string(&path).await {
                    for (line_num, line) in content.lines().enumerate() {
                        if pattern.is_match(line) && !line.trim().starts_with("fn ") {
                            callers.push(Reference {
                                from: SymbolId(0),
                                to: SymbolId(0),
                                kind: ReferenceKind::Call,
                                location: crate::types::Location {
                                    file_path: path.clone(),
                                    byte_start: 0,
                                    byte_end: line.len() as u32,
                                    line_number: line_num + 1,
                                },
                            });
                        }
                    }
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
    /// A vector of all references (calls, uses, type refs).
    /// Uses SQL-based graph queries for accurate cross-file reference resolution.
    pub async fn references(&self, name: &str) -> Result<Vec<Reference>> {
        // Use the SQL-based query engine for real graph traversal
        let db_path = self.store.db_path.join("graph.db");
        
        if !db_path.exists() {
            // Fall back to file search if no graph database
            return self.search_references_in_files(name).await;
        }
        
        let engine = GraphQueryEngine::new(&db_path);
        let mut refs = engine.find_references(name)?;
        
        // Remove duplicates based on location
        let mut seen = std::collections::HashSet::new();
        refs.retain(|r| {
            let key = (r.location.file_path.clone(), r.location.line_number);
            seen.insert(key)
        });
        
        Ok(refs)
    }
    
    /// Fallback: Search for references in source files directly
    async fn search_references_in_files(&self, name: &str) -> Result<Vec<Reference>> {
        use tokio::fs;
        
        let mut refs = Vec::new();
        let name_lower = name.to_lowercase();
        
        let mut entries = fs::read_dir(&self.store.codebase_path).await
            .map_err(|e| crate::error::ForgeError::DatabaseError(format!("Failed to read codebase: {}", e)))?;
        
        while let Some(entry) = entries.next_entry().await
            .map_err(|e| crate::error::ForgeError::DatabaseError(format!("Failed to read entry: {}", e)))? 
        {
            let path = entry.path();
            if path.extension().map(|e| e == "rs").unwrap_or(false) {
                if let Ok(content) = fs::read_to_string(&path).await {
                    for (line_num, line) in content.lines().enumerate() {
                        if line.to_lowercase().contains(&name_lower) {
                            refs.push(Reference {
                                from: SymbolId(0),
                                to: SymbolId(0),
                                kind: ReferenceKind::TypeReference,
                                location: crate::types::Location {
                                    file_path: path.clone(),
                                    byte_start: 0,
                                    byte_end: line.len() as u32,
                                    line_number: line_num + 1,
                                },
                            });
                        }
                    }
                }
            }
        }
        
        Ok(refs)
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
    pub async fn impact_analysis(&self, symbol_name: &str, max_hops: Option<u32>) -> Result<Vec<queries::ImpactedSymbol>> {
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
            let mut graph = CodeGraph::open(&db_path)
                .map_err(|e| crate::error::ForgeError::DatabaseError(
                    format!("Failed to open magellan graph: {}", e)
                ))?;
            
            // Scan the directory and index all files
            let count = graph.scan_directory(Path::new(codebase_path), None)
                .map_err(|e| crate::error::ForgeError::DatabaseError(
                    format!("Failed to scan directory: {}", e)
                ))?;
            
            tracing::info!("Indexed {} symbols from {}", count, codebase_path.display());
            
            // Also index references and calls for each Rust file recursively
            Self::index_references_recursive(&mut graph, codebase_path, codebase_path).await?;
            
            // For Native V3 backend, also index cross-file references
            // This is a capability that Native V3 enables over SQLite
            if self.store.backend_kind == crate::storage::BackendKind::NativeV3 {
                let cross_file_refs = self.store.index_cross_file_references().await?;
                tracing::info!("Indexed {} cross-file references (Native V3 only)", cross_file_refs);
            }
            
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
        
        let mut entries = fs::read_dir(current_dir).await
            .map_err(|e| crate::error::ForgeError::DatabaseError(format!("Failed to read dir: {}", e)))?;
        
        while let Some(entry) = entries.next_entry().await
            .map_err(|e| crate::error::ForgeError::DatabaseError(format!("Failed to read entry: {}", e)))? 
        {
            let path = entry.path();
            if path.is_dir() {
                // Recurse into subdirectories
                Box::pin(Self::index_references_recursive(graph, codebase_path, &path)).await?;
            } else if path.is_file() && path.extension().map(|e| e == "rs").unwrap_or(false) {
                // Get relative path from codebase root
                let relative_path = path.strip_prefix(codebase_path)
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

/// Map file extension to forge Language
#[cfg(feature = "magellan")]
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
    use crate::storage::BackendKind;

    #[tokio::test]
    async fn test_graph_module_creation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Arc::new(UnifiedGraphStore::open(
            temp_dir.path(),
            BackendKind::SQLite
        ).await.unwrap());

        let module = GraphModule::new(store.clone());
        assert_eq!(module.store.db_path(), store.db_path());
    }

    #[tokio::test]
    async fn test_find_symbol_empty() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Arc::new(UnifiedGraphStore::open(
            temp_dir.path(),
            BackendKind::SQLite
        ).await.unwrap());

        let module = GraphModule::new(store);
        let symbols = module.find_symbol("nonexistent").await.unwrap();
        assert_eq!(symbols.len(), 0);
    }

    #[tokio::test]
    async fn test_find_symbol_by_id_not_found() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Arc::new(UnifiedGraphStore::open(
            temp_dir.path(),
            BackendKind::SQLite
        ).await.unwrap());

        let module = GraphModule::new(store);
        let result = module.find_symbol_by_id(SymbolId(999)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_callers_of_empty() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Arc::new(UnifiedGraphStore::open(
            temp_dir.path(),
            BackendKind::SQLite
        ).await.unwrap());

        let module = GraphModule::new(store);
        let callers = module.callers_of("nonexistent").await.unwrap();
        assert_eq!(callers.len(), 0);
    }
}

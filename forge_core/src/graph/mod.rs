//! Graph module - Symbol and reference queries using sqlitegraph.
//!
//! This module provides access to code graph for querying symbols,
//! finding references, and running graph algorithms.

use crate::error::Result;
use crate::storage::UnifiedGraphStore;
use crate::types::{Cycle, CycleMember, Reference, ReferenceKind, Symbol, SymbolId};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

/// Impacted symbol from k-hop impact analysis.
#[derive(Debug, Clone)]
pub struct ImpactedSymbol {
    pub symbol_id: i64,
    pub name: String,
    pub kind: String,
    pub file_path: String,
    pub hop_distance: u32,
    pub edge_type: String,
}

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
            crate::error::ForgeError::DatabaseError(format!("Failed to open magellan graph: {}", e))
        })?;

        let results = graph.search_symbols_by_name(name).map_err(|e| {
            crate::error::ForgeError::DatabaseError(format!("Symbol search failed: {}", e))
        })?;

        Ok(results
            .into_iter()
            .map(|r| {
                let file_path = std::path::PathBuf::from(&r.file_path);
                let line_number = std::fs::read(&file_path)
                    .map(|content| byte_offset_to_line_number(&content, r.byte_start))
                    .unwrap_or(0);
                Symbol {
                    id: SymbolId(r.entity_id),
                    name: Arc::from(r.name.clone()),
                    fully_qualified_name: Arc::from(r.name.clone()),
                    kind: parse_symbol_kind_str(&r.kind),
                    language: map_magellan_language(&file_path),
                    location: crate::types::Location {
                        file_path,
                        byte_start: r.byte_start as u32,
                        byte_end: r.byte_end as u32,
                        line_number,
                    },
                    parent_id: None,
                    metadata: serde_json::Value::Null,
                }
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

    /// Finds all callers of a symbol by name.
    ///
    /// # Arguments
    ///
    /// * `name` - The symbol name
    ///
    /// # Returns
    ///
    /// A vector of call-references to this symbol, or empty if the graph DB does not exist.
    pub async fn callers_of(&self, name: &str) -> Result<Vec<Reference>> {
        use magellan::CodeGraph;

        let db_path = &self.store.db_path;
        if !db_path.exists() {
            return Ok(Vec::new());
        }

        let mut graph = CodeGraph::open(db_path).map_err(|e| {
            crate::error::ForgeError::DatabaseError(format!("Failed to open magellan graph: {}", e))
        })?;

        let symbols = graph.search_symbols_by_name(name).map_err(|e| {
            crate::error::ForgeError::DatabaseError(format!("Symbol search failed: {}", e))
        })?;

        let mut callers = Vec::new();
        for sym in &symbols {
            if let Ok(call_facts) = graph.callers_of_symbol(&sym.file_path, name) {
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

        Ok(callers)
    }

    /// Finds all cross-file references to a symbol by FQN.
    ///
    /// # Arguments
    ///
    /// * `name` - The symbol fully-qualified name
    ///
    /// # Returns
    ///
    /// A vector of all cross-file references, or empty if the graph DB does not exist.
    pub async fn references(&self, name: &str) -> Result<Vec<Reference>> {
        use magellan::{cross_file_references_to, CodeGraph};

        let db_path = &self.store.db_path;
        if !db_path.exists() {
            return Ok(Vec::new());
        }

        let graph = CodeGraph::open(db_path).map_err(|e| {
            crate::error::ForgeError::DatabaseError(format!("Failed to open magellan graph: {}", e))
        })?;

        let cross_refs = cross_file_references_to(&graph, name).map_err(|e| {
            crate::error::ForgeError::DatabaseError(format!("Reference query failed: {}", e))
        })?;

        Ok(cross_refs
            .into_iter()
            .map(|r| Reference {
                from: SymbolId(0),
                to: SymbolId(0),
                from_name: Some(r.from_symbol_id.clone()),
                to_name: Some(r.to_symbol_id.clone()),
                kind: ReferenceKind::TypeReference,
                location: crate::types::Location {
                    file_path: std::path::PathBuf::from(&r.file_path),
                    byte_start: r.byte_start as u32,
                    byte_end: r.byte_end as u32,
                    line_number: r.line_number,
                },
            })
            .collect())
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

    /// Detects cycles in the call graph using SCC condensation.
    ///
    /// Supernodes with more than one member represent strongly connected
    /// components (mutual recursion / call cycles).
    ///
    /// # Returns
    ///
    /// A vector of detected cycles, or empty if the graph DB does not exist.
    pub async fn cycles(&self) -> Result<Vec<Cycle>> {
        use magellan::CodeGraph;

        let db_path = &self.store.db_path;
        if !db_path.exists() {
            return Ok(Vec::new());
        }

        let graph = CodeGraph::open(db_path).map_err(|e| {
            crate::error::ForgeError::DatabaseError(format!("Failed to open magellan graph: {}", e))
        })?;

        let report = graph.detect_cycles().map_err(|e| {
            crate::error::ForgeError::DatabaseError(format!("Cycle detection failed: {}", e))
        })?;

        Ok(report
            .cycles
            .into_iter()
            .map(|c| Cycle {
                members: c
                    .members
                    .into_iter()
                    .map(|si| CycleMember {
                        symbol_id: si.symbol_id,
                        fqn: si.fqn,
                        file_path: si.file_path,
                        kind: si.kind,
                    })
                    .collect(),
            })
            .collect())
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
    /// A vector of impacted symbols with their hop distance from the target,
    /// or empty if the graph DB does not exist.
    pub async fn impact_analysis(
        &self,
        symbol_name: &str,
        max_hops: Option<u32>,
    ) -> Result<Vec<ImpactedSymbol>> {
        use magellan::CodeGraph;

        let db_path = &self.store.db_path;
        if !db_path.exists() {
            return Ok(Vec::new());
        }

        let mut graph = CodeGraph::open(db_path).map_err(|e| {
            crate::error::ForgeError::DatabaseError(format!("Failed to open magellan graph: {}", e))
        })?;

        let symbols = graph.search_symbols_by_name(symbol_name).map_err(|e| {
            crate::error::ForgeError::DatabaseError(format!("Symbol search failed: {}", e))
        })?;

        let start_entity_id = match symbols.first() {
            Some(s) => s.entity_id,
            None => return Ok(Vec::new()),
        };

        let hops = max_hops.unwrap_or(2);

        let mut impacted = Vec::new();
        let mut visited = HashSet::new();
        visited.insert(start_entity_id);
        let mut current_level = vec![start_entity_id];

        for hop in 1..=hops {
            let mut next_level = Vec::new();
            for &entity_id in &current_level {
                if let Ok(sym) = graph.symbol_by_entity_id(entity_id) {
                    let file = &sym.file_path;
                    if let Ok(callers) =
                        graph.callers_of_symbol(file, sym.fqn.as_deref().unwrap_or(&sym.kind))
                    {
                        for fact in callers {
                            let caller_entity = graph
                                .symbol_id_by_name(
                                    fact.file_path.to_str().unwrap_or(""),
                                    &fact.caller,
                                )
                                .ok()
                                .flatten();
                            if let Some(cid) = caller_entity {
                                if visited.insert(cid) {
                                    next_level.push(cid);
                                    if let Ok(info) = graph.symbol_by_entity_id(cid) {
                                        impacted.push(ImpactedSymbol {
                                            symbol_id: cid,
                                            name: info.fqn.clone().unwrap_or_default(),
                                            kind: info.kind,
                                            file_path: info.file_path,
                                            hop_distance: hop,
                                            edge_type: "call".to_string(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
            current_level = next_level;
            if current_level.is_empty() {
                break;
            }
        }

        Ok(impacted)
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
        use magellan::CodeGraph;
        use std::path::Path;

        let codebase_path = &self.store.codebase_path;
        let db_path = &self.store.db_path;

        let mut graph = CodeGraph::open(db_path).map_err(|e| {
            crate::error::ForgeError::DatabaseError(format!("Failed to open magellan graph: {}", e))
        })?;

        let count = graph
            .scan_directory(Path::new(codebase_path), None)
            .map_err(|e| {
                crate::error::ForgeError::DatabaseError(format!("Failed to scan directory: {}", e))
            })?;

        tracing::info!("Indexed {} symbols from {}", count, codebase_path.display());

        let _ = graph.rebuild_fts5();

        Self::index_references_recursive(&mut graph, codebase_path, codebase_path).await
    }

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

/// Returns the 1-indexed line number for a byte offset within file content.
///
/// Counts `\n` bytes before `byte_offset` and adds 1. If `byte_offset` exceeds
/// the content length, returns the last line number.
pub(crate) fn byte_offset_to_line_number(content: &[u8], byte_offset: usize) -> usize {
    let clamped = byte_offset.min(content.len());
    content[..clamped].iter().filter(|&&b| b == b'\n').count() + 1
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
        assert_eq!(
            module.store().db_path,
            temp_dir.path().join("test-graph.db")
        );
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

    #[test]
    fn test_byte_offset_to_line_number_first_line() {
        let content = b"fn foo() {}\nfn bar() {}\n";
        assert_eq!(byte_offset_to_line_number(content, 0), 1);
        assert_eq!(byte_offset_to_line_number(content, 5), 1);
    }

    #[test]
    fn test_byte_offset_to_line_number_second_line() {
        let content = b"fn foo() {}\nfn bar() {}\n";
        // byte 12 is start of "fn bar" (after the \n at byte 11)
        assert_eq!(byte_offset_to_line_number(content, 12), 2);
    }

    #[test]
    fn test_byte_offset_to_line_number_third_line() {
        let content = b"line1\nline2\nline3\n";
        assert_eq!(byte_offset_to_line_number(content, 12), 3);
    }

    #[test]
    fn test_byte_offset_to_line_number_clamps_to_end() {
        // Content without trailing newline: "abc\ndef" — 1 newline, so last line is 2
        let content = b"abc\ndef";
        assert_eq!(byte_offset_to_line_number(content, 9999), 2);
    }

    #[test]
    fn test_byte_offset_to_line_number_empty_content() {
        assert_eq!(byte_offset_to_line_number(b"", 0), 1);
    }

    #[tokio::test]
    async fn test_find_symbol_after_index() {
        let temp_dir = tempfile::tempdir().unwrap();
        let src_dir = temp_dir.path().join("src");
        tokio::fs::create_dir_all(&src_dir).await.unwrap();
        tokio::fs::write(
            src_dir.join("lib.rs"),
            "fn hello() {}\nfn world() -> i32 { 42 }\n",
        )
        .await
        .unwrap();

        let forge = test_forge(temp_dir.path()).await;
        forge.graph().index().await.unwrap();

        let symbols = forge.graph().find_symbol("hello").await.unwrap();
        assert!(!symbols.is_empty());
        assert_eq!(symbols[0].name.as_ref(), "hello");
    }

    #[tokio::test]
    async fn test_callers_of_after_index() {
        let temp_dir = tempfile::tempdir().unwrap();
        let src_dir = temp_dir.path().join("src");
        tokio::fs::create_dir_all(&src_dir).await.unwrap();
        tokio::fs::write(
            src_dir.join("lib.rs"),
            "fn helper() -> i32 { 1 }\nfn caller() -> i32 { helper() }\n",
        )
        .await
        .unwrap();

        let forge = test_forge(temp_dir.path()).await;
        forge.graph().index().await.unwrap();

        let callers = forge.graph().callers_of("helper").await.unwrap();
        assert!(!callers.is_empty(), "should find caller calling helper");
    }

    #[tokio::test]
    async fn test_cycles_detect_mutual_recursion() {
        let temp_dir = tempfile::tempdir().unwrap();
        let src_dir = temp_dir.path().join("src");
        tokio::fs::create_dir_all(&src_dir).await.unwrap();
        tokio::fs::write(src_dir.join("lib.rs"), "fn a() { b() }\nfn b() { a() }\n")
            .await
            .unwrap();

        let forge = test_forge(temp_dir.path()).await;
        forge.graph().index().await.unwrap();

        let cycles = forge.graph().cycles().await.unwrap();
        assert!(
            !cycles.is_empty(),
            "should detect mutual recursion between a and b"
        );
        let cycle = &cycles[0];
        assert!(cycle.members.len() >= 2);
        assert!(cycle.members.iter().any(|m| m.fqn.as_deref() == Some("a")));
        assert!(cycle.members.iter().any(|m| m.fqn.as_deref() == Some("b")));
    }

    #[tokio::test]
    async fn test_impact_analysis_after_index() {
        let temp_dir = tempfile::tempdir().unwrap();
        let src_dir = temp_dir.path().join("src");
        tokio::fs::create_dir_all(&src_dir).await.unwrap();
        tokio::fs::write(
            src_dir.join("lib.rs"),
            "fn base() -> i32 { 1 }\nfn mid() -> i32 { base() }\nfn top() -> i32 { mid() }\n",
        )
        .await
        .unwrap();

        let forge = test_forge(temp_dir.path()).await;
        forge.graph().index().await.unwrap();

        let impacted = forge
            .graph()
            .impact_analysis("base", Some(2))
            .await
            .unwrap();
        assert!(
            !impacted.is_empty(),
            "base should have mid and/or top as impacted"
        );
        let has_correct_hop = impacted.iter().any(|s| s.hop_distance == 1);
        assert!(has_correct_hop, "at least one symbol should be at hop 1");
    }
}

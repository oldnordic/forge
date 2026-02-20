//! Dead code detection analysis
//!
//! Finds symbols that are defined but never referenced/called.

use crate::error::{ForgeError, Result};
use crate::types::Symbol;
use std::path::Path;

/// Dead code analyzer
pub struct DeadCodeAnalyzer<'a> {
    db_path: &'a Path,
}

impl<'a> DeadCodeAnalyzer<'a> {
    /// Create a new dead code analyzer
    pub fn new(db_path: &'a Path) -> Self {
        Self { db_path }
    }

    /// Find all dead code (symbols with no references)
    pub fn find_dead_code(&self) -> Result<Vec<DeadSymbol>> {
        use sqlitegraph::{open_graph, GraphConfig, snapshot::SnapshotId};
        
        let config = GraphConfig::sqlite();
        let backend = open_graph(self.db_path, &config)
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to open graph: {}", e)))?;
        
        let snapshot = SnapshotId::current();
        let mut dead_symbols = Vec::new();
        
        let entity_ids = backend.entity_ids()
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to list entities: {}", e)))?;
        
        for id in entity_ids {
            if let Ok(node) = backend.get_node(snapshot, id) {
                if !is_function_kind(&node.kind) {
                    continue;
                }
                
                if is_test_or_entry_point(&node.name) {
                    continue;
                }
                
                let incoming = backend.fetch_incoming(id)
                    .map_err(|e| ForgeError::DatabaseError(format!("Query failed: {}", e)))?;
                
                if incoming.is_empty() {
                    let is_public = node.data.to_string().contains("\"public\":true") 
                        || node.data.to_string().contains("\"visibility\":\"public\"");
                    
                    if !is_public {
                        dead_symbols.push(DeadSymbol {
                            id,
                            kind: node.kind,
                            name: node.name,
                            file_path: node.file_path.unwrap_or_default(),
                            is_public,
                            reason: "No references found".to_string(),
                        });
                    }
                }
            }
        }
        
        Ok(dead_symbols)
    }
}

fn is_function_kind(kind: &str) -> bool {
    matches!(kind, "fn" | "function" | "method" | "const" | "static")
}

fn is_test_or_entry_point(name: &str) -> bool {
    name.starts_with("test_") 
        || name.ends_with("_test")
        || matches!(name, "main" | "lib" | "init" | "setup" | "teardown")
}

/// Dead symbol information
#[derive(Debug, Clone)]
pub struct DeadSymbol {
    pub id: i64,
    pub kind: String,
    pub name: String,
    pub file_path: String,
    pub is_public: bool,
    pub reason: String,
}

impl From<DeadSymbol> for Symbol {
    fn from(dead: DeadSymbol) -> Self {
        use crate::types::{SymbolId, Language, Location};
        
        Symbol {
            id: SymbolId(dead.id),
            name: dead.name.clone(),
            fully_qualified_name: dead.name,
            kind: parse_symbol_kind(&dead.kind),
            language: Language::Rust,
            location: Location {
                file_path: std::path::PathBuf::from(&dead.file_path),
                byte_start: 0,
                byte_end: 0,
                line_number: 0,
            },
            parent_id: None,
            metadata: serde_json::json!({
                "dead_code": true,
                "reason": dead.reason,
                "is_public": dead.is_public,
            }),
        }
    }
}

fn parse_symbol_kind(kind: &str) -> crate::types::SymbolKind {
    match kind {
        "fn" | "function" | "method" => crate::types::SymbolKind::Function,
        "struct" => crate::types::SymbolKind::Struct,
        "enum" => crate::types::SymbolKind::Enum,
        "trait" => crate::types::SymbolKind::Trait,
        "impl" => crate::types::SymbolKind::Impl,
        "const" => crate::types::SymbolKind::Constant,
        "static" => crate::types::SymbolKind::Static,
        "module" | "mod" => crate::types::SymbolKind::Module,
        "type" => crate::types::SymbolKind::TypeAlias,
        _ => crate::types::SymbolKind::LocalVariable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_analyzer_creation() {
        let temp = tempdir().unwrap();
        let db_path = temp.path().join("test.db");
        
        let analyzer = DeadCodeAnalyzer::new(&db_path);
        // Just verify it creates without error
        assert!(analyzer.db_path.exists() == false); // DB doesn't exist yet
    }
}

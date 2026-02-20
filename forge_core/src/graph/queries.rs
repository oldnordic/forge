//! Graph queries for caller and reference resolution using sqlitegraph
//!
//! This module implements graph traversal using sqlitegraph's high-level API.

use crate::error::{ForgeError, Result};
use crate::types::{Reference, SymbolId, ReferenceKind, Location};
use std::path::Path;

/// Graph query engine using sqlitegraph
pub struct GraphQueryEngine {
    db_path: std::path::PathBuf,
}

impl GraphQueryEngine {
    /// Create a new query engine for the given database path
    pub fn new(db_path: &Path) -> Self {
        Self {
            db_path: db_path.to_path_buf(),
        }
    }

    /// Find all callers of a symbol by name
    pub fn find_callers(&self, symbol_name: &str) -> Result<Vec<Reference>> {
        use sqlitegraph::{open_graph, GraphConfig, snapshot::SnapshotId};
        
        let config = GraphConfig::sqlite();
        let backend = open_graph(&self.db_path, &config)
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to open graph: {}", e)))?;
        
        let target_id = match self.find_symbol_id(&*backend, symbol_name)? {
            Some(id) => id,
            None => return Ok(Vec::new()),
        };
        
        let snapshot = SnapshotId::current();
        let caller_ids = backend.fetch_incoming(target_id)
            .map_err(|e| ForgeError::DatabaseError(format!("Query failed: {}", e)))?;
        
        let mut refs = Vec::new();
        for caller_id in caller_ids {
            if let Ok(node) = backend.get_node(snapshot, caller_id) {
                refs.push(Reference {
                    from: SymbolId(caller_id),
                    to: SymbolId(target_id),
                    kind: ReferenceKind::Call,
                    location: Location {
                        file_path: std::path::PathBuf::from(node.file_path.unwrap_or_default()),
                        byte_start: 0,
                        byte_end: 0,
                        line_number: 0,
                    },
                });
            }
        }
        
        Ok(refs)
    }

    /// Find all references to a symbol
    pub fn find_references(&self, symbol_name: &str) -> Result<Vec<Reference>> {
        use sqlitegraph::{open_graph, GraphConfig, snapshot::SnapshotId};
        
        let config = GraphConfig::sqlite();
        let backend = open_graph(&self.db_path, &config)
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to open graph: {}", e)))?;
        
        let target_id = match self.find_symbol_id(&*backend, symbol_name)? {
            Some(id) => id,
            None => return Ok(Vec::new()),
        };
        
        let snapshot = SnapshotId::current();
        let neighbor_ids = backend.fetch_incoming(target_id)
            .map_err(|e| ForgeError::DatabaseError(format!("Query failed: {}", e)))?;
        
        let mut refs = Vec::new();
        for neighbor_id in neighbor_ids {
            if let Ok(node) = backend.get_node(snapshot, neighbor_id) {
                refs.push(Reference {
                    from: SymbolId(neighbor_id),
                    to: SymbolId(target_id),
                    kind: ReferenceKind::TypeReference,
                    location: Location {
                        file_path: std::path::PathBuf::from(node.file_path.unwrap_or_default()),
                        byte_start: 0,
                        byte_end: 0,
                        line_number: 0,
                    },
                });
            }
        }
        
        Ok(refs)
    }

    /// Find symbol ID by name
    fn find_symbol_id(&self, backend: &dyn sqlitegraph::GraphBackend, symbol_name: &str) -> Result<Option<i64>> {
        use sqlitegraph::snapshot::SnapshotId;
        
        let snapshot = SnapshotId::current();
        let ids = backend.entity_ids()
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to list entities: {}", e)))?;
        
        for id in ids {
            if let Ok(node) = backend.get_node(snapshot, id) {
                if node.name == symbol_name {
                    return Ok(Some(id));
                }
            }
        }
        
        Ok(None)
    }

    /// K-hop traversal to find impacted symbols
    pub fn find_impacted_symbols(
        &self, 
        start_symbol: &str, 
        max_hops: u32
    ) -> Result<Vec<ImpactedSymbol>> {
        use sqlitegraph::{open_graph, GraphConfig, snapshot::SnapshotId, backend::BackendDirection};
        
        let config = GraphConfig::sqlite();
        let backend = open_graph(&self.db_path, &config)
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to open graph: {}", e)))?;
        
        let start_id = match self.find_symbol_id(&*backend, start_symbol)? {
            Some(id) => id,
            None => return Ok(Vec::new()),
        };
        
        let snapshot = SnapshotId::current();
        let impacted_ids = backend.k_hop(snapshot, start_id, max_hops, BackendDirection::Outgoing)
            .map_err(|e| ForgeError::DatabaseError(format!("k-hop query failed: {}", e)))?;
        
        let mut results = Vec::new();
        for id in impacted_ids {
            if id == start_id {
                continue;
            }
            
            if let Ok(node) = backend.get_node(snapshot, id) {
                results.push(ImpactedSymbol {
                    symbol_id: id,
                    name: node.name,
                    kind: node.kind,
                    file_path: node.file_path.unwrap_or_default(),
                    hop_distance: 1,
                    edge_type: "transitive".to_string(),
                });
            }
        }
        
        Ok(results)
    }
}

/// Impacted symbol from k-hop analysis
#[derive(Debug, Clone)]
pub struct ImpactedSymbol {
    pub symbol_id: i64,
    pub name: String,
    pub kind: String,
    pub file_path: String,
    pub hop_distance: u32,
    pub edge_type: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    // Note: Tests require a properly initialized sqlitegraph database
    // which has a specific schema. We test the API surface here.
    
    #[test]
    fn test_query_engine_creation() {
        let temp = tempdir().unwrap();
        let db_path = temp.path().join("test.db");
        
        let engine = GraphQueryEngine::new(&db_path);
        assert_eq!(engine.db_path, db_path);
    }
}

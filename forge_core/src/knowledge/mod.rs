//! Knowledge graph — sqlitegraph native-v3 backed graph for code intelligence.

pub mod types;

pub use types::*;

use std::path::{Path, PathBuf};

use crate::error::{ForgeError, Result};
use sqlitegraph::backend::GraphBackend;
use sqlitegraph::config::{open_graph, GraphConfig};

/// Knowledge graph backed by sqlitegraph native-v3.
///
/// Stores typed nodes (symbols, files, discoveries, patterns, issues,
/// CFG blocks, hotspots, knowledge entries) and typed edges (calls,
/// correlates, affects, flows_to, etc.) in a `.graph` binary file.
pub struct KnowledgeGraph {
    backend: Box<dyn GraphBackend>,
    graph_path: PathBuf,
    db_path: PathBuf,
}

impl KnowledgeGraph {
    /// Opens or creates a knowledge graph.
    ///
    /// The `.graph` file is created at `graph_path` using sqlitegraph
    /// native-v3 backend. The `db_path` points to the Magellan `.db`
    /// for FTS5 bridge lookups.
    pub fn open(graph_path: &Path, db_path: &Path) -> Result<Self> {
        if let Some(parent) = graph_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ForgeError::DatabaseError(format!(
                    "Failed to create graph directory: {}",
                    e
                ))
            })?;
        }

        let config = GraphConfig::native();
        let backend = open_graph(graph_path, &config).map_err(|e| {
            ForgeError::DatabaseError(format!("Failed to open knowledge graph: {}", e))
        })?;

        Ok(Self {
            backend,
            graph_path: graph_path.to_path_buf(),
            db_path: db_path.to_path_buf(),
        })
    }

    /// Returns the path to the .graph file.
    pub fn graph_path(&self) -> &Path {
        &self.graph_path
    }

    /// Returns the path to the Magellan .db.
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_knowledge_graph_open_creates_file() {
        let temp = tempfile::tempdir().unwrap();
        let graph_path = temp.path().join("knowledge.graph");
        let db_path = temp.path().join("magellan.db");

        let kg = KnowledgeGraph::open(&graph_path, &db_path).unwrap();

        assert!(graph_path.exists());
        assert_eq!(kg.graph_path(), graph_path);
        assert_eq!(kg.db_path(), db_path);
    }

    #[test]
    fn test_knowledge_graph_open_creates_parent_dirs() {
        let temp = tempfile::tempdir().unwrap();
        let graph_path = temp.path().join("nested").join("dir").join("kg.graph");
        let db_path = temp.path().join("magellan.db");

        let kg = KnowledgeGraph::open(&graph_path, &db_path).unwrap();
        assert!(graph_path.exists());
    }

    #[test]
    fn test_knowledge_graph_open_existing_file() {
        let temp = tempfile::tempdir().unwrap();
        let graph_path = temp.path().join("kg.graph");
        let db_path = temp.path().join("magellan.db");

        {
            let _kg = KnowledgeGraph::open(&graph_path, &db_path).unwrap();
        }

        let _kg2 = KnowledgeGraph::open(&graph_path, &db_path).unwrap();
        assert!(graph_path.exists());
    }
}

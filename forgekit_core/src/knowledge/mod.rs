//! Knowledge graph — sqlitegraph native-v3 backed graph for code intelligence.

pub mod nodes;
pub mod sync;
pub mod traversal;
pub mod types;

pub use nodes::SourceSpan;
pub use types::*;

use std::path::{Path, PathBuf};

use crate::error::{ForgeError, Result};
use sqlitegraph::backend::GraphBackend;
use sqlitegraph::config::{open_graph, GraphConfig};

pub struct KnowledgeGraph {
    pub(crate) backend: Box<dyn GraphBackend>,
    pub(crate) graph_path: PathBuf,
    pub(crate) db_path: PathBuf,
}

impl KnowledgeGraph {
    pub fn open(graph_path: &Path, db_path: &Path) -> Result<Self> {
        if let Some(parent) = graph_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ForgeError::DatabaseError(format!("Failed to create graph directory: {}", e))
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

    pub fn graph_path(&self) -> &Path {
        &self.graph_path
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub(crate) fn snapshot() -> sqlitegraph::snapshot::SnapshotId {
        sqlitegraph::snapshot::SnapshotId::current()
    }

    pub(crate) fn insert_node(
        &self,
        kind: &str,
        name: &str,
        file_path: Option<&str>,
        data: serde_json::Value,
    ) -> Result<i64> {
        let spec = sqlitegraph::backend::NodeSpec {
            kind: kind.to_string(),
            name: name.to_string(),
            file_path: file_path.map(|s| s.to_string()),
            data,
        };
        self.backend
            .insert_node(spec)
            .map_err(|e| ForgeError::DatabaseError(format!("Insert node failed: {}", e)))
    }
}

#[cfg(test)]
pub(crate) fn open_kg() -> (tempfile::TempDir, KnowledgeGraph) {
    let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");
    let kg = KnowledgeGraph::open(
        &temp.path().join("kg.graph"),
        &temp.path().join("magellan.db"),
    )
    .expect("invariant: fresh temp paths always open");
    (temp, kg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_knowledge_graph_open_creates_file() {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");
        let graph_path = temp.path().join("knowledge.graph");
        let db_path = temp.path().join("magellan.db");

        let kg = KnowledgeGraph::open(&graph_path, &db_path)
            .expect("invariant: fresh temp paths always open");

        assert!(graph_path.exists());
        assert_eq!(kg.graph_path(), graph_path);
        assert_eq!(kg.db_path(), db_path);
    }

    #[test]
    fn test_knowledge_graph_open_creates_parent_dirs() {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");
        let graph_path = temp.path().join("nested").join("dir").join("kg.graph");
        let db_path = temp.path().join("magellan.db");

        let _kg = KnowledgeGraph::open(&graph_path, &db_path)
            .expect("invariant: fresh temp paths always open");
        assert!(graph_path.exists());
    }

    #[test]
    fn test_knowledge_graph_open_existing_file() {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");
        let graph_path = temp.path().join("kg.graph");
        let db_path = temp.path().join("magellan.db");

        {
            let _kg = KnowledgeGraph::open(&graph_path, &db_path)
                .expect("invariant: fresh temp paths always open");
        }

        let _kg2 = KnowledgeGraph::open(&graph_path, &db_path)
            .expect("invariant: fresh temp paths always open");
        assert!(graph_path.exists());
    }
}

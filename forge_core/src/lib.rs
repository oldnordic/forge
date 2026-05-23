//! ForgeKit - Deterministic Code Intelligence SDK
//!
//! This crate provides core SDK for programmatic code intelligence operations.
//!
//! # Overview
//!
//! ForgeKit unifies several code intelligence tools into a single API:
//!
//! - **Graph Module**: Symbol and reference queries (native implementation)
//! - **Search Module**: Semantic code search (via LLMGrep)
//! - **CFG Module**: Control flow analysis (via Mirage)
//! - **Edit Module**: Span-safe code editing (via Splice)
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use forge_core::Forge;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let forge = Forge::open("./my-project").await?;
//!
//!     // Query code graph
//!     let symbols = forge.graph().find_symbol("main").await?;
//!     println!("Found: {:?}", symbols);
//!
//!     Ok(())
//! }
//! ```

// Core modules
pub mod error;
pub mod types;

use std::sync::Arc;

// Public API modules
pub mod analysis;
pub mod cfg;
pub mod edit;
pub mod graph;
pub mod search;
pub mod storage;
pub mod treesitter;

// Knowledge graph module (sqlitegraph native-v3)
pub mod knowledge;

// Runtime layer modules
pub mod cache;
pub mod indexing;
pub mod pool;
pub mod runtime;
pub mod watcher;

// Re-export sqlitegraph types for advanced usage
pub use sqlitegraph::backend::{EdgeSpec, NodeSpec};
pub use sqlitegraph::config::{open_graph, BackendKind as SqliteGraphBackendKind, GraphConfig};
pub use sqlitegraph::graph::{GraphEntity, SqliteGraph};

// Re-export commonly used types
pub use error::{ForgeError, Result};
pub use storage::{BackendKind, UnifiedGraphStore};
pub use types::{Location, SymbolId};

// Re-export runtime module types
pub use cache::QueryCache;
pub use indexing::{FlushStats, IncrementalIndexer, PathFilter};
pub use pool::{ConnectionPermit, ConnectionPool};
pub use runtime::Runtime;
pub use watcher::{WatchEvent, Watcher};

use anyhow::anyhow;

/// Main entry point for ForgeKit SDK.
///
/// The `Forge` type provides access to all code intelligence modules.

#[derive(Clone, Debug)]
pub struct Forge {
    store: std::sync::Arc<UnifiedGraphStore>,
}

impl Forge {
    /// Opens a Forge instance on given codebase path.
    ///
    /// This will create or open a graph database at `.forge/graph.db`
    /// within the codebase directory. Uses SQLite backend by default.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to codebase directory
    ///
    /// # Returns
    ///
    /// A `Forge` instance or an error if database cannot be opened.
    pub async fn open(path: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        Self::open_with_backend(path, BackendKind::default()).await
    }

    /// Opens a Forge instance with a specific backend.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to codebase directory
    /// * `backend` - Backend kind (SQLite or Native V3)
    ///
    /// # Returns
    ///
    /// A `Forge` instance or an error if database cannot be opened.
    pub async fn open_with_backend(
        path: impl AsRef<std::path::Path>,
        backend: BackendKind,
    ) -> anyhow::Result<Self> {
        let store = std::sync::Arc::new(storage::UnifiedGraphStore::open(path, backend).await?);
        let forge = Forge { store };

        #[cfg(feature = "magellan")]
        {
            if forge.store.needs_indexing() {
                tracing::info!("Graph empty — auto-indexing codebase with magellan");
                if let Err(e) = forge.graph().index().await {
                    tracing::warn!("Auto-indexing failed: {}", e);
                }
            }
        }

        Ok(forge)
    }

    /// Returns the backend kind currently in use.
    pub fn backend_kind(&self) -> BackendKind {
        self.store.backend_kind()
    }

    /// Returns the graph module for symbol and reference queries.
    pub fn graph(&self) -> graph::GraphModule {
        graph::GraphModule::new(Arc::clone(&self.store))
    }

    /// Returns the search module for semantic code queries.
    pub fn search(&self) -> search::SearchModule {
        search::SearchModule::new(Arc::clone(&self.store))
    }

    /// Returns the CFG module for control flow analysis.
    pub fn cfg(&self) -> cfg::CfgModule {
        cfg::CfgModule::new(Arc::clone(&self.store))
    }

    /// Returns the edit module for span-safe refactoring.
    pub fn edit(&self) -> edit::EditModule {
        edit::EditModule::new(Arc::clone(&self.store))
    }

    /// Returns the analysis module for combined operations.
    pub fn analysis(&self) -> analysis::AnalysisModule {
        analysis::AnalysisModule::new(self.graph(), self.cfg(), self.edit(), self.search())
    }

    /// Returns the codebase path.
    pub fn codebase_path(&self) -> &std::path::Path {
        &self.store.codebase_path
    }

    /// Returns the knowledge graph module.
    ///
    /// Opens or creates the `.magellan/knowledge.graph` file using
    /// sqlitegraph native-v3 backend.
    #[cfg(feature = "native-v3")]
    pub fn knowledge(&self) -> anyhow::Result<knowledge::KnowledgeGraph> {
        let graph_path = self
            .store
            .codebase_path
            .join(".magellan")
            .join("knowledge.graph");
        let db_path = self.store.db_path.clone();

        if let Some(parent) = graph_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        knowledge::KnowledgeGraph::open(&graph_path, &db_path)
            .map_err(|e| anyhow!("Failed to open knowledge graph: {}", e))
    }
}

/// Builder for configuring and creating a Forge instance.
#[derive(Clone, Default)]
pub struct ForgeBuilder {
    path: Option<std::path::PathBuf>,
    backend_kind: Option<BackendKind>,
    db_path: Option<std::path::PathBuf>,
    db_dir: Option<std::path::PathBuf>,
}

impl ForgeBuilder {
    /// Creates a new builder with default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the path to the codebase.
    pub fn path(self, path: impl AsRef<std::path::Path>) -> Self {
        Self {
            path: Some(path.as_ref().to_path_buf()),
            ..self
        }
    }

    /// Sets the backend kind (SQLite or Native V3).
    pub fn backend_kind(self, kind: BackendKind) -> Self {
        Self {
            backend_kind: Some(kind),
            ..self
        }
    }

    /// Sets an explicit database path, overriding the default ~/.magellan/<stem>.db.
    pub fn db_path(self, path: std::path::PathBuf) -> Self {
        Self {
            db_path: Some(path),
            ..self
        }
    }

    /// Sets the database directory; stem is still derived from the project root.
    pub fn db_dir(self, dir: std::path::PathBuf) -> Self {
        Self {
            db_dir: Some(dir),
            ..self
        }
    }

    /// Builds a `Forge` instance with configured options.
    pub async fn build(self) -> anyhow::Result<Forge> {
        let path = self.path.ok_or_else(|| anyhow!("path is required"))?;
        let backend = self.backend_kind.unwrap_or_default();

        let resolved_db = if let Some(explicit) = self.db_path {
            explicit
        } else if let Some(dir) = self.db_dir {
            let stem = path.file_name().and_then(|n| n.to_str()).unwrap_or("graph");
            dir.join(format!("{}.db", stem))
        } else {
            storage::default_db_path(&path)
        };

        let store = std::sync::Arc::new(
            storage::UnifiedGraphStore::open_with_path(&path, &resolved_db, backend).await?,
        );

        Ok(Forge { store })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Forge Creation Tests

    #[tokio::test]
    async fn test_forge_open_creates_database() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test-graph.db");

        // Verify database doesn't exist initially
        assert!(!db_path.exists());

        // Open Forge with explicit db_path — never writes to ~/.magellan/ in tests
        let forge = ForgeBuilder::new()
            .path(temp_dir.path())
            .db_path(db_path.clone())
            .build()
            .await
            .unwrap();

        // Verify database was created
        assert!(db_path.exists());

        // Verify Forge instance is valid
        let _graph = forge.graph();
        let _search = forge.search();

        drop(forge);
    }

    // Module Accessor Tests

    #[tokio::test]
    async fn test_forge_graph_accessor() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            storage::UnifiedGraphStore::open_with_path(
                temp_dir.path(),
                temp_dir.path().join("test-graph.db"),
                BackendKind::default(),
            )
            .await
            .unwrap(),
        );

        let forge = Forge { store };

        // Graph accessor should return GraphModule
        let graph = forge.graph();
        drop(graph);
    }

    #[tokio::test]
    async fn test_forge_search_accessor() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            storage::UnifiedGraphStore::open_with_path(
                temp_dir.path(),
                temp_dir.path().join("test-graph.db"),
                BackendKind::default(),
            )
            .await
            .unwrap(),
        );

        let forge = Forge { store };

        // Search accessor should return SearchModule
        let search = forge.search();
        drop(search);
    }

    #[tokio::test]
    async fn test_forge_cfg_accessor() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            storage::UnifiedGraphStore::open_with_path(
                temp_dir.path(),
                temp_dir.path().join("test-graph.db"),
                BackendKind::default(),
            )
            .await
            .unwrap(),
        );

        let forge = Forge { store };

        // CFG accessor should return CfgModule
        let cfg = forge.cfg();
        drop(cfg);
    }

    #[tokio::test]
    async fn test_forge_edit_accessor() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            storage::UnifiedGraphStore::open_with_path(
                temp_dir.path(),
                temp_dir.path().join("test-graph.db"),
                BackendKind::default(),
            )
            .await
            .unwrap(),
        );

        let forge = Forge { store };

        // Edit accessor should return EditModule
        let edit = forge.edit();
        drop(edit);
    }

    #[tokio::test]
    async fn test_forge_analysis_accessor() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            storage::UnifiedGraphStore::open_with_path(
                temp_dir.path(),
                temp_dir.path().join("test-graph.db"),
                BackendKind::default(),
            )
            .await
            .unwrap(),
        );

        let forge = Forge { store };

        // Analysis accessor should return AnalysisModule
        let analysis = forge.analysis();
        drop(analysis);
    }

    // ForgeBuilder Tests

    #[test]
    fn test_forge_builder_default() {
        let builder = ForgeBuilder::new();

        // Default builder should have None for all fields
        assert!(builder.path.is_none());
        assert!(builder.backend_kind.is_none());
    }

    #[test]
    fn test_forge_builder_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("test");
        let builder = ForgeBuilder::new().path(&path);

        assert_eq!(builder.path, Some(path));
    }

    #[test]
    fn test_forge_builder_backend_kind() {
        let builder = ForgeBuilder::new().backend_kind(BackendKind::NativeV3);

        assert_eq!(builder.backend_kind, Some(BackendKind::NativeV3));
    }

    #[tokio::test]
    async fn test_forge_builder_build_success() {
        let temp_dir = tempfile::tempdir().unwrap();
        let builder = ForgeBuilder::new()
            .path(temp_dir.path())
            .db_path(temp_dir.path().join("test.db"))
            .backend_kind(BackendKind::SQLite);

        let forge = builder.build().await.unwrap();

        assert!(forge.store.is_connected());
    }

    #[tokio::test]
    async fn test_forge_builder_missing_path() {
        let builder = ForgeBuilder::new();

        let result = builder.build().await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("path"));
    }

    #[cfg(feature = "native-v3")]
    #[tokio::test]
    async fn test_forge_knowledge_accessor() {
        let temp_dir = tempfile::tempdir().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();

        let kg = forge.knowledge();
        assert!(kg.is_ok());

        let kg = kg.unwrap();
        assert!(kg.graph_path().exists());
    }

    #[tokio::test]
    async fn test_forge_builder_db_path_override() {
        let temp_dir = tempfile::tempdir().unwrap();
        let custom_db = temp_dir.path().join("custom.db");

        let forge = ForgeBuilder::new()
            .path(temp_dir.path())
            .db_path(custom_db.clone())
            .build()
            .await
            .unwrap();

        assert_eq!(forge.store.db_path, custom_db);
    }

    #[tokio::test]
    async fn test_forge_builder_db_dir_override() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_dir = temp_dir.path().join("custom_dir");
        std::fs::create_dir_all(&db_dir).unwrap();

        let project_dir = temp_dir.path().join("my-project");
        std::fs::create_dir_all(&project_dir).unwrap();

        let forge = ForgeBuilder::new()
            .path(&project_dir)
            .db_dir(db_dir.clone())
            .build()
            .await
            .unwrap();

        assert_eq!(forge.store.db_path, db_dir.join("my-project.db"));
    }

    #[tokio::test]
    async fn test_connection_pool_exported() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let pool = crate::ConnectionPool::new(&db_path, 4);
        assert_eq!(pool.available_connections(), 4);
    }

    #[tokio::test]
    async fn test_runtime_exported() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let rt = crate::Runtime::new(dir.path().to_path_buf()).await.unwrap();
        assert!(!rt.is_watching());
    }
}

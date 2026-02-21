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

// Public API modules
pub mod storage;
pub mod graph;
pub mod search;
pub mod cfg;
pub mod edit;
pub mod analysis;
pub mod treesitter;

// Runtime layer modules (Phase 2)
// TODO: Re-enable when dependencies are available
// pub mod watcher;
// pub mod indexing;
// pub mod cache;
// pub mod pool;
// pub mod runtime;

// Re-export sqlitegraph types for advanced usage
pub use sqlitegraph::backend::{NodeSpec, EdgeSpec};
pub use sqlitegraph::graph::{GraphEntity, SqliteGraph};
pub use sqlitegraph::config::{BackendKind as SqliteGraphBackendKind, GraphConfig, open_graph};

// Re-export commonly used types
pub use error::{ForgeError, Result};
pub use types::{SymbolId, Location};
pub use storage::{BackendKind, UnifiedGraphStore};

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
        backend: BackendKind
    ) -> anyhow::Result<Self> {
        let store = std::sync::Arc::new(
            storage::UnifiedGraphStore::open(path, backend).await?
        );
        Ok(Forge { store })
    }
    
    /// Returns the backend kind currently in use.
    pub fn backend_kind(&self) -> BackendKind {
        self.store.backend_kind()
    }

    /// Returns the graph module for symbol and reference queries.
    pub fn graph(&self) -> graph::GraphModule {
        graph::GraphModule::new(self.store.clone())
    }

    /// Returns the search module for semantic code queries.
    pub fn search(&self) -> search::SearchModule {
        search::SearchModule::new(self.store.clone())
    }

    /// Returns the CFG module for control flow analysis.
    pub fn cfg(&self) -> cfg::CfgModule {
        cfg::CfgModule::new(self.store.clone())
    }

    /// Returns the edit module for span-safe refactoring.
    pub fn edit(&self) -> edit::EditModule {
        edit::EditModule::new(self.store.clone())
    }

    /// Returns the analysis module for combined operations.
    pub fn analysis(&self) -> analysis::AnalysisModule {
        analysis::AnalysisModule::new(
            self.graph(),
            self.cfg(),
            self.edit(),
            self.search(),
        )
    }
}

/// Builder for configuring and creating a Forge instance.
#[derive(Clone, Default)]
pub struct ForgeBuilder {
    path: Option<std::path::PathBuf>,
    database_path: Option<std::path::PathBuf>,
    backend_kind: Option<BackendKind>,
    cache_ttl: Option<std::time::Duration>,
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

    /// Sets a custom path for the graph database file.
    pub fn database_path(self, db_path: impl AsRef<std::path::Path>) -> Self {
        Self {
            database_path: Some(db_path.as_ref().to_path_buf()),
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

    /// Sets the cache TTL for query results.
    pub fn cache_ttl(self, ttl: std::time::Duration) -> Self {
        Self {
            cache_ttl: Some(ttl),
            ..self
        }
    }

    /// Builds a `Forge` instance with configured options.
    pub async fn build(self) -> anyhow::Result<Forge> {
        let path = self.path
            .ok_or_else(|| anyhow!("path is required"))?;

        let store = std::sync::Arc::new(storage::UnifiedGraphStore::open(
                &path,
                self.backend_kind.unwrap_or_default()
            ).await?);

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
        let db_path = temp_dir.path().join(".forge").join("graph.db");

        // Verify database doesn't exist initially
        assert!(!db_path.exists());

        // Open Forge - this creates database directory and file
        let forge = Forge::open(temp_dir.path()).await.unwrap();

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
        let store = std::sync::Arc::new(storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::default()).await.unwrap());

        let forge = Forge { store };

        // Graph accessor should return GraphModule
        let graph = forge.graph();
        drop(graph);
    }

    #[tokio::test]
    async fn test_forge_search_accessor() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::default()).await.unwrap());

        let forge = Forge { store };

        // Search accessor should return SearchModule
        let search = forge.search();
        drop(search);
    }

    #[tokio::test]
    async fn test_forge_cfg_accessor() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::default()).await.unwrap());

        let forge = Forge { store };

        // CFG accessor should return CfgModule
        let cfg = forge.cfg();
        drop(cfg);
    }

    #[tokio::test]
    async fn test_forge_edit_accessor() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::default()).await.unwrap());

        let forge = Forge { store };

        // Edit accessor should return EditModule
        let edit = forge.edit();
        drop(edit);
    }

    #[tokio::test]
    async fn test_forge_analysis_accessor() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::default()).await.unwrap());

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
        assert!(builder.database_path.is_none());
        assert!(builder.backend_kind.is_none());
        assert!(builder.cache_ttl.is_none());
    }

    #[test]
    fn test_forge_builder_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("test");
        let builder = ForgeBuilder::new().path(&path);

        assert_eq!(builder.path, Some(std::path::PathBuf::from(path)));
    }

    #[test]
    fn test_forge_builder_database_path() {
        let builder = ForgeBuilder::new().database_path("custom.db");

        assert_eq!(builder.database_path, Some(std::path::PathBuf::from("custom.db")));
    }

    #[test]
    fn test_forge_builder_backend_kind() {
        let builder = ForgeBuilder::new().backend_kind(BackendKind::NativeV3);

        assert_eq!(builder.backend_kind, Some(BackendKind::NativeV3));
    }

    #[test]
    fn test_forge_builder_cache_ttl() {
        let ttl = std::time::Duration::from_secs(60);
        let builder = ForgeBuilder::new().cache_ttl(ttl);

        assert_eq!(builder.cache_ttl, Some(ttl));
    }

    #[tokio::test]
    async fn test_forge_builder_build_success() {
        let temp_dir = tempfile::tempdir().unwrap();
        let builder = ForgeBuilder::new()
            .path(temp_dir.path())
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
}

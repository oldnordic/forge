//! ForgeKit - Deterministic Code Intelligence SDK
//!
//! This crate provides the core SDK for programmatic code intelligence operations.
//!
//! # Overview
//!
//! ForgeKit unifies several code intelligence tools into a single API:
//!
//! - **Graph Module**: Symbol and reference queries (via Magellan)
//! - **Search Module**: Semantic code search (via LLMGrep)
//! - **CFG Module**: Control flow analysis (via Mirage)
//! - **Edit Module**: Span-safe refactoring (via Splice)
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
//!     // Query the code graph
//!     let symbols = forge.graph().find_symbol("main").await?;
//!     println!("Found: {:?}", symbols);
//!
//!     Ok(())
//! }
//! ```
//!
//! # Modules
//!
//! - [`types`]: Core types used across the SDK
//! - [`error`]: Error types for all operations
//! - [`storage`]: Storage abstraction layer
//! - [`graph`]: Symbol and reference queries
//! - [`search`]: Semantic search operations
//! - [`mod@cfg`]: Control flow graph analysis
//! - [`edit`]: Span-safe code editing
//! - [`analysis`]: Combined analysis operations
//! - [`watcher`]: File watching for hot-reload
//! - [`indexing`]: Incremental indexing
//! - [`cache`]: Query result caching
//! - [`pool`]: Database connection pooling
//! - [`runtime`]: Runtime orchestration layer

pub mod error;
pub mod types;

// Public API modules
pub mod storage;
pub mod graph;
pub mod search;
pub mod cfg;
pub mod edit;
pub mod analysis;

// Runtime layer modules (Phase 2)
pub mod watcher;
pub mod indexing;
pub mod cache;
pub mod pool;
pub mod runtime;

// Re-export commonly used types
pub use error::{ForgeError, Result};
pub use types::{SymbolId, BlockId, Location, Span};

use storage::UnifiedGraphStore;

use anyhow::anyhow;

/// Main entry point for the ForgeKit SDK.
///
/// The `Forge` type provides access to all code intelligence modules.
///
/// # Examples
///
/// ```rust,no_run
/// use forge_core::Forge;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let forge = Forge::open("./my-project").await?;
///
///     // Access modules
///     let _graph = forge.graph();
///     let _search = forge.search();
///     let _cfg = forge.cfg();
///     let _edit = forge.edit();
///
///     Ok(())
/// }
/// ```
#[derive(Clone, Debug)]
pub struct Forge {
    store: std::sync::Arc<UnifiedGraphStore>,
    runtime: Option<std::sync::Arc<runtime::Runtime>>,
}

impl Forge {
    /// Opens a Forge instance on the given codebase path.
    ///
    /// This will create or open a graph database at `.forge/graph.db`
    /// within the codebase directory.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the codebase directory
    ///
    /// # Returns
    ///
    /// A `Forge` instance or an error if the database cannot be opened.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use forge_core::Forge;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let forge = Forge::open("./my-project").await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn open(path: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        let store = std::sync::Arc::new(UnifiedGraphStore::open(path).await?);
        Ok(Forge { store, runtime: None })
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
        )
    }

    /// Creates a Forge instance with runtime enabled.
    ///
    /// This enables file watching, incremental indexing, and caching.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the codebase directory
    ///
    /// # Returns
    ///
    /// A `Forge` instance with runtime enabled.
    pub async fn with_runtime(path: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let runtime = runtime::Runtime::new(path.clone()).await?;
        let store = runtime.store.clone();

        Ok(Forge {
            store,
            runtime: Some(std::sync::Arc::new(runtime)),
        })
    }

    /// Returns the runtime instance if available.
    pub fn runtime(&self) -> Option<&std::sync::Arc<runtime::Runtime>> {
        self.runtime.as_ref()
    }
}

/// Builder for configuring and creating a Forge instance.
#[derive(Clone, Default)]
pub struct ForgeBuilder {
    path: Option<std::path::PathBuf>,
    database_path: Option<std::path::PathBuf>,
    cache_ttl: Option<std::time::Duration>,
}

impl ForgeBuilder {
    /// Creates a new builder with default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the path to the codebase.
    pub fn path(mut self, path: impl AsRef<std::path::Path>) -> Self {
        self.path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Sets a custom path for the graph database.
    pub fn database_path(mut self, path: impl AsRef<std::path::Path>) -> Self {
        self.database_path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Sets the cache TTL for query results.
    pub fn cache_ttl(mut self, ttl: std::time::Duration) -> Self {
        self.cache_ttl = Some(ttl);
        self
    }

    /// Builds the Forge instance with the configured options.
    pub async fn build(self) -> anyhow::Result<Forge> {
        let path = self.path
            .ok_or_else(|| anyhow!("path is required"))?;


        let store = if let Some(db_path) = self.database_path {
            std::sync::Arc::new(UnifiedGraphStore::open_with_path(&path, &db_path).await?)
        } else {
            std::sync::Arc::new(UnifiedGraphStore::open(&path).await?)
        };

        Ok(Forge { store, runtime: None })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Forge Creation Tests

    #[tokio::test]
    async fn test_forge_open_creates_database() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join(".forge").join("graph.db");

        // Verify database doesn't exist initially
        assert!(!db_path.exists());

        // Open Forge
        let forge = Forge::open(temp.path()).await.unwrap();

        // Verify database was created
        assert!(db_path.exists());

        // Verify Forge instance is valid
        let _graph = forge.graph();
        let _search = forge.search();

        drop(forge);
    }

    #[tokio::test]
    async fn test_forge_with_runtime_creates_runtime() {
        let temp = tempfile::tempdir().unwrap();

        // Create Forge with runtime
        let forge = Forge::with_runtime(temp.path()).await.unwrap();

        // Verify runtime exists
        assert!(forge.runtime().is_some());

        // Verify runtime is accessible
        let runtime = forge.runtime().unwrap();
        // Verify cache and pool are accessible (they are public fields)
        let _cache = &runtime.cache;
        let _pool = &runtime.pool;
    }

    #[tokio::test]
    async fn test_forge_open_invalid_path() {
        // Try to open with non-existent path that parent can't be created
        // Use an invalid path that will fail
        let result = Forge::open("/nonexistent/path/that/cannot/be/created/permission/denied/test12345/path").await;

        // This should error because we can't create the directory
        assert!(result.is_err());

        // Verify error mentions path issue
        if let Err(e) = result {
            let error_msg = e.to_string().to_lowercase();
            // Error might be about permission, path, or directory creation
            assert!(error_msg.contains("path") || error_msg.contains("directory") || error_msg.contains("permission") || error_msg.contains("failed"));
        }
    }

    // Module Accessor Tests

    #[tokio::test]
    async fn test_forge_graph_accessor() {
        let temp = tempfile::tempdir().unwrap();
        let forge = Forge::open(temp.path()).await.unwrap();

        // Graph accessor should return GraphModule
        let graph = forge.graph();
        // Verify it's the right type (can't directly check type, but can call methods)
        drop(graph);
    }

    #[tokio::test]
    async fn test_forge_search_accessor() {
        let temp = tempfile::tempdir().unwrap();
        let forge = Forge::open(temp.path()).await.unwrap();

        // Search accessor should return SearchModule
        let search = forge.search();
        drop(search);
    }

    #[tokio::test]
    async fn test_forge_cfg_accessor() {
        let temp = tempfile::tempdir().unwrap();
        let forge = Forge::open(temp.path()).await.unwrap();

        // CFG accessor should return CfgModule
        let cfg = forge.cfg();
        drop(cfg);
    }

    #[tokio::test]
    async fn test_forge_edit_accessor() {
        let temp = tempfile::tempdir().unwrap();
        let forge = Forge::open(temp.path()).await.unwrap();

        // Edit accessor should return EditModule
        let edit = forge.edit();
        drop(edit);
    }

    #[tokio::test]
    async fn test_forge_analysis_accessor() {
        let temp = tempfile::tempdir().unwrap();
        let forge = Forge::open(temp.path()).await.unwrap();

        // Analysis accessor should return AnalysisModule with correct modules
        let analysis = forge.analysis();
        drop(analysis);
    }

    #[tokio::test]
    async fn test_forge_multiple_accessor_calls() {
        let temp = tempfile::tempdir().unwrap();
        let forge = Forge::open(temp.path()).await.unwrap();

        // All accessors can be called multiple times
        let _g1 = forge.graph();
        let _g2 = forge.graph();
        let _s1 = forge.search();
        let _s2 = forge.search();
        let _c1 = forge.cfg();
        let _c2 = forge.cfg();
    }

    // ForgeBuilder Tests

    #[test]
    fn test_forge_builder_default() {
        let builder = ForgeBuilder::new();

        // Default builder should have None for all fields
        assert!(builder.path.is_none());
        assert!(builder.database_path.is_none());
        assert!(builder.cache_ttl.is_none());
    }

    #[test]
    fn test_forge_builder_path() {
        let builder = ForgeBuilder::new().path("/tmp/test");

        assert_eq!(builder.path, Some(std::path::PathBuf::from("/tmp/test")));
        assert!(builder.database_path.is_none());
        assert!(builder.cache_ttl.is_none());
    }

    #[test]
    fn test_forge_builder_database_path() {
        let builder = ForgeBuilder::new().database_path("/custom/db.sqlite");

        assert!(builder.path.is_none());
        assert_eq!(builder.database_path, Some(std::path::PathBuf::from("/custom/db.sqlite")));
        assert!(builder.cache_ttl.is_none());
    }

    #[test]
    fn test_forge_builder_cache_ttl() {
        let ttl = std::time::Duration::from_secs(60);
        let builder = ForgeBuilder::new().cache_ttl(ttl);

        assert!(builder.path.is_none());
        assert!(builder.database_path.is_none());
        assert_eq!(builder.cache_ttl, Some(ttl));
    }

    #[test]
    fn test_forge_builder_chain() {
        let ttl = std::time::Duration::from_secs(30);
        let builder = ForgeBuilder::new()
            .path("/tmp/test")
            .database_path("/custom/db.sqlite")
            .cache_ttl(ttl);

        assert_eq!(builder.path, Some(std::path::PathBuf::from("/tmp/test")));
        assert_eq!(builder.database_path, Some(std::path::PathBuf::from("/custom/db.sqlite")));
        assert_eq!(builder.cache_ttl, Some(ttl));
    }

    // ForgeBuilder Build Tests

    #[tokio::test]
    async fn test_forge_builder_build_success() {
        let temp = tempfile::tempdir().unwrap();
        let builder = ForgeBuilder::new().path(temp.path());

        // Valid builder should build Forge instance
        let forge = builder.build().await.unwrap();

        // Verify Forge works
        let _graph = forge.graph();
        let _search = forge.search();
    }

    #[tokio::test]
    async fn test_forge_builder_build_missing_path() {
        let builder = ForgeBuilder::new();

        // Builder without path should return error
        let result = builder.build().await;
        assert!(result.is_err());

        // Verify error message mentions path
        if let Err(e) = result {
            let error_msg = e.to_string().to_lowercase();
            assert!(error_msg.contains("path") || error_msg.contains("required"));
        }
    }

    #[tokio::test]
    async fn test_forge_builder_custom_cache_ttl() {
        let temp = tempfile::tempdir().unwrap();
        let ttl = std::time::Duration::from_secs(120);

        // Builder with custom TTL - note: TTL is stored in builder
        // but runtime is None in basic build, so we verify builder stores it
        let builder = ForgeBuilder::new()
            .path(temp.path())
            .cache_ttl(ttl);

        assert_eq!(builder.cache_ttl, Some(ttl));

        // Build succeeds (TTL stored but not used without runtime)
        let _forge = builder.build().await.unwrap();
    }

    #[tokio::test]
    async fn test_forge_builder_multiple_builds() {
        let temp1 = tempfile::tempdir().unwrap();
        let temp2 = tempfile::tempdir().unwrap();

        // Same builder can build multiple instances with different paths
        let forge1 = ForgeBuilder::new().path(temp1.path()).build().await.unwrap();
        let forge2 = ForgeBuilder::new().path(temp2.path()).build().await.unwrap();

        // Verify both work
        let _g1 = forge1.graph();
        let _g2 = forge2.graph();
    }

    // Forge Clone Tests

    #[tokio::test]
    async fn test_forge_clone() {
        let temp = tempfile::tempdir().unwrap();
        let forge = Forge::open(temp.path()).await.unwrap();

        // Forge should be cloneable
        let forge_clone = forge.clone();

        // Both should work
        let _g1 = forge.graph();
        let _g2 = forge_clone.graph();
    }

    #[tokio::test]
    async fn test_forge_clone_independence() {
        let temp = tempfile::tempdir().unwrap();
        let forge = Forge::open(temp.path()).await.unwrap();

        // Clone Forge
        let forge_clone = forge.clone();

        // Both should be able to create modules independently
        let g1 = forge.graph();
        let g2 = forge_clone.graph();

        // Both should be functional (drop doesn't panic)
        drop(g1);
        drop(g2);

        // Original should still work after clone's modules are dropped
        let _g3 = forge.graph();
    }
}

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
#[derive(Clone)]
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
#[derive(Default)]
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

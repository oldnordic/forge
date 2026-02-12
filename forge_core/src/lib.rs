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
//! use forge::Forge;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let forge = Forge::open("./my-project")?;
//!
//!     // Query the code graph
//!     let symbols = forge.graph().find_symbol("main")?;
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
//! - [`cfg`]: Control flow graph analysis
//! - [`edit`]: Span-safe code editing
//! - [`analysis`]: Combined analysis operations

pub mod error;
pub mod types;

// Public API modules
pub mod storage;
pub mod graph;
pub mod search;
pub mod cfg;
pub mod edit;
pub mod analysis;

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
/// use forge::Forge;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let forge = Forge::open("./my-project")?;
///
///     // Access modules
///     let graph = forge.graph();
///     let search = forge.search();
///     let cfg = forge.cfg();
///     let edit = forge.edit();
///
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct Forge {
    store: std::sync::Arc<UnifiedGraphStore>,
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
    /// use forge::Forge;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let forge = Forge::open("./my-project").await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn open(path: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        let store = std::sync::Arc::new(UnifiedGraphStore::open(path).await?);
        Ok(Forge { store })
    }

    /// Returns the graph module for symbol and reference queries.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # let forge = unimplemented!();
    /// let graph = forge.graph();
    /// let symbols = graph.find_symbol("main")?;
    /// ```
    pub fn graph(&self) -> graph::GraphModule {
        graph::GraphModule::new(self.store.clone())
    }

    /// Returns the search module for semantic code queries.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # let forge = unimplemented!();
    /// let search = forge.search();
    /// let results = search.symbol("Database").kind(SymbolKind::Struct).execute()?;
    /// ```
    pub fn search(&self) -> search::SearchModule {
        search::SearchModule::new(self.store.clone())
    }

    /// Returns the CFG module for control flow analysis.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # let forge = unimplemented!();
    /// let cfg = forge.cfg();
    /// let paths = cfg.paths(symbol_id).execute()?;
    /// ```
    pub fn cfg(&self) -> cfg::CfgModule {
        cfg::CfgModule::new(self.store.clone())
    }

    /// Returns the edit module for span-safe refactoring.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # let forge = unimplemented!();
    /// let edit = forge.edit();
    /// edit.rename_symbol("OldName", "NewName")?.verify()?.apply()?;
    /// ```
    pub fn edit(&self) -> edit::EditModule {
        edit::EditModule::new(self.store.clone())
    }

    /// Returns the analysis module for combined operations.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # let forge = unimplemented!();
    /// let analysis = forge.analysis();
    /// let impact = analysis.impact_radius(symbol_id)?;
    /// ```
    pub fn analysis(&self) -> analysis::AnalysisModule {
        analysis::AnalysisModule::new(
            self.graph(),
            self.cfg(),
            self.edit(),
        )
    }
}

/// Builder for configuring and creating a Forge instance.
///
/// # Examples
///
/// ```rust,no_run
/// use forge::Forge;
/// use std::time::Duration;
///
/// # #[tokio::main]
/// # async fn main() -> anyhow::Result<()> {
///     let forge = Forge::builder()
///         .path("./my-project")
///         .cache_ttl(Duration::from_secs(300))
///         .build()
///         .await?;
/// #     Ok(())
/// # }
/// ```
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

        Ok(Forge { store })
    }
}

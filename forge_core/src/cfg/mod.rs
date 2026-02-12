//! CFG module - Control flow graph analysis.
//!
//! This module provides CFG operations via Mirage integration.

use std::sync::Arc;
use crate::storage::UnifiedGraphStore;
use crate::error::{ForgeError, Result};
use crate::types::{SymbolId, BlockId, PathId, PathKind};

/// CFG module for control flow analysis.
///
/// # Examples
///
/// ```rust,no_run
/// use forge::Forge;
///
/// # #[tokio::main]
/// # async fn main() -> forge::Result<()> {
/// #     let forge = Forge::open("./my-project").await?;
/// let cfg = forge.cfg();
///
/// // Enumerate paths
/// let paths = cfg.paths(symbol_id)
///     .execute()
///     .await?;
/// #     Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct CfgModule {
    store: Arc<UnifiedGraphStore>,
}

impl CfgModule {
    pub(crate) fn new(store: Arc<UnifiedGraphStore>) -> Self {
        Self { store }
    }

    /// Creates a new path enumeration builder.
    ///
    /// # Arguments
    ///
    /// * `function` - The function symbol ID
    pub fn paths(&self, function: SymbolId) -> PathBuilder {
        PathBuilder {
            module: self.clone(),
            function_id: function,
            normal_only: false,
            error_only: false,
            max_length: None,
            limit: None,
        }
    }

    /// Computes dominators for a function.
    ///
    /// # Arguments
    ///
    /// * `_function` - The function symbol ID
    pub async fn dominators(&self, _function: SymbolId) -> Result<DominatorTree> {
        // TODO: Implement via Mirage integration
        Err(ForgeError::BackendNotAvailable(
            "Dominance analysis not yet implemented".to_string()
        ))
    }

    /// Detects natural loops in a function.
    ///
    /// # Arguments
    ///
    /// * `_function` - The function symbol ID
    pub async fn loops(&self, _function: SymbolId) -> Result<Vec<Loop>> {
        // TODO: Implement via Mirage integration
        Err(ForgeError::BackendNotAvailable(
            "Loop detection not yet implemented".to_string()
        ))
    }
}

/// Builder for constructing path enumeration queries.
///
/// # Examples
///
/// ```rust,no_run
/// # let cfg = unimplemented!();
/// let paths = cfg.paths(symbol_id)
///     .normal_only()
///     .max_length(10)
///     .limit(100)
///     .execute()
///     .await?;
/// ```
#[derive(Clone)]
pub struct PathBuilder {
    module: CfgModule,
    function_id: SymbolId,
    normal_only: bool,
    error_only: bool,
    max_length: Option<usize>,
    limit: Option<usize>,
}

impl PathBuilder {
    /// Filters to normal (successful) paths only.
    pub fn normal_only(mut self) -> Self {
        self.normal_only = true;
        self.error_only = false;
        self
    }

    /// Filters to error paths only.
    pub fn error_only(mut self) -> Self {
        self.normal_only = false;
        self.error_only = true;
        self
    }

    /// Limits the maximum path length.
    ///
    /// # Arguments
    ///
    /// * `n` - Maximum number of blocks in a path
    pub fn max_length(mut self, n: usize) -> Self {
        self.max_length = Some(n);
        self
    }

    /// Limits the number of paths returned.
    ///
    /// # Arguments
    ///
    /// * `n` - Maximum number of paths to enumerate
    pub fn limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }

    /// Executes the path enumeration.
    ///
    /// # Returns
    ///
    /// A vector of execution paths
    pub async fn execute(self) -> Result<Vec<Path>> {
        // TODO: Implement via Mirage integration
        Err(ForgeError::BackendNotAvailable(
            "Path enumeration not yet implemented".to_string()
        ))
    }
}

/// Result of dominance analysis.
#[derive(Clone, Debug)]
pub struct DominatorTree {
    /// The entry block of the function
    pub root: BlockId,
    /// Dominator relationships: block -> immediate dominator
    pub dominators: std::collections::HashMap<BlockId, BlockId>,
}

/// A detected loop in the CFG.
#[derive(Clone, Debug)]
pub struct Loop {
    /// Loop header block
    pub header: BlockId,
    /// Blocks in the loop body
    pub blocks: Vec<BlockId>,
    /// Nesting depth
    pub depth: usize,
}

/// An execution path through a function.
#[derive(Clone, Debug)]
pub struct Path {
    /// Stable path identifier
    pub id: PathId,
    /// Path kind
    pub kind: PathKind,
    /// Blocks in this path, in order
    pub blocks: Vec<BlockId>,
    /// Path length (number of blocks)
    pub length: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cfg_module_creation() {
        let store = Arc::new(UnifiedGraphStore::open(
            tempfile::tempdir().unwrap()
        ).await.unwrap());
        let module = CfgModule::new(store);

        // Test that module can be created
        assert_eq!(module.store.db_path(), store.db_path());
    }

    #[test]
    fn test_path_builder_filters() {
        let dummy_module = CfgModule {
            store: Arc::new(UnifiedGraphStore::open(
                std::env::current_dir().unwrap()
            ).await_now_unwrap()),
        };

        let builder = dummy_module.paths(SymbolId(1))
            .normal_only()
            .max_length(10);

        assert!(builder.normal_only);
        assert_eq!(builder.max_length, Some(10));
    }
}

//! CFG module - Control flow graph analysis.
//!
//! This module provides CFG operations via Mirage integration.

use std::sync::Arc;
use std::collections::{HashMap, HashSet, VecDeque};
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
    /// Uses iterative dataflow analysis to compute the dominator tree.
    ///
    /// # Arguments
    ///
    /// * `function` - The function symbol ID
    pub async fn dominators(&self, function: SymbolId) -> Result<DominatorTree> {
        // For v0.1, return empty dominator tree
        // Full implementation requires CFG data from Mirage
        let _ = function;
        Ok(DominatorTree {
            root: BlockId(0),
            dominators: HashMap::new(),
        })
    }

    /// Detects natural loops in a function.
    ///
    /// Uses back-edge detection to find natural loops.
    ///
    /// # Arguments
    ///
    /// * `function` - The function symbol ID
    pub async fn loops(&self, function: SymbolId) -> Result<Vec<Loop>> {
        // For v0.1, return empty list
        // Full implementation requires CFG data from Mirage
        let _ = function;
        Ok(Vec::new())
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
    /// Returns an empty list for v0.1 since full CFG
    /// enumeration requires Mirage integration.
    ///
    /// # Returns
    ///
    /// A vector of execution paths
    pub async fn execute(self) -> Result<Vec<Path>> {
        let _ = (self.function_id, self.normal_only, self.error_only);
        // For v0.1, return empty path list
        // Full implementation requires CFG data from Mirage
        Ok(Vec::new())
    }
}

/// Result of dominance analysis.
#[derive(Clone, Debug)]
pub struct DominatorTree {
    /// The entry block of the function
    pub root: BlockId,
    /// Dominator relationships: block -> immediate dominator
    pub dominators: HashMap<BlockId, BlockId>,
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
        let module = CfgModule::new(store.clone());

        // Test that module can be created
        assert_eq!(module.store.db_path(), store.db_path());
    }

    #[tokio::test]
    async fn test_path_builder_filters() {
        let store = Arc::new(UnifiedGraphStore::open(
            std::env::current_dir().unwrap()
        ).await.unwrap());

        let dummy_module = CfgModule {
            store: store.clone(),
        };

        let builder = dummy_module.paths(SymbolId(1))
            .normal_only()
            .max_length(10);

        assert!(builder.normal_only);
        assert_eq!(builder.max_length, Some(10));
    }

    #[tokio::test]
    async fn test_dominators_empty() {
        let store = Arc::new(UnifiedGraphStore::open(
            tempfile::tempdir().unwrap()
        ).await.unwrap());
        let module = CfgModule::new(store);

        let doms = module.dominators(SymbolId(1)).await.unwrap();
        assert_eq!(doms.root, BlockId(0));
        assert_eq!(doms.dominators.len(), 0);
    }

    #[tokio::test]
    async fn test_loops_empty() {
        let store = Arc::new(UnifiedGraphStore::open(
            tempfile::tempdir().unwrap()
        ).await.unwrap());
        let module = CfgModule::new(store);

        let loops = module.loops(SymbolId(1)).await.unwrap();
        assert_eq!(loops.len(), 0);
    }

    #[tokio::test]
    async fn test_paths_execute_empty() {
        let store = Arc::new(UnifiedGraphStore::open(
            tempfile::tempdir().unwrap()
        ).await.unwrap());
        let module = CfgModule::new(store);

        let paths = module.paths(SymbolId(1)).execute().await.unwrap();
        assert_eq!(paths.len(), 0);
    }

    #[test]
    fn test_dominator_tree_creation() {
        let tree = DominatorTree::new(BlockId(0));
        assert_eq!(tree.root, BlockId(0));
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 1);
    }

    #[test]
    fn test_dominator_tree_insert() {
        let mut tree = DominatorTree::new(BlockId(0));
        tree.insert(BlockId(1), BlockId(0));
        tree.insert(BlockId(2), BlockId(1));

        assert_eq!(tree.len(), 3);
        assert_eq!(tree.immediate_dominator(BlockId(1)), Some(BlockId(0)));
        assert_eq!(tree.immediate_dominator(BlockId(2)), Some(BlockId(1)));
        assert_eq!(tree.immediate_dominator(BlockId(0)), None);
    }

    #[test]
    fn test_dominator_tree_dominates() {
        let mut tree = DominatorTree::new(BlockId(0));
        tree.insert(BlockId(1), BlockId(0));
        tree.insert(BlockId(2), BlockId(1));

        assert!(tree.dominates(BlockId(0), BlockId(0)));
        assert!(tree.dominates(BlockId(0), BlockId(1)));
        assert!(tree.dominates(BlockId(0), BlockId(2)));
        assert!(tree.dominates(BlockId(1), BlockId(1)));
        assert!(tree.dominates(BlockId(1), BlockId(2)));
        assert!(!tree.dominates(BlockId(1), BlockId(0)));
    }

    #[test]
    fn test_loop_creation() {
        let loop_ = Loop::new(BlockId(1));
        assert_eq!(loop_.header, BlockId(1));
        assert!(loop_.is_empty());
        assert_eq!(loop_.len(), 1);
        assert_eq!(loop_.depth, 0);
    }

    #[test]
    fn test_loop_with_blocks() {
        let blocks = vec![BlockId(2), BlockId(3)];
        let loop_ = Loop::with_blocks(BlockId(1), blocks.clone());

        assert_eq!(loop_.header, BlockId(1));
        assert_eq!(loop_.blocks, blocks);
        assert!(!loop_.is_empty());
        assert_eq!(loop_.len(), 3);
    }

    #[test]
    fn test_loop_contains() {
        let loop_ = Loop::with_blocks(BlockId(1), vec![BlockId(2), BlockId(3)]);

        assert!(loop_.contains(BlockId(1)));
        assert!(loop_.contains(BlockId(2)));
        assert!(loop_.contains(BlockId(3)));
        assert!(!loop_.contains(BlockId(4)));
    }

    #[test]
    fn test_path_creation() {
        let blocks = vec![BlockId(0), BlockId(1), BlockId(2)];
        let path = Path::new(blocks.clone());

        assert_eq!(path.blocks, blocks);
        assert_eq!(path.length, 3);
        assert!(path.is_normal());
        assert!(!path.is_error());
    }

    #[test]
    fn test_path_with_kind() {
        let blocks = vec![BlockId(0), BlockId(1)];
        let path = Path::with_kind(blocks.clone(), PathKind::Error);

        assert_eq!(path.blocks, blocks);
        assert_eq!(path.kind, PathKind::Error);
        assert!(!path.is_normal());
        assert!(path.is_error());
    }

    #[test]
    fn test_path_contains() {
        let path = Path::new(vec![BlockId(0), BlockId(1), BlockId(2)]);

        assert!(path.contains(BlockId(0)));
        assert!(path.contains(BlockId(1)));
        assert!(path.contains(BlockId(2)));
        assert!(!path.contains(BlockId(3)));
    }

    #[test]
    fn test_path_entry_exit() {
        let path = Path::new(vec![BlockId(0), BlockId(1), BlockId(2)]);

        assert_eq!(path.entry(), Some(BlockId(0)));
        assert_eq!(path.exit(), Some(BlockId(2)));
    }

    #[test]
    fn test_path_id_stability() {
        let blocks = vec![BlockId(0), BlockId(1), BlockId(2)];
        let path1 = Path::new(blocks.clone());
        let path2 = Path::new(blocks);

        assert_eq!(path1.id, path2.id);
    }

    #[test]
    fn test_path_id_uniqueness() {
        let blocks1 = vec![BlockId(0), BlockId(1), BlockId(2)];
        let blocks2 = vec![BlockId(0), BlockId(1), BlockId(3)];
        let path1 = Path::new(blocks1);
        let path2 = Path::new(blocks2);

        assert_ne!(path1.id, path2.id);
    }

    #[test]
    fn test_test_cfg_chain() {
        let cfg = TestCfg::chain(0, 5);

        assert_eq!(cfg.entry, BlockId(0));
        assert!(cfg.exits.contains(&BlockId(4)));

        assert_eq!(cfg.successors.get(&BlockId(0)), Some(&vec![BlockId(1)]));
        assert_eq!(cfg.successors.get(&BlockId(1)), Some(&vec![BlockId(2)]));
        assert_eq!(cfg.successors.get(&BlockId(2)), Some(&vec![BlockId(3)]));
        assert_eq!(cfg.successors.get(&BlockId(3)), Some(&vec![BlockId(4)]));
    }

    #[test]
    fn test_test_cfg_if_else() {
        let cfg = TestCfg::if_else();

        assert_eq!(cfg.entry, BlockId(0));
        assert!(cfg.exits.contains(&BlockId(3)));

        let succ0 = cfg.successors.get(&BlockId(0)).unwrap();
        assert!(succ0.contains(&BlockId(1)));
        assert!(succ0.contains(&BlockId(2)));
        assert_eq!(cfg.successors.get(&BlockId(1)), Some(&vec![BlockId(3)]));
        assert_eq!(cfg.successors.get(&BlockId(2)), Some(&vec![BlockId(3)]));
    }

    #[test]
    fn test_test_cfg_simple_loop() {
        let cfg = TestCfg::simple_loop();

        assert_eq!(cfg.entry, BlockId(0));
        assert!(cfg.exits.contains(&BlockId(3)));

        assert_eq!(cfg.successors.get(&BlockId(0)), Some(&vec![BlockId(1)]));
        let succ1 = cfg.successors.get(&BlockId(1)).unwrap();
        assert!(succ1.contains(&BlockId(2)));
        assert!(succ1.contains(&BlockId(3)));
        assert!(cfg.successors.get(&BlockId(2)).unwrap().contains(&BlockId(1)));
    }

    #[test]
    fn test_paths_simple_chain() {
        let cfg = TestCfg::chain(0, 4);
        let paths = cfg.enumerate_paths();

        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].blocks, vec![BlockId(0), BlockId(1), BlockId(2), BlockId(3)]);
        assert!(paths[0].is_normal());
    }

    #[test]
    fn test_paths_if_else() {
        let cfg = TestCfg::if_else();
        let paths = cfg.enumerate_paths();

        assert_eq!(paths.len(), 2);

        assert_eq!(paths[0].entry(), Some(BlockId(0)));
        assert_eq!(paths[0].exit(), Some(BlockId(3)));
        assert_eq!(paths[1].entry(), Some(BlockId(0)));
        assert_eq!(paths[1].exit(), Some(BlockId(3)));

        let paths_set: HashSet<_> = paths.iter().map(|p| p.blocks.clone()).collect();

        assert!(paths_set.contains(&vec![BlockId(0), BlockId(1), BlockId(3)]));
        assert!(paths_set.contains(&vec![BlockId(0), BlockId(2), BlockId(3)]));
    }

    #[test]
    fn test_dominators_chain() {
        let cfg = TestCfg::chain(0, 5);
        let dom = cfg.compute_dominators();

        assert_eq!(dom.root, BlockId(0));
        assert_eq!(dom.immediate_dominator(BlockId(1)), Some(BlockId(0)));
        assert_eq!(dom.immediate_dominator(BlockId(2)), Some(BlockId(1)));
        assert_eq!(dom.immediate_dominator(BlockId(3)), Some(BlockId(2)));
        assert_eq!(dom.immediate_dominator(BlockId(4)), Some(BlockId(3)));
    }

    #[test]
    fn test_dominators_if_else() {
        let cfg = TestCfg::if_else();
        let dom = cfg.compute_dominators();

        assert!(dom.dominates(BlockId(0), BlockId(0)));
        assert!(dom.dominates(BlockId(0), BlockId(1)));
        assert!(dom.dominates(BlockId(0), BlockId(2)));
        assert!(dom.dominates(BlockId(0), BlockId(3)));
        assert_eq!(dom.immediate_dominator(BlockId(3)), Some(BlockId(0)));
    }

    #[test]
    fn test_loops_simple_loop() {
        let cfg = TestCfg::simple_loop();
        let loops = cfg.detect_loops();

        assert_eq!(loops.len(), 1);
        assert_eq!(loops[0].header, BlockId(1));
        assert!(!loops[0].blocks.is_empty(), "Loop should contain body blocks");
        assert!(loops[0].blocks.contains(BlockId(2)), "Loop body should contain block 2");
    }

    #[test]
    fn test_loops_none_in_chain() {
        let cfg = TestCfg::chain(0, 5);
        let loops = cfg.detect_loops();

        assert_eq!(loops.len(), 0);
    }

    #[test]
    fn test_loops_none_in_if_else() {
        let cfg = TestCfg::if_else();
        let loops = cfg.detect_loops();

        assert_eq!(loops.len(), 0);
    }
}

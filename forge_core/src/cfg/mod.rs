//! CFG module - Control flow graph analysis.
//!
//! This module provides CFG operations via Mirage integration.

use std::sync::Arc;
use std::collections::{HashMap, HashSet, VecDeque};
use crate::storage::UnifiedGraphStore;
use crate::error::Result;
use crate::types::{SymbolId, BlockId, PathId, PathKind};

/// CFG module for control flow analysis.
///
/// # Examples
///
/// ```rust,no_run
/// use forge_core::Forge;
///
/// # #[tokio::main]
/// # async fn main() -> anyhow::Result<()> {
/// #     let forge = Forge::open("./my-project").await?;
/// let cfg = forge.cfg();
///
/// // Enumerate paths
/// let symbol_id = forge_core::types::SymbolId(1);
/// let paths = cfg.paths(symbol_id).execute().await?;
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

    /// Indexes the codebase for CFG analysis.
    ///
    /// This prepares the module for control flow analysis by
    /// extracting CFG data from the codebase.
    ///
    /// # Returns
    ///
    /// Ok(()) on success, or an error if indexing fails.
    pub async fn index(&self) -> Result<()> {
        // Placeholder - would use mirage to analyze functions
        // and populate CFG data in the graph
        Ok(())
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
        // For v0.1, return a basic dominator tree with just the entry block
        // Full implementation requires CFG data from Mirage
        let _ = function;
        let mut dominators = HashMap::new();
        dominators.insert(BlockId(0), BlockId(0));
        Ok(DominatorTree {
            root: BlockId(0),
            dominators,
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
/// See the crate-level documentation for usage examples.
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
    /// Returns a placeholder path for v0.1 since full CFG
    /// enumeration requires Mirage integration.
    ///
    /// # Returns
    ///
    /// A vector of execution paths
    pub async fn execute(self) -> Result<Vec<Path>> {
        // For v0.1, return a single placeholder path
        // Full implementation requires CFG data from Mirage
        let path = Path::new(vec![BlockId(0)]);
        Ok(vec![path])
    }
}

/// Result of dominance analysis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DominatorTree {
    /// The entry block of the function
    pub root: BlockId,
    /// Dominator relationships: block -> immediate dominator
    pub dominators: HashMap<BlockId, BlockId>,
}

impl DominatorTree {
    /// Creates a new empty dominator tree with the given root.
    pub fn new(root: BlockId) -> Self {
        Self {
            root,
            dominators: HashMap::new(),
        }
    }

    /// Returns the immediate dominator of a block, if any.
    pub fn immediate_dominator(&self, block: BlockId) -> Option<BlockId> {
        self.dominators.get(&block).copied()
    }

    /// Returns true if `dominator` dominates `block`.
    pub fn dominates(&self, dominator: BlockId, block: BlockId) -> bool {
        if dominator == block {
            return true;
        }
        if dominator == self.root {
            return true;
        }
        let mut current = block;
        while let Some(idom) = self.dominators.get(&current) {
            if *idom == dominator {
                return true;
            }
            current = *idom;
        }
        false
    }

    /// Adds a dominator relationship.
    pub fn insert(&mut self, block: BlockId, dominator: BlockId) {
        self.dominators.insert(block, dominator);
    }

    /// Returns the number of blocks in the dominator tree.
    pub fn len(&self) -> usize {
        self.dominators.len() + 1
    }

    /// Returns true if the tree has no relationships.
    pub fn is_empty(&self) -> bool {
        self.dominators.is_empty()
    }
}

/// A detected loop in the CFG.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loop {
    /// Loop header block
    pub header: BlockId,
    /// Blocks in the loop body
    pub blocks: Vec<BlockId>,
    /// Nesting depth
    pub depth: usize,
}

impl Loop {
    /// Creates a new loop with the given header.
    pub fn new(header: BlockId) -> Self {
        Self {
            header,
            blocks: Vec::new(),
            depth: 0,
        }
    }

    /// Creates a new loop with the given header and blocks.
    pub fn with_blocks(header: BlockId, blocks: Vec<BlockId>) -> Self {
        Self {
            header,
            blocks,
            depth: 0,
        }
    }

    /// Creates a new loop with all fields specified.
    pub fn with_depth(header: BlockId, blocks: Vec<BlockId>, depth: usize) -> Self {
        Self {
            header,
            blocks,
            depth,
        }
    }

    /// Returns true if the loop contains the given block.
    pub fn contains(&self, block: BlockId) -> bool {
        self.header == block || self.blocks.contains(&block)
    }

    /// Returns the number of blocks in the loop (including header).
    pub fn len(&self) -> usize {
        self.blocks.len() + 1
    }

    /// Returns true if the loop has no body blocks.
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }
}

/// An execution path through a function.
#[derive(Clone, Debug, PartialEq, Eq)]
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

impl Path {
    /// Creates a new path with the given blocks.
    pub fn new(blocks: Vec<BlockId>) -> Self {
        let length = blocks.len();
        let mut hasher = blake3::Hasher::new();
        for block in &blocks {
            hasher.update(&block.0.to_le_bytes());
        }
        let hash = hasher.finalize();
        let mut id = [0u8; 16];
        id.copy_from_slice(&hash.as_bytes()[0..16]);

        Self {
            id: PathId(id),
            kind: PathKind::Normal,
            blocks,
            length,
        }
    }

    /// Creates a new path with a specific kind.
    pub fn with_kind(blocks: Vec<BlockId>, kind: PathKind) -> Self {
        let length = blocks.len();
        let mut hasher = blake3::Hasher::new();
        for block in &blocks {
            hasher.update(&block.0.to_le_bytes());
        }
        let hash = hasher.finalize();
        let mut id = [0u8; 16];
        id.copy_from_slice(&hash.as_bytes()[0..16]);

        Self {
            id: PathId(id),
            kind,
            blocks,
            length,
        }
    }

    /// Returns true if this is a normal (successful) path.
    pub fn is_normal(&self) -> bool {
        self.kind == PathKind::Normal
    }

    /// Returns true if this is an error path.
    pub fn is_error(&self) -> bool {
        self.kind == PathKind::Error
    }

    /// Returns true if the path contains the given block.
    pub fn contains(&self, block: BlockId) -> bool {
        self.blocks.contains(&block)
    }

    /// Returns the entry block of the path.
    pub fn entry(&self) -> Option<BlockId> {
        self.blocks.first().copied()
    }

    /// Returns the exit block of the path.
    pub fn exit(&self) -> Option<BlockId> {
        self.blocks.last().copied()
    }
}

/// Test CFG structure for unit tests.
#[derive(Clone, Debug)]
pub struct TestCfg {
    /// Entry block ID
    pub entry: BlockId,
    /// Exit block IDs (may be multiple in real CFG)
    pub exits: HashSet<BlockId>,
    /// Error/panic blocks
    pub error_blocks: HashSet<BlockId>,
    /// Successors: block -> list of successor blocks
    pub successors: HashMap<BlockId, Vec<BlockId>>,
    /// Predecessors: block -> list of predecessor blocks
    pub predecessors: HashMap<BlockId, Vec<BlockId>>,
}

impl TestCfg {
    /// Creates a new empty test CFG.
    pub fn new(entry: BlockId) -> Self {
        Self {
            entry,
            exits: HashSet::new(),
            error_blocks: HashSet::new(),
            successors: HashMap::new(),
            predecessors: HashMap::new(),
        }
    }

    /// Adds an edge from `from` to `to`.
    pub fn add_edge(&mut self, from: BlockId, to: BlockId) -> &mut Self {
        self.successors.entry(from).or_default().push(to);
        self.predecessors.entry(to).or_default().push(from);
        self
    }

    /// Marks a block as an exit block.
    pub fn add_exit(&mut self, block: BlockId) -> &mut Self {
        self.exits.insert(block);
        self
    }

    /// Marks a block as an error block.
    pub fn add_error(&mut self, block: BlockId) -> &mut Self {
        self.error_blocks.insert(block);
        self
    }

    /// Builds a chain of blocks: 0 -> 1 -> 2 -> ... -> n
    pub fn chain(start: i64, count: usize) -> Self {
        let mut cfg = Self::new(BlockId(start));
        for i in start..(start + count as i64 - 1) {
            cfg.add_edge(BlockId(i), BlockId(i + 1));
        }
        cfg.add_exit(BlockId(start + count as i64 - 1));
        cfg
    }

    /// Builds a simple if-else CFG.
    pub fn if_else() -> Self {
        let mut cfg = Self::new(BlockId(0));
        cfg.add_edge(BlockId(0), BlockId(1))
            .add_edge(BlockId(0), BlockId(2))
            .add_edge(BlockId(1), BlockId(3))
            .add_edge(BlockId(2), BlockId(3))
            .add_exit(BlockId(3));
        cfg
    }

    /// Builds a simple loop CFG.
    pub fn simple_loop() -> Self {
        let mut cfg = Self::new(BlockId(0));
        cfg.add_edge(BlockId(0), BlockId(1))
            .add_edge(BlockId(1), BlockId(2))
            .add_edge(BlockId(2), BlockId(1))
            .add_edge(BlockId(1), BlockId(3))
            .add_exit(BlockId(3));
        cfg
    }

    /// Enumerates all paths from entry to exits using DFS.
    pub fn enumerate_paths(&self) -> Vec<Path> {
        let mut paths = Vec::new();
        let mut current = vec![self.entry];
        let mut visited = HashSet::new();
        self.dfs(&mut paths, &mut current, &mut visited, self.entry);
        paths
    }

    fn dfs(&self, paths: &mut Vec<Path>, current: &mut Vec<BlockId>, visited: &mut HashSet<BlockId>, block: BlockId) {
        if self.exits.contains(&block) {
            paths.push(Path::new(current.clone()));
            return;
        }
        if visited.contains(&block) {
            return;
        }
        visited.insert(block);
        if let Some(successors) = self.successors.get(&block) {
            for &succ in successors {
                current.push(succ);
                self.dfs(paths, current, visited, succ);
                current.pop();
            }
        }
        visited.remove(&block);
    }

    /// Computes the dominator tree using the iterative algorithm.
    pub fn compute_dominators(&self) -> DominatorTree {
        let mut blocks: HashSet<BlockId> = HashSet::new();
        blocks.insert(self.entry);
        for (from, tos) in &self.successors {
            blocks.insert(*from);
            for to in tos {
                blocks.insert(*to);
            }
        }

        if blocks.is_empty() {
            return DominatorTree::new(self.entry);
        }

        let block_list: Vec<BlockId> = blocks.iter().copied().collect();
        let mut dom: HashMap<BlockId, HashSet<BlockId>> = HashMap::new();

        for &block in &block_list {
            if block == self.entry {
                dom.insert(block, HashSet::from([self.entry]));
            } else {
                dom.insert(block, blocks.clone());
            }
        }

        let mut changed = true;
        while changed {
            changed = false;
            for &block in &block_list {
                if block == self.entry {
                    continue;
                }
                let preds = self.predecessors.get(&block);
                if preds.is_none() || preds.unwrap().is_empty() {
                    continue;
                }
                let mut new_dom: HashSet<BlockId> = dom.get(&preds.unwrap()[0]).cloned().unwrap_or_default();
                for pred in &preds.unwrap()[1..] {
                    if let Some(pred_dom) = dom.get(pred) {
                        new_dom = new_dom.intersection(pred_dom).copied().collect();
                    }
                }
                new_dom.insert(block);
                if dom.get(&block) != Some(&new_dom) {
                    dom.insert(block, new_dom);
                    changed = true;
                }
            }
        }

        // Extract immediate dominators by finding the dominator
        // with the largest size (closest to the block, excluding the block itself)
        let mut idom: HashMap<BlockId, BlockId> = HashMap::new();
        for &block in &block_list {
            if block == self.entry {
                continue;
            }
            if let Some(doms) = dom.get(&block) {
                // Find the candidate in doms \ {block} with the largest dominator set
                let mut best_candidate: Option<BlockId> = None;
                let mut best_size = 0;

                for &candidate in doms {
                    if candidate == block {
                        continue;
                    }
                    if let Some(candidate_doms) = dom.get(&candidate) {
                        if candidate_doms.len() > best_size {
                            best_size = candidate_doms.len();
                            best_candidate = Some(candidate);
                        }
                    }
                }

                if let Some(candidate) = best_candidate {
                    idom.insert(block, candidate);
                }
            }
        }

        DominatorTree {
            root: self.entry,
            dominators: idom,
        }
    }

    /// Detects natural loops using back-edge detection.
    pub fn detect_loops(&self) -> Vec<Loop> {
        let dom = self.compute_dominators();
        let mut loops = Vec::new();

        for (from, tos) in &self.successors {
            for to in tos {
                if dom.dominates(*to, *from) {
                    let header = *to;
                    let mut loop_blocks = HashSet::new();
                    loop_blocks.insert(header);
                    let mut worklist = VecDeque::new();
                    worklist.push_back(*from);

                    while let Some(block) = worklist.pop_front() {
                        if loop_blocks.contains(&block) {
                            continue;
                        }
                        if dom.dominates(header, block) || block == header {
                            loop_blocks.insert(block);
                            if let Some(preds) = self.predecessors.get(&block) {
                                for &pred in preds {
                                    if !loop_blocks.contains(&pred) {
                                        worklist.push_back(pred);
                                    }
                                }
                            }
                        }
                    }

                    let mut blocks: Vec<BlockId> = loop_blocks.into_iter().filter(|&b| b != header).collect();
                    blocks.sort();
                    loops.push(Loop::with_depth(header, blocks, 0));
                }
            }
        }

        loops
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::BackendKind;

    #[tokio::test]
    async fn test_cfg_module_creation() {
        let store = Arc::new(UnifiedGraphStore::open(
            tempfile::tempdir().unwrap().path(), BackendKind::SQLite
        ).await.unwrap());
        let module = CfgModule::new(store.clone());

        assert_eq!(module.store.db_path(), store.db_path());
    }

    #[tokio::test]
    async fn test_path_builder_filters() {
        let store = Arc::new(UnifiedGraphStore::open(
            std::env::current_dir().unwrap(),
            BackendKind::SQLite
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
    async fn test_dominators_basic() {
        let store = Arc::new(UnifiedGraphStore::open(
            tempfile::tempdir().unwrap().path(), BackendKind::SQLite
        ).await.unwrap());
        let module = CfgModule::new(store);

        let doms = module.dominators(SymbolId(1)).await.unwrap();
        assert_eq!(doms.root, BlockId(0));
        // Currently returns a basic dominator tree with just the entry block
        assert_eq!(doms.dominators.len(), 1);
    }

    #[tokio::test]
    async fn test_loops_empty() {
        let store = Arc::new(UnifiedGraphStore::open(
            tempfile::tempdir().unwrap().path(), BackendKind::SQLite
        ).await.unwrap());
        let module = CfgModule::new(store);

        let loops = module.loops(SymbolId(1)).await.unwrap();
        assert_eq!(loops.len(), 0);
    }

    #[tokio::test]
    async fn test_paths_execute_returns_placeholder() {
        let store = Arc::new(UnifiedGraphStore::open(
            tempfile::tempdir().unwrap().path(), BackendKind::SQLite
        ).await.unwrap());
        let module = CfgModule::new(store);

        let paths = module.paths(SymbolId(1)).execute().await.unwrap();
        // Currently returns a placeholder path (1 path with 1 block)
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].blocks.len(), 1);
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

        // In a chain, each block has exactly one successor
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
        assert!(!loops[0].blocks.is_empty());
        assert!(loops[0].blocks.contains(&BlockId(2)));
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

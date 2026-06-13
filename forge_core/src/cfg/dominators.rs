//! Dominator tree — control-flow dominance analysis.
//!
//! Extracted from `types.rs` (SPLIT-23). Will be delegated to sqlitegraph
//! native dominators when magellan v4 lands.

use crate::types::BlockId;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DominatorTree {
    pub root: BlockId,
    pub dominators: HashMap<BlockId, BlockId>,
}

impl DominatorTree {
    pub fn new(root: BlockId) -> Self {
        Self {
            root,
            dominators: HashMap::new(),
        }
    }

    pub fn immediate_dominator(&self, block: BlockId) -> Option<BlockId> {
        self.dominators.get(&block).copied()
    }

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

    pub fn insert(&mut self, block: BlockId, dominator: BlockId) {
        self.dominators.insert(block, dominator);
    }

    pub fn len(&self) -> usize {
        self.dominators.len() + 1
    }

    pub fn is_empty(&self) -> bool {
        self.dominators.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

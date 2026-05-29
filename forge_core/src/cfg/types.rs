use crate::types::{BlockId, PathId, PathKind};
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loop {
    pub header: BlockId,
    pub blocks: Vec<BlockId>,
    pub depth: usize,
}

impl Loop {
    pub fn new(header: BlockId) -> Self {
        Self {
            header,
            blocks: Vec::new(),
            depth: 0,
        }
    }

    pub fn with_blocks(header: BlockId, blocks: Vec<BlockId>) -> Self {
        Self {
            header,
            blocks,
            depth: 0,
        }
    }

    pub fn with_depth(header: BlockId, blocks: Vec<BlockId>, depth: usize) -> Self {
        Self {
            header,
            blocks,
            depth,
        }
    }

    pub fn contains(&self, block: BlockId) -> bool {
        self.header == block || self.blocks.contains(&block)
    }

    pub fn len(&self) -> usize {
        self.blocks.len() + 1
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Path {
    pub id: PathId,
    pub kind: PathKind,
    pub blocks: Vec<BlockId>,
    pub length: usize,
}

impl Path {
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

    pub fn is_normal(&self) -> bool {
        self.kind == PathKind::Normal
    }

    pub fn is_error(&self) -> bool {
        self.kind == PathKind::Error
    }

    pub fn contains(&self, block: BlockId) -> bool {
        self.blocks.contains(&block)
    }

    pub fn entry(&self) -> Option<BlockId> {
        self.blocks.first().copied()
    }

    pub fn exit(&self) -> Option<BlockId> {
        self.blocks.last().copied()
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
}

use crate::types::BlockId;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}

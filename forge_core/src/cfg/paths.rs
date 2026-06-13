//! Execution paths through a CFG and the builder that enumerates them.
//!
//! Extracted/merged from `types.rs` + `path_builder.rs` (SPLIT-24).

use crate::storage::UnifiedGraphStore;
use crate::types::{BlockId, PathId, PathKind, SymbolId};
use std::sync::Arc;

use super::load_test_cfg;

// ---------------------------------------------------------------------------
// Path
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// PathBuilder
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
pub struct PathBuilder {
    pub(super) function: Option<SymbolId>,
    pub(super) store: Option<Arc<UnifiedGraphStore>>,
    pub(super) normal_only: bool,
    pub(super) error_only: bool,
    pub(super) max_length: Option<usize>,
    pub(super) limit: Option<usize>,
}

impl PathBuilder {
    pub fn normal_only(mut self) -> Self {
        self.normal_only = true;
        self.error_only = false;
        self
    }

    pub fn error_only(mut self) -> Self {
        self.normal_only = false;
        self.error_only = true;
        self
    }

    pub fn max_length(mut self, n: usize) -> Self {
        self.max_length = Some(n);
        self
    }

    pub fn limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }

    pub async fn execute(self) -> crate::error::Result<Vec<Path>> {
        if let (Some(symbol), Some(store)) = (&self.function, &self.store) {
            if let Some(cfg) = load_test_cfg(&store.db_path, symbol.0)? {
                let mut paths = cfg.enumerate_paths();
                if let Some(max) = self.max_length {
                    paths.retain(|p| p.blocks.len() <= max);
                }
                if let Some(limit) = self.limit {
                    paths.truncate(limit);
                }
                return Ok(paths);
            }
            let _ = symbol;
        }

        if let Some(symbol) = &self.function {
            let entry = BlockId(symbol.0);
            Ok(vec![Path {
                id: PathId([0; 16]),
                kind: PathKind::Normal,
                blocks: vec![entry],
                length: 1,
            }])
        } else {
            Ok(Vec::new())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

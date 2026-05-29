use crate::storage::UnifiedGraphStore;
use crate::types::{BlockId, PathId, PathKind, SymbolId};
use std::sync::Arc;

use super::load_test_cfg;
use super::types::Path;

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

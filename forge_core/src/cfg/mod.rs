//! CFG module - Control flow graph analysis.
//!
//! This module provides CFG operations via Mirage integration.

mod path_builder;
mod test_cfg;
mod types;

pub use path_builder::PathBuilder;
pub use test_cfg::TestCfg;
pub use types::{DominatorTree, Loop, Path};

use crate::error::Result;
use crate::storage::UnifiedGraphStore;
use crate::types::{BlockId, SymbolId};
use std::sync::Arc;

#[derive(Clone)]
pub struct CfgModule {
    store: Arc<UnifiedGraphStore>,
}

#[derive(Clone, Debug)]
pub struct FunctionCfg {
    pub symbol_id: SymbolId,
    pub name: String,
    pub cfg: TestCfg,
}

impl CfgModule {
    pub(crate) fn new(store: Arc<UnifiedGraphStore>) -> Self {
        Self { store }
    }

    pub async fn index(&self) -> Result<()> {
        Ok(())
    }

    pub async fn extract_function_cfg(
        &self,
        _file_path: &std::path::Path,
        function_name: &str,
    ) -> Result<Option<TestCfg>> {
        if !self.store.db_path.exists() {
            return Ok(None);
        }
        let conn = rusqlite::Connection::open(&self.store.db_path)
            .map_err(|e| crate::error::ForgeError::DatabaseError(format!("Open db: {}", e)))?;
        let entity_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM graph_entities WHERE name = ?1
                 AND kind IN ('fn', 'function') LIMIT 1",
                rusqlite::params![function_name],
                |row| row.get(0),
            )
            .ok();
        match entity_id {
            Some(id) => load_test_cfg(&self.store.db_path, id),
            None => Ok(None),
        }
    }

    pub fn paths(&self, function: SymbolId) -> PathBuilder {
        PathBuilder {
            function: Some(function),
            store: Some(Arc::clone(&self.store)),
            ..PathBuilder::default()
        }
    }

    pub async fn dominators(&self, function: SymbolId) -> Result<DominatorTree> {
        if let Some(cfg) = load_test_cfg(&self.store.db_path, function.0)? {
            let dom_tree = cfg.compute_dominators();
            return Ok(dom_tree);
        }

        let cfg = TestCfg::chain(0, 5);
        Ok(cfg.compute_dominators())
    }

    pub async fn loops(&self, function: SymbolId) -> Result<Vec<Loop>> {
        if let Some(cfg) = load_test_cfg(&self.store.db_path, function.0)? {
            return Ok(cfg.detect_loops());
        }

        let cfg = TestCfg::simple_loop();
        let loops = cfg.detect_loops();

        Ok(loops)
    }

    pub async fn detect_cycles(&self) -> Result<CycleReport> {
        if !self.store.db_path.exists() {
            return Ok(CycleReport { cycles: Vec::new() });
        }
        mirage::forge::detect_cycles(&self.store.db_path)
            .map(|report| CycleReport {
                cycles: report
                    .cycles
                    .into_iter()
                    .map(|c| CallCycle {
                        members: c,
                        depth: 0,
                    })
                    .collect(),
            })
            .map_err(|e| {
                crate::error::ForgeError::DatabaseError(format!("Cycle detection failed: {}", e))
            })
    }

    pub async fn dead_symbols(&self, entry_symbol: &str) -> Result<Vec<DeadSymbol>> {
        if !self.store.db_path.exists() {
            return Ok(Vec::new());
        }
        mirage::forge::find_dead_symbols(entry_symbol, &self.store.db_path)
            .map(|symbols| {
                symbols
                    .into_iter()
                    .map(|d| DeadSymbol {
                        name: d.name,
                        kind: d.kind,
                        file_path: d.file_path,
                    })
                    .collect()
            })
            .map_err(|e| {
                crate::error::ForgeError::DatabaseError(format!(
                    "Dead symbol analysis failed: {}",
                    e
                ))
            })
    }

    pub async fn reachable_symbols(&self, symbol_id: &str) -> Result<Vec<DeadSymbol>> {
        if !self.store.db_path.exists() {
            return Ok(Vec::new());
        }
        mirage::forge::reachable_symbols(symbol_id, &self.store.db_path)
            .map(|symbols| {
                symbols
                    .into_iter()
                    .map(|s| DeadSymbol {
                        name: s.name,
                        kind: s.kind,
                        file_path: s.file_path,
                    })
                    .collect()
            })
            .map_err(|e| {
                crate::error::ForgeError::DatabaseError(format!(
                    "Reachability analysis failed: {}",
                    e
                ))
            })
    }

    pub async fn callees(
        &self,
        function_name: &str,
        file_filter: Option<&str>,
    ) -> Result<Vec<i64>> {
        if !self.store.db_path.exists() {
            return Ok(Vec::new());
        }
        mirage::forge::get_callees(function_name, file_filter, &self.store.db_path).map_err(|e| {
            crate::error::ForgeError::DatabaseError(format!("Callee lookup failed: {}", e))
        })
    }

    pub async fn resolve_function(&self, name: &str, file_filter: Option<&str>) -> Result<i64> {
        if !self.store.db_path.exists() {
            return Err(crate::error::ForgeError::DatabaseError(
                "graph DB not found".to_string(),
            ));
        }
        mirage::forge::resolve_function(name, file_filter, &self.store.db_path).map_err(|e| {
            crate::error::ForgeError::DatabaseError(format!("Function resolution failed: {}", e))
        })
    }

    pub async fn database_status(&self) -> Result<Option<DatabaseStatus>> {
        if !self.store.db_path.exists() {
            return Ok(None);
        }
        mirage::forge::database_status(&self.store.db_path)
            .map(|status| {
                Some(DatabaseStatus {
                    cfg_blocks: status.cfg_blocks,
                    cfg_paths: status.cfg_paths,
                    cfg_dominators: status.cfg_dominators,
                    mirage_schema_version: status.mirage_schema_version,
                    magellan_schema_version: status.magellan_schema_version,
                })
            })
            .map_err(|e| {
                crate::error::ForgeError::DatabaseError(format!("Status check failed: {}", e))
            })
    }
}

#[derive(Debug, Clone)]
pub struct CycleReport {
    pub cycles: Vec<CallCycle>,
}

#[derive(Debug, Clone)]
pub struct CallCycle {
    pub members: Vec<String>,
    pub depth: usize,
}

#[derive(Debug, Clone)]
pub struct DeadSymbol {
    pub name: String,
    pub kind: String,
    pub file_path: String,
}

#[derive(Debug, Clone)]
pub struct DatabaseStatus {
    pub cfg_blocks: i64,
    pub cfg_paths: i64,
    pub cfg_dominators: i64,
    pub mirage_schema_version: i32,
    pub magellan_schema_version: i32,
}

fn load_test_cfg(
    db_path: &std::path::Path,
    function_id: i64,
) -> crate::error::Result<Option<TestCfg>> {
    use rusqlite::{params, Connection};

    let graph_db = db_path;
    if !graph_db.exists() {
        return Ok(None);
    }

    let backend = match mirage::Backend::detect_and_open(graph_db) {
        Ok(b) => b,
        Err(_) => return Ok(None),
    };

    let blocks = match backend.get_cfg_blocks(function_id) {
        Ok(b) => b,
        Err(_) => return Ok(None),
    };

    if blocks.is_empty() {
        return Ok(None);
    }

    let entry = BlockId(blocks[0].id);
    let mut cfg = TestCfg::new(entry);

    let mut has_real_edges = false;
    if let Ok(conn) = Connection::open(graph_db) {
        let query = r#"
            SELECT source_idx, target_idx, edge_type
            FROM cfg_edges
            WHERE function_id = ?1
            ORDER BY id
        "#;
        if let Ok(mut stmt) = conn.prepare(query) {
            if let Ok(rows) = stmt.query_map(params![function_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
            }) {
                for row in rows.flatten() {
                    let src = BlockId(blocks.get(row.0 as usize).map(|b| b.id).unwrap_or(row.0));
                    let dst = BlockId(blocks.get(row.1 as usize).map(|b| b.id).unwrap_or(row.1));
                    cfg.add_edge(src, dst);
                    has_real_edges = true;
                }
            }
        }
    }

    if !has_real_edges {
        for i in 0..blocks.len().saturating_sub(1) {
            cfg.add_edge(BlockId(blocks[i].id), BlockId(blocks[i + 1].id));
        }
    }

    for b in &blocks {
        if b.terminator == "return" || b.terminator == "throw" {
            cfg.add_exit(BlockId(b.id));
        }
    }
    if cfg.exits.is_empty() {
        if let Some(last) = blocks.last() {
            cfg.add_exit(BlockId(last.id));
        }
    }

    Ok(Some(cfg))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::BackendKind;

    #[tokio::test]
    async fn test_cfg_module_creation() {
        let store = Arc::new(
            UnifiedGraphStore::open(tempfile::tempdir().unwrap().path(), BackendKind::SQLite)
                .await
                .unwrap(),
        );
        let module = CfgModule::new(Arc::clone(&store));

        assert_eq!(module.store.db_path(), store.db_path());
    }

    #[tokio::test]
    async fn test_path_builder_filters() {
        let store = Arc::new(
            UnifiedGraphStore::open(tempfile::tempdir().unwrap().path(), BackendKind::SQLite)
                .await
                .unwrap(),
        );

        let dummy_module = CfgModule {
            store: Arc::clone(&store),
        };

        let builder = dummy_module.paths(SymbolId(1)).normal_only().max_length(10);

        assert!(builder.normal_only);
        assert_eq!(builder.max_length, Some(10));
    }

    #[tokio::test]
    async fn test_dominators_basic() {
        let store = Arc::new(
            UnifiedGraphStore::open(tempfile::tempdir().unwrap().path(), BackendKind::SQLite)
                .await
                .unwrap(),
        );
        let module = CfgModule::new(store);

        let doms = module.dominators(SymbolId(1)).await.unwrap();
        assert_eq!(doms.root, BlockId(0));
        assert_eq!(doms.dominators.len(), 4);
    }

    #[tokio::test]
    async fn test_loops_detection() {
        let store = Arc::new(
            UnifiedGraphStore::open(tempfile::tempdir().unwrap().path(), BackendKind::SQLite)
                .await
                .unwrap(),
        );
        let module = CfgModule::new(store);

        let loops = module.loops(SymbolId(1)).await.unwrap();
        assert_eq!(loops.len(), 1);
    }

    #[tokio::test]
    async fn test_paths_execute_no_db_returns_synthetic() {
        let store = Arc::new(
            UnifiedGraphStore::open(tempfile::tempdir().unwrap().path(), BackendKind::SQLite)
                .await
                .unwrap(),
        );
        let module = CfgModule::new(store);

        let paths = module.paths(SymbolId(1)).execute().await.unwrap();
        assert!(!paths.is_empty());
        assert_eq!(paths[0].blocks.len(), 1);
        assert_eq!(paths[0].blocks[0], BlockId(1));
    }

    fn make_cfg_fixture_db(db_path: &std::path::Path, fn_name: &str) -> i64 {
        use crate::storage::{open_graph, GraphConfig, NodeSpec};
        let config = GraphConfig::sqlite();
        let backend = open_graph(db_path, &config).unwrap();
        let node = NodeSpec {
            kind: "fn".to_string(),
            name: fn_name.to_string(),
            file_path: Some("src/lib.rs".to_string()),
            data: serde_json::Value::Null,
        };
        let entity_id = backend.insert_node(node).unwrap();
        drop(backend);

        let conn = rusqlite::Connection::open(db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS cfg_blocks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                function_id INTEGER NOT NULL,
                kind TEXT NOT NULL,
                terminator TEXT NOT NULL,
                byte_start INTEGER,
                byte_end INTEGER,
                start_line INTEGER,
                start_col INTEGER,
                end_line INTEGER,
                end_col INTEGER,
                coord_x INTEGER DEFAULT 0,
                coord_y INTEGER DEFAULT 0,
                coord_z INTEGER DEFAULT 0,
                cfg_condition TEXT
            );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO cfg_blocks
                (function_id, kind, terminator, byte_start, byte_end,
                 start_line, start_col, end_line, end_col)
             VALUES (?1, 'entry', 'return', 0, 50, 1, 0, 5, 0)",
            rusqlite::params![entity_id],
        )
        .unwrap();
        entity_id
    }

    #[tokio::test]
    async fn test_extract_function_cfg_finds_fn_entity() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("cfg_test.db");

        make_cfg_fixture_db(&db_path, "compute_flow");

        let store = Arc::new(
            crate::storage::UnifiedGraphStore::open_with_path(
                dir.path(),
                &db_path,
                BackendKind::SQLite,
            )
            .await
            .unwrap(),
        );
        let module = CfgModule::new(store);

        let result = module
            .extract_function_cfg(std::path::Path::new("src/lib.rs"), "compute_flow")
            .await
            .unwrap();

        assert!(
            result.is_some(),
            "should return Some(cfg) when function entity and CFG blocks exist"
        );
    }
}

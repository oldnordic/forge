use crate::error::{ForgeError, Result};
use crate::knowledge::types::{QueryResult, SyncReport};
use crate::knowledge::KnowledgeGraph;

impl KnowledgeGraph {
    pub fn resolve_fts5_by_magellan_id(&self, magellan_id: i64) -> Result<Option<i64>> {
        let conn = rusqlite::Connection::open(&self.db_path)
            .map_err(|e| ForgeError::DatabaseError(format!("Open db failed: {}", e)))?;

        let result = conn
            .query_row(
                "SELECT node_id FROM graph_node_index WHERE magellan_id = ?1",
                rusqlite::params![magellan_id],
                |row| row.get::<_, i64>(0),
            )
            .ok();

        Ok(result)
    }

    pub fn resolve_fts5(&self, keyword: &str) -> Result<Option<i64>> {
        if !self.db_path.exists() {
            return Ok(None);
        }
        let conn = rusqlite::Connection::open(&self.db_path)
            .map_err(|e| ForgeError::DatabaseError(format!("Open db failed: {}", e)))?;
        let pattern = format!("{}*", keyword);
        let magellan_id: Option<i64> = conn
            .query_row(
                "SELECT rowid FROM symbol_fts WHERE symbol_fts MATCH ?1 LIMIT 1",
                rusqlite::params![pattern],
                |row| row.get(0),
            )
            .ok();
        match magellan_id {
            Some(mid) => self.resolve_fts5_by_magellan_id(mid),
            None => Ok(None),
        }
    }

    pub fn insert_bridge_entry(
        &self,
        node_id: i64,
        magellan_id: i64,
        graph_file: &str,
    ) -> Result<()> {
        let conn = rusqlite::Connection::open(&self.db_path)
            .map_err(|e| ForgeError::DatabaseError(format!("Open db failed: {}", e)))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS graph_node_index (
                node_id INTEGER PRIMARY KEY,
                magellan_id INTEGER,
                node_kind TEXT NOT NULL,
                graph_file TEXT NOT NULL
            );",
        )
        .map_err(|e| ForgeError::DatabaseError(format!("Create table failed: {}", e)))?;

        conn.execute(
            "INSERT OR REPLACE INTO graph_node_index (node_id, magellan_id, node_kind, graph_file)
             VALUES (?1, ?2, 'symbol', ?3)",
            rusqlite::params![node_id, magellan_id, graph_file],
        )
        .map_err(|e| ForgeError::DatabaseError(format!("Insert bridge failed: {}", e)))?;

        Ok(())
    }

    pub async fn sync_symbols(&self) -> Result<SyncReport> {
        if !self.db_path.exists() {
            return Ok(SyncReport::default());
        }
        let conn = rusqlite::Connection::open(&self.db_path)
            .map_err(|e| ForgeError::DatabaseError(format!("Open db failed: {}", e)))?;
        let mut stmt =
            match conn.prepare("SELECT id, kind, name, file_path FROM graph_entities LIMIT 5000") {
                Ok(s) => s,
                Err(_) => return Ok(SyncReport::default()),
            };
        let rows: Vec<(i64, String, String, Option<String>)> = stmt
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .map_err(|e| ForgeError::DatabaseError(format!("Query failed: {}", e)))?
            .flatten()
            .collect();
        drop(stmt);
        drop(conn);

        let specs: Vec<sqlitegraph::backend::NodeSpec> = rows
            .iter()
            .map(|(_, kind, name, file_path)| {
                let file = file_path.as_deref().unwrap_or("");
                sqlitegraph::backend::NodeSpec {
                    kind: crate::knowledge::types::node::SYMBOL.to_string(),
                    name: name.clone(),
                    file_path: Some(file.to_string()),
                    data: serde_json::json!({
                        "symbol_kind": kind,
                        "qualified_name": name,
                        "file": file,
                        "line": 0u64,
                        "byte_start": 0u64,
                        "byte_end": 0u64,
                        "language": "unknown",
                    }),
                }
            })
            .collect();

        let kg_ids = self
            .backend
            .insert_nodes_bulk(&specs)
            .map_err(|e| ForgeError::DatabaseError(format!("Bulk node insert failed: {}", e)))?;

        let graph_file = self.graph_path.to_string_lossy().into_owned();
        for ((magellan_id, ..), &kg_id) in rows.iter().zip(kg_ids.iter()) {
            self.insert_bridge_entry(kg_id, *magellan_id, &graph_file)?;
        }

        Ok(SyncReport {
            nodes_added: kg_ids.len(),
            ..Default::default()
        })
    }

    pub async fn sync_references(&self) -> Result<SyncReport> {
        if !self.db_path.exists() {
            return Ok(SyncReport::default());
        }
        let conn = rusqlite::Connection::open(&self.db_path)
            .map_err(|e| ForgeError::DatabaseError(format!("Open db failed: {}", e)))?;
        let mut stmt =
            match conn.prepare("SELECT from_id, to_id, edge_type FROM graph_edges LIMIT 10000") {
                Ok(s) => s,
                Err(_) => return Ok(SyncReport::default()),
            };
        let edges: Vec<(i64, i64, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .map_err(|e| ForgeError::DatabaseError(format!("Query failed: {}", e)))?
            .flatten()
            .collect();
        drop(stmt);
        drop(conn);

        let mut specs = Vec::with_capacity(edges.len());
        for (from_magellan, to_magellan, edge_type) in &edges {
            if let (Some(from_id), Some(to_id)) = (
                self.resolve_fts5_by_magellan_id(*from_magellan)?,
                self.resolve_fts5_by_magellan_id(*to_magellan)?,
            ) {
                specs.push(sqlitegraph::backend::EdgeSpec {
                    from: from_id,
                    to: to_id,
                    edge_type: edge_type.clone(),
                    data: serde_json::Value::Null,
                });
            }
        }

        let edge_ids = self
            .backend
            .insert_edges_bulk(&specs)
            .map_err(|e| ForgeError::DatabaseError(format!("Bulk edge insert failed: {}", e)))?;

        Ok(SyncReport {
            edges_added: edge_ids.len(),
            ..Default::default()
        })
    }

    pub async fn query(&self, keyword: &str, depth: u32) -> Result<QueryResult> {
        let entry_id = self.resolve_fts5(keyword)?;
        let Some(entry_id) = entry_id else {
            return Ok(QueryResult::default());
        };

        let entry_node = self.get_node(entry_id).ok();
        let callers = self.callers_of(entry_id, depth).unwrap_or_default();
        let callees = self.callees_of(entry_id, depth).unwrap_or_default();
        let correlated = self.correlated(entry_id).unwrap_or_default();
        let affected = self.affected_by(entry_id, depth).unwrap_or_default();

        Ok(QueryResult {
            entry_node,
            callers,
            callees,
            correlated,
            affected,
            similar: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::knowledge::KnowledgeGraph;

    fn setup_bridge_table(db_path: &std::path::Path) {
        let conn = rusqlite::Connection::open(db_path).expect("invariant: temp db always opens");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS graph_node_index (
                node_id INTEGER PRIMARY KEY,
                magellan_id INTEGER,
                node_kind TEXT NOT NULL,
                graph_file TEXT NOT NULL
            );",
        )
        .expect("invariant: DDL on fresh db succeeds");
    }

    fn setup_fts5_db(db_path: &std::path::Path, fn_name: &str) -> i64 {
        use sqlitegraph::config::{open_graph, GraphConfig};
        let config = GraphConfig::sqlite();
        let backend = open_graph(db_path, &config).expect("invariant: fresh db always opens");
        let node = sqlitegraph::backend::NodeSpec {
            kind: "fn".to_string(),
            name: fn_name.to_string(),
            file_path: None,
            data: serde_json::Value::Null,
        };
        let entity_id = backend
            .insert_node(node)
            .expect("invariant: fresh backend accepts inserts");
        drop(backend);

        let conn = rusqlite::Connection::open(db_path).expect("invariant: temp db always opens");
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS symbol_fts
             USING fts5(name, content='graph_entities', content_rowid='id');
             INSERT INTO symbol_fts(symbol_fts) VALUES('rebuild');",
        )
        .expect("invariant: DDL on fresh db succeeds");
        entity_id
    }

    fn setup_entities_db(db_path: &std::path::Path, names: &[&str]) -> Vec<i64> {
        use sqlitegraph::backend::NodeSpec;
        use sqlitegraph::config::{open_graph, GraphConfig};
        let config = GraphConfig::sqlite();
        let backend = open_graph(db_path, &config).expect("invariant: fresh db always opens");
        names
            .iter()
            .map(|name| {
                backend
                    .insert_node(NodeSpec {
                        kind: "fn".to_string(),
                        name: name.to_string(),
                        file_path: Some("src/lib.rs".to_string()),
                        data: serde_json::Value::Null,
                    })
                    .expect("invariant: fresh backend accepts inserts")
            })
            .collect()
    }

    #[test]
    fn test_fts5_resolve_empty() {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");
        let db_path = temp.path().join("magellan.db");

        setup_bridge_table(&db_path);

        let kg = KnowledgeGraph::open(&temp.path().join("kg.graph"), &db_path)
            .expect("invariant: fresh temp paths always open");
        let result = kg
            .resolve_fts5("nonexistent")
            .expect("invariant: fts5 lookup succeeds");
        assert!(result.is_none());
    }

    #[test]
    fn test_fts5_resolve_after_populate() {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");
        let db_path = temp.path().join("magellan.db");

        let conn = rusqlite::Connection::open(&db_path).expect("invariant: temp db always opens");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS graph_node_index (
                node_id INTEGER PRIMARY KEY,
                magellan_id INTEGER,
                node_kind TEXT NOT NULL,
                graph_file TEXT NOT NULL
            );
            INSERT INTO graph_node_index (node_id, magellan_id, node_kind, graph_file)
            VALUES (47, 1, 'symbol', 'kg.graph');",
        )
        .expect("invariant: DDL on fresh db succeeds");

        let kg = KnowledgeGraph::open(&temp.path().join("kg.graph"), &db_path)
            .expect("invariant: fresh temp paths always open");
        let node_id = kg
            .resolve_fts5_by_magellan_id(1)
            .expect("invariant: bridge lookup succeeds");
        assert_eq!(node_id, Some(47));
    }

    #[tokio::test]
    async fn test_sync_symbols_empty_db() {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");
        let db_path = temp.path().join("magellan.db");

        setup_bridge_table(&db_path);

        let kg = KnowledgeGraph::open(&temp.path().join("kg.graph"), &db_path)
            .expect("invariant: fresh temp paths always open");
        let report = kg
            .sync_symbols()
            .await
            .expect("invariant: sync on valid db succeeds");
        assert_eq!(report.nodes_added, 0);
    }

    #[tokio::test]
    async fn test_query_no_results() {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");
        let db_path = temp.path().join("magellan.db");

        setup_bridge_table(&db_path);

        let kg = KnowledgeGraph::open(&temp.path().join("kg.graph"), &db_path)
            .expect("invariant: fresh temp paths always open");
        let result = kg
            .query("nonexistent", 3)
            .await
            .expect("invariant: query on valid graph succeeds");
        assert!(result.entry_node.is_none());
        assert!(result.callers.is_empty());
    }

    #[test]
    fn test_query_traverse_from_bridge_entry() {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");
        let db_path = temp.path().join("magellan.db");

        setup_bridge_table(&db_path);

        let kg = KnowledgeGraph::open(&temp.path().join("kg.graph"), &db_path)
            .expect("invariant: fresh temp paths always open");

        let sym_id = kg
            .add_symbol(
                "my_func",
                "Function",
                "a::my_func",
                "f.rs",
                1,
                0,
                10,
                "Rust",
                None,
            )
            .expect("invariant: fresh graph accepts inserts");
        let caller_id = kg
            .add_symbol(
                "caller",
                "Function",
                "a::caller",
                "f.rs",
                5,
                0,
                10,
                "Rust",
                None,
            )
            .expect("invariant: fresh graph accepts inserts");
        kg.add_edge(caller_id, sym_id, "calls", serde_json::json!({}))
            .expect("invariant: fresh graph accepts edge inserts");

        kg.insert_bridge_entry(sym_id, 1, "kg.graph")
            .expect("invariant: bridge insert on fresh db succeeds");

        let entry = kg
            .resolve_fts5_by_magellan_id(1)
            .expect("invariant: bridge lookup succeeds");
        assert_eq!(entry, Some(sym_id));

        let callers = kg
            .callers_of(sym_id, 1)
            .expect("invariant: traversal on known graph succeeds");
        assert_eq!(callers.len(), 1);
        assert_eq!(callers[0].name, "caller");
    }

    #[test]
    fn test_resolve_fts5_finds_indexed_symbol() {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");
        let graph_path = temp.path().join("kg.graph");
        let db_path = temp.path().join("magellan.db");

        let magellan_id = setup_fts5_db(&db_path, "unique_resolve_target");
        let kg = KnowledgeGraph::open(&graph_path, &db_path)
            .expect("invariant: fresh temp paths always open");

        let sym_id = kg
            .add_symbol(
                "unique_resolve_target",
                "Function",
                "crate::unique_resolve_target",
                "src/lib.rs",
                1,
                0,
                10,
                "Rust",
                None,
            )
            .expect("invariant: fresh graph accepts inserts");
        kg.insert_bridge_entry(sym_id, magellan_id, "kg.graph")
            .expect("invariant: bridge insert on fresh db succeeds");

        let result = kg
            .resolve_fts5("unique_resolve_target")
            .expect("invariant: fts5 lookup succeeds");
        assert!(
            result.is_some(),
            "resolve_fts5 should find node via FTS5 index and bridge"
        );
        assert_eq!(result, Some(sym_id));
    }

    #[tokio::test]
    async fn test_sync_symbols_inserts_entities() {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");
        let db_path = temp.path().join("magellan.db");
        setup_entities_db(&db_path, &["sync_fn_one", "sync_fn_two"]);
        let kg = KnowledgeGraph::open(&temp.path().join("kg.graph"), &db_path)
            .expect("invariant: fresh temp paths always open");
        let report = kg
            .sync_symbols()
            .await
            .expect("invariant: sync on valid db succeeds");
        assert_eq!(
            report.nodes_added, 2,
            "sync_symbols should add one KG node per magellan entity"
        );
    }

    #[tokio::test]
    async fn test_sync_symbols_bulk_all_bridge_entries_accessible() {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");
        let db_path = temp.path().join("magellan.db");
        let names = ["alpha", "beta", "gamma", "delta", "epsilon"];
        let magellan_ids = setup_entities_db(&db_path, &names);

        let kg = KnowledgeGraph::open(&temp.path().join("kg.graph"), &db_path)
            .expect("invariant: fresh temp paths always open");
        let report = kg
            .sync_symbols()
            .await
            .expect("invariant: sync on valid db succeeds");
        assert_eq!(report.nodes_added, 5);

        for mid in &magellan_ids {
            let node_id = kg
                .resolve_fts5_by_magellan_id(*mid)
                .expect("invariant: bridge lookup succeeds");
            assert!(
                node_id.is_some(),
                "bridge entry missing for magellan_id={mid}"
            );
        }
    }

    #[tokio::test]
    async fn test_sync_references_inserts_edges() {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");
        let db_path = temp.path().join("magellan.db");
        let ids = setup_entities_db(&db_path, &["caller_fn", "callee_fn"]);
        {
            let conn =
                rusqlite::Connection::open(&db_path).expect("invariant: temp db always opens");
            conn.execute(
                "INSERT INTO graph_edges (from_id, to_id, edge_type, data) VALUES (?1, ?2, 'calls', '{}')",
                rusqlite::params![ids[0], ids[1]],
            )
            .expect("invariant: DML on fresh db succeeds");
        }
        let kg = KnowledgeGraph::open(&temp.path().join("kg.graph"), &db_path)
            .expect("invariant: fresh temp paths always open");
        kg.sync_symbols()
            .await
            .expect("invariant: sync on valid db succeeds");
        let ref_report = kg
            .sync_references()
            .await
            .expect("invariant: sync on valid db succeeds");
        assert_eq!(
            ref_report.edges_added, 1,
            "sync_references should add one KG edge per magellan graph_edge"
        );
    }
}

//! Knowledge graph — sqlitegraph native-v3 backed graph for code intelligence.

pub mod types;

pub use types::*;

use std::path::{Path, PathBuf};

use crate::error::{ForgeError, Result};
use sqlitegraph::backend::GraphBackend;
use sqlitegraph::config::{open_graph, GraphConfig};

/// Knowledge graph backed by sqlitegraph native-v3.
///
/// Stores typed nodes (symbols, files, discoveries, patterns, issues,
/// CFG blocks, hotspots, knowledge entries) and typed edges (calls,
/// correlates, affects, flows_to, etc.) in a `.graph` binary file.
pub struct KnowledgeGraph {
    backend: Box<dyn GraphBackend>,
    graph_path: PathBuf,
    db_path: PathBuf,
}

impl KnowledgeGraph {
    /// Opens or creates a knowledge graph.
    ///
    /// The `.graph` file is created at `graph_path` using sqlitegraph
    /// native-v3 backend. The `db_path` points to the Magellan `.db`
    /// for FTS5 bridge lookups.
    pub fn open(graph_path: &Path, db_path: &Path) -> Result<Self> {
        if let Some(parent) = graph_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ForgeError::DatabaseError(format!(
                    "Failed to create graph directory: {}",
                    e
                ))
            })?;
        }

        let config = GraphConfig::native();
        let backend = open_graph(graph_path, &config).map_err(|e| {
            ForgeError::DatabaseError(format!("Failed to open knowledge graph: {}", e))
        })?;

        Ok(Self {
            backend,
            graph_path: graph_path.to_path_buf(),
            db_path: db_path.to_path_buf(),
        })
    }

    /// Returns the path to the .graph file.
    pub fn graph_path(&self) -> &Path {
        &self.graph_path
    }

    /// Returns the path to the Magellan .db.
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    // -- Internal helpers --

    fn snapshot() -> sqlitegraph::snapshot::SnapshotId {
        sqlitegraph::snapshot::SnapshotId::current()
    }

    fn insert_node(
        &self,
        kind: &str,
        name: &str,
        file_path: Option<&str>,
        data: serde_json::Value,
    ) -> Result<i64> {
        let spec = sqlitegraph::backend::NodeSpec {
            kind: kind.to_string(),
            name: name.to_string(),
            file_path: file_path.map(|s| s.to_string()),
            data,
        };
        self.backend.insert_node(spec).map_err(|e| {
            ForgeError::DatabaseError(format!("Insert node failed: {}", e))
        })
    }

    // -- Node CRUD --

    /// Retrieves a node by ID.
    pub fn get_node(&self, node_id: i64) -> Result<GraphNode> {
        let entity = self.backend.get_node(Self::snapshot(), node_id).map_err(
            |e| ForgeError::DatabaseError(format!("Node not found: {}", e)),
        )?;
        Ok(GraphNode {
            id: node_id,
            kind: entity.kind,
            name: entity.name,
            file_path: entity.file_path,
            data: entity.data,
        })
    }

    /// Finds all nodes of a given kind.
    pub fn find_nodes_by_kind(&self, kind: &str) -> Result<Vec<GraphNode>> {
        let snap = Self::snapshot();
        let ids = self.backend.entity_ids().map_err(|e| {
            ForgeError::DatabaseError(format!("Entity list failed: {}", e))
        })?;
        let mut results = Vec::new();
        for id in ids {
            if let Ok(entity) = self.backend.get_node(snap, id) {
                if entity.kind == kind {
                    results.push(GraphNode {
                        id,
                        kind: entity.kind,
                        name: entity.name,
                        file_path: entity.file_path,
                        data: entity.data,
                    });
                }
            }
        }
        Ok(results)
    }

    /// Adds a symbol node.
    pub fn add_symbol(
        &self,
        name: &str,
        symbol_kind: &str,
        qualified_name: &str,
        file: &str,
        line: usize,
        byte_start: u32,
        byte_end: u32,
        language: &str,
        parent_id: Option<i64>,
    ) -> Result<i64> {
        let mut data = serde_json::json!({
            "symbol_kind": symbol_kind,
            "qualified_name": qualified_name,
            "file": file,
            "line": line,
            "byte_start": byte_start,
            "byte_end": byte_end,
            "language": language,
        });
        if let Some(pid) = parent_id {
            data["parent_id"] = serde_json::json!(pid);
        }
        self.insert_node(types::node::SYMBOL, name, Some(file), data)
    }

    /// Adds a file node.
    pub fn add_file(&self, path: &str, language: &str, hash: &str) -> Result<i64> {
        let data = serde_json::json!({
            "path": path,
            "language": language,
            "hash": hash,
        });
        self.insert_node(types::node::FILE, path, None, data)
    }

    /// Adds a discovery node.
    pub fn add_discovery(
        &self,
        agent: &str,
        discovery_type: &str,
        target: &str,
        metadata: serde_json::Value,
    ) -> Result<i64> {
        let data = serde_json::json!({
            "discovery_type": discovery_type,
            "agent": agent,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "metadata": metadata,
        });
        self.insert_node(types::node::DISCOVERY, target, None, data)
    }

    /// Adds an issue node.
    pub fn add_issue(
        &self,
        severity: &str,
        description: &str,
        rule_id: Option<&str>,
    ) -> Result<i64> {
        let mut data =
            serde_json::json!({"severity": severity, "description": description,});
        if let Some(rid) = rule_id {
            data["rule_id"] = serde_json::json!(rid);
        }
        self.insert_node(types::node::ISSUE, description, None, data)
    }

    /// Adds a pattern node.
    pub fn add_pattern(
        &self,
        pattern_type: &str,
        confidence: f64,
        description: &str,
    ) -> Result<i64> {
        let data = serde_json::json!({
            "pattern_type": pattern_type,
            "confidence": confidence,
            "description": description,
        });
        self.insert_node(types::node::PATTERN, pattern_type, None, data)
    }

    /// Adds a knowledge node.
    pub fn add_knowledge(
        &self,
        source: &str,
        title: &str,
        tags: &[String],
        summary: &str,
    ) -> Result<i64> {
        let data = serde_json::json!({
            "source": source,
            "title": title,
            "tags": tags,
            "summary": summary,
        });
        self.insert_node(types::node::KNOWLEDGE, title, None, data)
    }

    /// Adds a hotspot node.
    pub fn add_hotspot(
        &self,
        complexity: u32,
        risk_score: f64,
        loop_depth: u32,
        description: &str,
    ) -> Result<i64> {
        let data = serde_json::json!({
            "complexity": complexity,
            "risk_score": risk_score,
            "loop_depth": loop_depth,
            "description": description,
        });
        self.insert_node(types::node::HOTSPOT, description, None, data)
    }

    /// Adds a CFG block node.
    pub fn add_cfg_block(&self, function_id: i64, block: &CfgBlockData) -> Result<i64> {
        let data = serde_json::json!({
            "function_id": function_id,
            "start_byte": block.start_byte,
            "end_byte": block.end_byte,
            "block_kind": block.block_kind,
            "is_error": block.is_error,
        });
        self.insert_node(
            types::node::CFG_BLOCK,
            &format!("block_{}", block.start_byte),
            None,
            data,
        )
    }

    // -- Edge operations --

    /// Adds an edge between two nodes.
    pub fn add_edge(
        &self,
        from: i64,
        to: i64,
        edge_type: &str,
        data: serde_json::Value,
    ) -> Result<i64> {
        let spec = sqlitegraph::backend::EdgeSpec {
            from,
            to,
            edge_type: edge_type.to_string(),
            data,
        };
        self.backend.insert_edge(spec).map_err(|e| {
            ForgeError::DatabaseError(format!("Insert edge failed: {}", e))
        })
    }

    /// Adds a bidirectional correlation between two nodes.
    pub fn add_correlation(&self, from: i64, to: i64, confidence: f64, agent: &str) -> Result<()> {
        let data = serde_json::json!({"confidence": confidence, "agent": agent,});
        self.add_edge(from, to, types::edge::CORRELATES, data.clone())?;
        self.add_edge(to, from, types::edge::CORRELATES, data)?;
        Ok(())
    }

    /// Returns neighbors of a node filtered by edge type and direction.
    // -- Traversal --

    /// Finds all symbols that call the given symbol (incoming `calls` edges).
    pub fn callers_of(&self, symbol_id: i64, max_depth: u32) -> Result<Vec<GraphNode>> {
        self.bfs_by_edge_type(symbol_id, types::edge::CALLS, Direction::Incoming, max_depth)
    }

    /// Finds all symbols called by the given symbol (outgoing `calls` edges).
    pub fn callees_of(&self, symbol_id: i64, max_depth: u32) -> Result<Vec<GraphNode>> {
        self.bfs_by_edge_type(symbol_id, types::edge::CALLS, Direction::Outgoing, max_depth)
    }

    /// Finds all nodes correlated with the given node (bidirectional `correlates` edges).
    pub fn correlated(&self, node_id: i64) -> Result<Vec<GraphNode>> {
        let incoming = self.neighbors(node_id, types::edge::CORRELATES, Direction::Incoming)?;
        let outgoing = self.neighbors(node_id, types::edge::CORRELATES, Direction::Outgoing)?;
        let mut seen = std::collections::HashSet::new();
        let mut results = Vec::new();
        for node in incoming.into_iter().chain(outgoing) {
            if seen.insert(node.id) {
                results.push(node);
            }
        }
        Ok(results)
    }

    /// Finds all symbols affected by issues (incoming `affects` edges, BFS to depth).
    pub fn affected_by(&self, symbol_id: i64, depth: u32) -> Result<Vec<GraphNode>> {
        self.bfs_by_edge_type(symbol_id, types::edge::AFFECTS, Direction::Incoming, depth)
    }

    fn bfs_by_edge_type(
        &self,
        start: i64,
        edge_type: &str,
        direction: Direction,
        max_depth: u32,
    ) -> Result<Vec<GraphNode>> {
        let snap = Self::snapshot();
        let dir = match direction {
            Direction::Incoming => sqlitegraph::backend::BackendDirection::Incoming,
            Direction::Outgoing => sqlitegraph::backend::BackendDirection::Outgoing,
        };
        let mut visited = std::collections::HashSet::new();
        visited.insert(start);
        let mut results = Vec::new();
        let mut frontier: Vec<i64> = vec![start];

        for _ in 0..max_depth {
            let mut next_frontier = Vec::new();
            for node_id in &frontier {
                let query = sqlitegraph::backend::NeighborQuery {
                    direction: dir,
                    edge_type: Some(edge_type.to_string()),
                };
                if let Ok(neighbor_ids) = self.backend.neighbors(snap, *node_id, query) {
                    for nid in neighbor_ids {
                        if visited.insert(nid) {
                            next_frontier.push(nid);
                            if let Ok(entity) = self.backend.get_node(snap, nid) {
                                results.push(GraphNode {
                                    id: nid,
                                    kind: entity.kind,
                                    name: entity.name,
                                    file_path: entity.file_path,
                                    data: entity.data,
                                });
                            }
                        }
                    }
                }
            }
            frontier = next_frontier;
        }
        Ok(results)
    }

    /// Returns neighbors of a node filtered by edge type and direction.
    pub fn neighbors(
        &self,
        node_id: i64,
        edge_type: &str,
        direction: Direction,
    ) -> Result<Vec<GraphNode>> {
        let snap = Self::snapshot();
        let dir = match direction {
            Direction::Incoming => sqlitegraph::backend::BackendDirection::Incoming,
            Direction::Outgoing => sqlitegraph::backend::BackendDirection::Outgoing,
        };
        let query = sqlitegraph::backend::NeighborQuery {
            direction: dir,
            edge_type: Some(edge_type.to_string()),
        };
        let neighbor_ids = self
            .backend
            .neighbors(snap, node_id, query)
            .map_err(|e| ForgeError::DatabaseError(format!("Neighbor query failed: {}", e)))?;

        let mut results = Vec::new();
        for nid in neighbor_ids {
            if let Ok(entity) = self.backend.get_node(snap, nid) {
                results.push(GraphNode {
                    id: nid,
                    kind: entity.kind,
                    name: entity.name,
                    file_path: entity.file_path,
                    data: entity.data,
                });
            }
        }
        Ok(results)
    }

    // -- Graph algorithms --

    /// Finds the shortest path between two nodes.
    pub fn shortest_path(&self, from: i64, to: i64) -> Result<Option<Vec<i64>>> {
        self.backend
            .shortest_path(Self::snapshot(), from, to)
            .map_err(|e| ForgeError::DatabaseError(format!("Shortest path failed: {}", e)))
    }

    /// Returns all nodes reachable from the given node via BFS.
    pub fn reachability(&self, from: i64) -> Result<Vec<i64>> {
        self.backend
            .bfs(Self::snapshot(), from, 100)
            .map_err(|e| ForgeError::DatabaseError(format!("Reachability failed: {}", e)))
    }

    /// K-hop traversal returning node IDs within `depth` hops.
    pub fn k_hop(&self, from: i64, depth: u32, direction: Direction) -> Result<Vec<i64>> {
        let dir = match direction {
            Direction::Incoming => sqlitegraph::backend::BackendDirection::Incoming,
            Direction::Outgoing => sqlitegraph::backend::BackendDirection::Outgoing,
        };
        self.backend
            .k_hop(Self::snapshot(), from, depth, dir)
            .map_err(|e| ForgeError::DatabaseError(format!("K-hop failed: {}", e)))
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_knowledge_graph_open_creates_file() {
        let temp = tempfile::tempdir().unwrap();
        let graph_path = temp.path().join("knowledge.graph");
        let db_path = temp.path().join("magellan.db");

        let kg = KnowledgeGraph::open(&graph_path, &db_path).unwrap();

        assert!(graph_path.exists());
        assert_eq!(kg.graph_path(), graph_path);
        assert_eq!(kg.db_path(), db_path);
    }

    #[test]
    fn test_knowledge_graph_open_creates_parent_dirs() {
        let temp = tempfile::tempdir().unwrap();
        let graph_path = temp.path().join("nested").join("dir").join("kg.graph");
        let db_path = temp.path().join("magellan.db");

        let kg = KnowledgeGraph::open(&graph_path, &db_path).unwrap();
        assert!(graph_path.exists());
    }

    #[test]
    fn test_knowledge_graph_open_existing_file() {
        let temp = tempfile::tempdir().unwrap();
        let graph_path = temp.path().join("kg.graph");
        let db_path = temp.path().join("magellan.db");

        {
            let _kg = KnowledgeGraph::open(&graph_path, &db_path).unwrap();
        }

        let _kg2 = KnowledgeGraph::open(&graph_path, &db_path).unwrap();
        assert!(graph_path.exists());
    }

    // -- Node CRUD tests --

    fn open_kg() -> (tempfile::TempDir, KnowledgeGraph) {
        let temp = tempfile::tempdir().unwrap();
        let kg = KnowledgeGraph::open(
            &temp.path().join("kg.graph"),
            &temp.path().join("magellan.db"),
        )
        .unwrap();
        (temp, kg)
    }

    #[test]
    fn test_add_symbol_node() {
        let (_temp, kg) = open_kg();
        let id = kg
            .add_symbol(
                "my_func",
                "Function",
                "crate::module::my_func",
                "src/lib.rs",
                42,
                100,
                200,
                "Rust",
                None,
            )
            .unwrap();
        assert!(id > 0);
        let node = kg.get_node(id).unwrap();
        assert_eq!(node.kind, "symbol");
        assert_eq!(node.name, "my_func");
        assert_eq!(node.prop_str("symbol_kind"), Some("Function"));
        assert_eq!(node.prop_str("qualified_name"), Some("crate::module::my_func"));
        assert_eq!(node.prop_str("file"), Some("src/lib.rs"));
        assert_eq!(node.prop_u64("line"), Some(42));
    }

    #[test]
    fn test_add_file_node() {
        let (_temp, kg) = open_kg();
        let id = kg.add_file("src/lib.rs", "Rust", "abc123").unwrap();
        let node = kg.get_node(id).unwrap();
        assert_eq!(node.kind, "file");
        assert_eq!(node.name, "src/lib.rs");
        assert_eq!(node.prop_str("language"), Some("Rust"));
        assert_eq!(node.prop_str("hash"), Some("abc123"));
    }

    #[test]
    fn test_add_discovery_node() {
        let (_temp, kg) = open_kg();
        let id = kg
            .add_discovery(
                "claude1",
                "Symbol",
                "my_func",
                serde_json::json!({"complexity": 8}),
            )
            .unwrap();
        let node = kg.get_node(id).unwrap();
        assert_eq!(node.kind, "discovery");
        assert_eq!(node.name, "my_func");
        assert_eq!(node.prop_str("agent"), Some("claude1"));
        assert_eq!(node.prop_str("discovery_type"), Some("Symbol"));
    }

    #[test]
    fn test_add_issue_node() {
        let (_temp, kg) = open_kg();
        let id = kg
            .add_issue("high", "unwrap in production code", Some("M001"))
            .unwrap();
        let node = kg.get_node(id).unwrap();
        assert_eq!(node.kind, "issue");
        assert_eq!(node.prop_str("severity"), Some("high"));
        assert_eq!(node.prop_str("rule_id"), Some("M001"));
    }

    #[test]
    fn test_add_pattern_node() {
        let (_temp, kg) = open_kg();
        let id = kg.add_pattern("builder", 0.92, "builder pattern detected").unwrap();
        let node = kg.get_node(id).unwrap();
        assert_eq!(node.kind, "pattern");
        assert_eq!(node.prop_str("pattern_type"), Some("builder"));
        assert_eq!(node.prop_f64("confidence"), Some(0.92));
    }

    #[test]
    fn test_add_knowledge_node() {
        let (_temp, kg) = open_kg();
        let tags = vec!["auth".to_string(), "middleware".to_string()];
        let id = kg
            .add_knowledge("wiki", "Auth Architecture", &tags, "Overview of auth flow")
            .unwrap();
        let node = kg.get_node(id).unwrap();
        assert_eq!(node.kind, "knowledge");
        assert_eq!(node.prop_str("source"), Some("wiki"));
        assert_eq!(node.prop_str("title"), Some("Auth Architecture"));
    }

    #[test]
    fn test_add_hotspot_node() {
        let (_temp, kg) = open_kg();
        let id = kg.add_hotspot(15, 0.85, 3, "high complexity loop").unwrap();
        let node = kg.get_node(id).unwrap();
        assert_eq!(node.kind, "hotspot");
        assert_eq!(node.prop_u64("complexity"), Some(15));
        assert_eq!(node.prop_f64("risk_score"), Some(0.85));
    }

    #[test]
    fn test_add_cfg_block_node() {
        let (_temp, kg) = open_kg();
        let block = CfgBlockData {
            start_byte: 100,
            end_byte: 200,
            block_kind: "Basic".to_string(),
            is_error: false,
        };
        let id = kg.add_cfg_block(42, &block).unwrap();
        let node = kg.get_node(id).unwrap();
        assert_eq!(node.kind, "cfg_block");
        assert_eq!(node.prop_u64("function_id"), Some(42));
        assert_eq!(node.prop_str("block_kind"), Some("Basic"));
    }

    #[test]
    fn test_find_nodes_by_kind() {
        let (_temp, kg) = open_kg();
        kg.add_symbol("func_a", "Function", "a", "f.rs", 1, 0, 10, "Rust", None)
            .unwrap();
        kg.add_symbol("func_b", "Function", "b", "f.rs", 2, 0, 10, "Rust", None)
            .unwrap();
        kg.add_file("f.rs", "Rust", "hash").unwrap();

        let symbols = kg.find_nodes_by_kind("symbol").unwrap();
        assert_eq!(symbols.len(), 2);
        let files = kg.find_nodes_by_kind("file").unwrap();
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn test_get_node_not_found() {
        let (_temp, kg) = open_kg();
        let result = kg.get_node(99999);
        assert!(result.is_err());
    }

    // -- Edge tests --

    #[test]
    fn test_add_edge() {
        let (_temp, kg) = open_kg();
        let a = kg.add_symbol("func_a", "Function", "a", "f.rs", 1, 0, 10, "Rust", None).unwrap();
        let b = kg.add_symbol("func_b", "Function", "b", "f.rs", 2, 0, 10, "Rust", None).unwrap();

        let edge_id = kg.add_edge(a, b, "calls", serde_json::json!({"location_line": 5})).unwrap();
        assert!(edge_id > 0);
    }

    #[test]
    fn test_add_correlation_bidirectional() {
        let (_temp, kg) = open_kg();
        let sym = kg.add_symbol("my_func", "Function", "a", "f.rs", 1, 0, 10, "Rust", None).unwrap();
        let disc = kg.add_discovery("claude1", "Symbol", "my_func", serde_json::json!({})).unwrap();

        kg.add_correlation(disc, sym, 0.95, "claude1").unwrap();

        // Outgoing: discovery -> symbol
        let outgoing = kg.neighbors(disc, "correlates", Direction::Outgoing).unwrap();
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].name, "my_func");

        // Incoming: symbol <- discovery
        let incoming = kg.neighbors(sym, "correlates", Direction::Incoming).unwrap();
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].name, "my_func");
    }

    // -- Traversal tests --

    #[test]
    fn test_callers_of() {
        let (_temp, kg) = open_kg();
        let target = kg.add_symbol("target_func", "Function", "t", "f.rs", 1, 0, 10, "Rust", None).unwrap();
        let caller_a = kg.add_symbol("caller_a", "Function", "a", "f.rs", 5, 0, 10, "Rust", None).unwrap();
        let caller_b = kg.add_symbol("caller_b", "Function", "b", "f.rs", 10, 0, 10, "Rust", None).unwrap();

        kg.add_edge(caller_a, target, "calls", serde_json::json!({})).unwrap();
        kg.add_edge(caller_b, target, "calls", serde_json::json!({})).unwrap();

        let callers = kg.callers_of(target, 1).unwrap();
        assert_eq!(callers.len(), 2);
    }

    #[test]
    fn test_callees_of() {
        let (_temp, kg) = open_kg();
        let func = kg.add_symbol("func", "Function", "f", "f.rs", 1, 0, 10, "Rust", None).unwrap();
        let callee_a = kg.add_symbol("callee_a", "Function", "a", "f.rs", 5, 0, 10, "Rust", None).unwrap();
        let callee_b = kg.add_symbol("callee_b", "Function", "b", "f.rs", 10, 0, 10, "Rust", None).unwrap();

        kg.add_edge(func, callee_a, "calls", serde_json::json!({})).unwrap();
        kg.add_edge(func, callee_b, "calls", serde_json::json!({})).unwrap();

        let callees = kg.callees_of(func, 1).unwrap();
        assert_eq!(callees.len(), 2);
    }

    #[test]
    fn test_correlated_nodes() {
        let (_temp, kg) = open_kg();
        let sym = kg.add_symbol("my_func", "Function", "a", "f.rs", 1, 0, 10, "Rust", None).unwrap();
        let disc1 = kg.add_discovery("agent1", "Symbol", "my_func", serde_json::json!({})).unwrap();
        let disc2 = kg.add_discovery("agent2", "CFG", "my_func", serde_json::json!({})).unwrap();

        kg.add_correlation(disc1, sym, 0.9, "agent1").unwrap();
        kg.add_correlation(disc2, sym, 0.8, "agent2").unwrap();

        let correlated = kg.correlated(sym).unwrap();
        assert_eq!(correlated.len(), 2);
    }

    #[test]
    fn test_affected_by() {
        let (_temp, kg) = open_kg();
        let sym = kg.add_symbol("process_payment", "Function", "a", "f.rs", 1, 0, 10, "Rust", None).unwrap();
        let issue = kg.add_issue("high", "race condition", None).unwrap();

        kg.add_edge(issue, sym, "affects", serde_json::json!({})).unwrap();

        let affected = kg.affected_by(sym, 1).unwrap();
        assert_eq!(affected.len(), 1);
    }

    // -- Graph algorithm tests --

    #[test]
    fn test_shortest_path() {
        let (_temp, kg) = open_kg();
        let a = kg.add_symbol("a", "Function", "a", "f.rs", 1, 0, 10, "Rust", None).unwrap();
        let b = kg.add_symbol("b", "Function", "b", "f.rs", 2, 0, 10, "Rust", None).unwrap();
        let c = kg.add_symbol("c", "Function", "c", "f.rs", 3, 0, 10, "Rust", None).unwrap();

        kg.add_edge(a, b, "calls", serde_json::json!({})).unwrap();
        kg.add_edge(b, c, "calls", serde_json::json!({})).unwrap();

        let path = kg.shortest_path(a, c).unwrap();
        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.contains(&a));
        assert!(path.contains(&c));
    }

    #[test]
    fn test_reachability() {
        let (_temp, kg) = open_kg();
        let a = kg.add_symbol("a", "Function", "a", "f.rs", 1, 0, 10, "Rust", None).unwrap();
        let b = kg.add_symbol("b", "Function", "b", "f.rs", 2, 0, 10, "Rust", None).unwrap();
        let c = kg.add_symbol("c", "Function", "c", "f.rs", 3, 0, 10, "Rust", None).unwrap();

        kg.add_edge(a, b, "calls", serde_json::json!({})).unwrap();
        kg.add_edge(b, c, "calls", serde_json::json!({})).unwrap();

        let reachable = kg.reachability(a).unwrap();
        assert!(reachable.contains(&b));
        assert!(reachable.contains(&c));
    }

    #[test]
    fn test_k_hop() {
        let (_temp, kg) = open_kg();
        let a = kg.add_symbol("a", "Function", "a", "f.rs", 1, 0, 10, "Rust", None).unwrap();
        let b = kg.add_symbol("b", "Function", "b", "f.rs", 2, 0, 10, "Rust", None).unwrap();
        let c = kg.add_symbol("c", "Function", "c", "f.rs", 3, 0, 10, "Rust", None).unwrap();

        kg.add_edge(a, b, "calls", serde_json::json!({})).unwrap();
        kg.add_edge(b, c, "calls", serde_json::json!({})).unwrap();

        let hop1 = kg.k_hop(a, 1, Direction::Outgoing).unwrap();
        assert!(hop1.contains(&b));

        let hop2 = kg.k_hop(a, 2, Direction::Outgoing).unwrap();
        assert!(hop2.contains(&c));
    }
}

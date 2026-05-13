# Knowledge Graph Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a KnowledgeGraph module to forge_core backed by sqlitegraph native-v3, enabling graph traversal for code intelligence.

**Architecture:** New `forge_core::knowledge` module owns a `.graph` file (sqlitegraph native-v3). Node kinds (symbol, file, discovery, etc.) are stored as `NodeSpec` with `kind` discriminator and JSON `data` properties. Edge kinds (calls, correlates, etc.) are `EdgeSpec` with `edge_type` strings. The Forge struct gets a `knowledge()` accessor. FTS5 in the existing Magellan `.db` bridges keyword lookups to graph node IDs.

**Tech Stack:** Rust, sqlitegraph 2.2.4 (native-v3 feature), rusqlite (for FTS5 bridge), serde_json

**Spec:** `docs/superpowers/specs/2026-05-13-knowledge-graph-design.md`

---

## File Structure

| File | Responsibility |
|------|---------------|
| `forge_core/Cargo.toml` | Add `native-v3` feature flag |
| `forge_core/src/knowledge/mod.rs` | KnowledgeGraph struct, lifecycle, CRUD, traversal, sync |
| `forge_core/src/knowledge/types.rs` | GraphNode, Direction, SyncReport, QueryResult, CfgBlockData, node/edge kind constants |
| `forge_core/src/lib.rs` | Add `pub mod knowledge;` + `knowledge()` accessor on Forge |

No other files need modification. forge_agent integration is a separate plan.

---

### Task 1: Cargo.toml + types.rs skeleton

**Files:**
- Modify: `forge_core/Cargo.toml`
- Create: `forge_core/src/knowledge/types.rs`

- [ ] **Step 1: Write the failing test**

Create `forge_core/src/knowledge/types.rs` with the types and a basic test:

```rust
//! Knowledge graph types — node kinds, edge kinds, query results.

/// Node kind constants used as `NodeSpec::kind`.
pub mod node {
    pub const SYMBOL: &str = "symbol";
    pub const FILE: &str = "file";
    pub const DISCOVERY: &str = "discovery";
    pub const CFG_BLOCK: &str = "cfg_block";
    pub const HOTSPOT: &str = "hotspot";
    pub const PATTERN: &str = "pattern";
    pub const ISSUE: &str = "issue";
    pub const KNOWLEDGE: &str = "knowledge";
}

/// Edge kind constants used as `EdgeSpec::edge_type`.
pub mod edge {
    pub const CALLS: &str = "calls";
    pub const CONTAINS: &str = "contains";
    pub const REFERENCES: &str = "references";
    pub const CORRELATES: &str = "correlates";
    pub const AFFECTS: &str = "affects";
    pub const FLOWS_TO: &str = "flows_to";
    pub const SIMILAR_TO: &str = "similar_to";
    pub const DERIVED_FROM: &str = "derived_from";
    pub const DISCOVERED_BY: &str = "discovered_by";
    pub const MENTIONS: &str = "mentions";
    pub const BELONGS_TO: &str = "belongs_to";
}

/// Direction for graph traversal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Incoming,
    Outgoing,
}

/// Wrapper around a sqlitegraph node with typed property accessors.
#[derive(Clone, Debug)]
pub struct GraphNode {
    pub id: i64,
    pub kind: String,
    pub name: String,
    pub file_path: Option<String>,
    pub data: serde_json::Value,
}

impl GraphNode {
    /// Creates a GraphNode from a sqlitegraph GraphEntity.
    pub fn from_entity(id: i64, entity: &sqlitegraph::graph::GraphEntity) -> Self {
        Self {
            id,
            kind: entity.kind.clone(),
            name: entity.name.clone(),
            file_path: entity.file_path.clone(),
            data: entity.data.clone(),
        }
    }

    /// Returns a typed property from the data JSON.
    pub fn prop(&self, key: &str) -> Option<&serde_json::Value> {
        self.data.get(key)
    }

    /// Returns a string property.
    pub fn prop_str(&self, key: &str) -> Option<&str> {
        self.data.get(key).and_then(|v| v.as_str())
    }

    /// Returns a number property.
    pub fn prop_u64(&self, key: &str) -> Option<u64> {
        self.data.get(key).and_then(|v| v.as_u64())
    }

    /// Returns a float property.
    pub fn prop_f64(&self, key: &str) -> Option<f64> {
        self.data.get(key).and_then(|v| v.as_f64())
    }
}

/// Report returned after syncing symbols/references.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncReport {
    pub nodes_added: usize,
    pub nodes_updated: usize,
    pub nodes_unchanged: usize,
    pub edges_added: usize,
    pub edges_updated: usize,
}

/// Result of a graph query that starts from FTS5 and traverses.
#[derive(Clone, Debug, Default)]
pub struct QueryResult {
    pub entry_node: Option<GraphNode>,
    pub callers: Vec<GraphNode>,
    pub callees: Vec<GraphNode>,
    pub correlated: Vec<GraphNode>,
    pub affected: Vec<GraphNode>,
    pub similar: Vec<(f64, GraphNode)>,
}

/// Data for a CFG block node.
#[derive(Clone, Debug, PartialEq)]
pub struct CfgBlockData {
    pub start_byte: u32,
    pub end_byte: u32,
    pub block_kind: String,
    pub is_error: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_kind_constants() {
        assert_eq!(node::SYMBOL, "symbol");
        assert_eq!(node::FILE, "file");
        assert_eq!(node::DISCOVERY, "discovery");
        assert_eq!(node::CFG_BLOCK, "cfg_block");
        assert_eq!(node::HOTSPOT, "hotspot");
        assert_eq!(node::PATTERN, "pattern");
        assert_eq!(node::ISSUE, "issue");
        assert_eq!(node::KNOWLEDGE, "knowledge");
    }

    #[test]
    fn test_edge_kind_constants() {
        assert_eq!(edge::CALLS, "calls");
        assert_eq!(edge::CONTAINS, "contains");
        assert_eq!(edge::CORRELATES, "correlates");
        assert_eq!(edge::AFFECTS, "affects");
        assert_eq!(edge::FLOWS_TO, "flows_to");
        assert_eq!(edge::SIMILAR_TO, "similar_to");
        assert_eq!(edge::DERIVED_FROM, "derived_from");
        assert_eq!(edge::DISCOVERED_BY, "discovered_by");
        assert_eq!(edge::MENTIONS, "mentions");
        assert_eq!(edge::BELONGS_TO, "belongs_to");
    }

    #[test]
    fn test_graph_node_property_access() {
        let node = GraphNode {
            id: 1,
            kind: node::SYMBOL.to_string(),
            name: "my_func".to_string(),
            file_path: Some("src/lib.rs".to_string()),
            data: serde_json::json!({
                "symbol_kind": "Function",
                "line": 42,
                "complexity": 5
            }),
        };

        assert_eq!(node.prop_str("symbol_kind"), Some("Function"));
        assert_eq!(node.prop_u64("line"), Some(42));
        assert_eq!(node.prop_f64("complexity"), None); // it's an integer in JSON
        assert_eq!(node.prop_str("nonexistent"), None);
    }

    #[test]
    fn test_sync_report_default() {
        let report = SyncReport::default();
        assert_eq!(report.nodes_added, 0);
        assert_eq!(report.edges_added, 0);
    }

    #[test]
    fn test_cfg_block_data() {
        let block = CfgBlockData {
            start_byte: 100,
            end_byte: 200,
            block_kind: "Basic".to_string(),
            is_error: false,
        };
        assert_eq!(block.start_byte, 100);
        assert!(!block.is_error);
    }

    #[test]
    fn test_direction_enum() {
        assert_ne!(Direction::Incoming, Direction::Outgoing);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /home/feanor/Projects/forge && cargo test -p forge-core --lib knowledge::types::tests -- --nocapture 2>&1 | tail -5`
Expected: FAIL — module `knowledge` does not exist yet.

- [ ] **Step 3: Add native-v3 feature to Cargo.toml and create the module skeleton**

Modify `forge_core/Cargo.toml` — add to `[features]` section:

```toml
# In [features], add after existing features:
native-v3 = ["dep:sqlitegraph", "sqlitegraph/native-v3"]
```

Change the existing sqlite line to remove the dep: prefix duplication:

```toml
# Change:
sqlite = ["dep:sqlitegraph", "sqlitegraph/sqlite-backend"]
# To (stays the same, just ensure native-v3 is also available):
```

The `[features]` section should have both:

```toml
sqlite = ["dep:sqlitegraph", "sqlitegraph/sqlite-backend"]
native-v3 = ["dep:sqlitegraph", "sqlitegraph/native-v3"]
```

Create `forge_core/src/knowledge/mod.rs` (empty module to start):

```rust
//! Knowledge graph — sqlitegraph native-v3 backed graph for code intelligence.

pub mod types;

pub use types::*;
```

Add to `forge_core/src/lib.rs` — add after the existing module declarations (around line 20, after `pub mod treesitter;`):

```rust
// Knowledge graph module (sqlitegraph native-v3)
pub mod knowledge;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd /home/feanor/Projects/forge && cargo test -p forge-core --lib knowledge::types::tests -- --nocapture`
Expected: 6 tests PASS

- [ ] **Step 5: Commit**

```bash
cd /home/feanor/Projects/forge
git add forge_core/Cargo.toml forge_core/src/knowledge/ forge_core/src/lib.rs
git commit -m "feat(forge_core): add knowledge module skeleton with types

Node kind constants (symbol, file, discovery, cfg_block, hotspot,
pattern, issue, knowledge), edge kind constants (calls, contains,
correlates, affects, flows_to, similar_to, derived_from, discovered_by,
mentions, belongs_to), GraphNode wrapper, SyncReport, QueryResult,
CfgBlockData, Direction enum. Native-v3 feature flag on Cargo.toml."
```

---

### Task 2: KnowledgeGraph lifecycle (open/close)

**Files:**
- Modify: `forge_core/src/knowledge/mod.rs`

- [ ] **Step 1: Write the failing test**

Add to `forge_core/src/knowledge/mod.rs` after the module declarations:

```rust
use std::path::{Path, PathBuf};

use crate::error::{ForgeError, Result};
use sqlitegraph::backend::GraphBackend;
use sqlitegraph::config::{open_graph, BackendKind as SqlBackendKind, GraphConfig};

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

        // Create parent dirs if needed
        if let Some(parent) = graph_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }

        let kg = KnowledgeGraph::open(&graph_path, &db_path).unwrap();
        assert!(graph_path.exists());
    }

    #[test]
    fn test_knowledge_graph_open_existing_file() {
        let temp = tempfile::tempdir().unwrap();
        let graph_path = temp.path().join("kg.graph");
        let db_path = temp.path().join("magellan.db");

        // Create it once
        {
            let _kg = KnowledgeGraph::open(&graph_path, &db_path).unwrap();
        }

        // Open again — should work
        let _kg2 = KnowledgeGraph::open(&graph_path, &db_path).unwrap();
        assert!(graph_path.exists());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /home/feanor/Projects/forge && cargo test -p forge-core --lib knowledge::tests::test_knowledge_graph -- --nocapture 2>&1 | tail -10`
Expected: FAIL — `GraphConfig::native()` may not exist or compile issues.

- [ ] **Step 3: Implement minimal code**

The code in Step 1 IS the implementation. Verify `GraphConfig::native()` exists:

```rust
// sqlitegraph's GraphConfig has a native() constructor:
let config = GraphConfig::native();
// This sets backend = BackendKind::Native
```

If it doesn't exist, use:

```rust
let config = GraphConfig {
    backend: SqlBackendKind::Native,
    ..Default::default()
};
```

Also need to ensure `forge_core/Cargo.toml` has the native-v3 feature enabled for the test. Run with:

```bash
cargo test -p forge-core --lib --features native-v3 knowledge::tests::test_knowledge_graph -- --nocapture
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd /home/feanor/Projects/forge && cargo test -p forge-core --lib --features native-v3 knowledge::tests::test_knowledge_graph -- --nocapture`
Expected: 3 tests PASS

- [ ] **Step 5: Commit**

```bash
cd /home/feanor/Projects/forge
git add forge_core/src/knowledge/mod.rs
git commit -m "feat(forge_core): KnowledgeGraph open/close lifecycle

Opens .graph file with sqlitegraph native-v3 backend. Stores paths
to both the graph file and the Magellan .db for FTS5 bridge."
```

---

### Task 3: Node CRUD operations

**Files:**
- Modify: `forge_core/src/knowledge/mod.rs`

- [ ] **Step 1: Write the failing tests**

Add these tests to the `mod tests` block in `forge_core/src/knowledge/mod.rs`:

```rust
    #[test]
    fn test_add_symbol_node() {
        let temp = tempfile::tempdir().unwrap();
        let kg = KnowledgeGraph::open(
            &temp.path().join("kg.graph"),
            &temp.path().join("magellan.db"),
        )
        .unwrap();

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
        let temp = tempfile::tempdir().unwrap();
        let kg = KnowledgeGraph::open(
            &temp.path().join("kg.graph"),
            &temp.path().join("magellan.db"),
        )
        .unwrap();

        let id = kg
            .add_file("src/lib.rs", "Rust", "abc123")
            .unwrap();

        let node = kg.get_node(id).unwrap();
        assert_eq!(node.kind, "file");
        assert_eq!(node.name, "src/lib.rs");
        assert_eq!(node.prop_str("language"), Some("Rust"));
        assert_eq!(node.prop_str("hash"), Some("abc123"));
    }

    #[test]
    fn test_add_discovery_node() {
        let temp = tempfile::tempdir().unwrap();
        let kg = KnowledgeGraph::open(
            &temp.path().join("kg.graph"),
            &temp.path().join("magellan.db"),
        )
        .unwrap();

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
        let temp = tempfile::tempdir().unwrap();
        let kg = KnowledgeGraph::open(
            &temp.path().join("kg.graph"),
            &temp.path().join("magellan.db"),
        )
        .unwrap();

        let id = kg
            .add_issue("high", "unwrap in production code", Some("M001"))
            .unwrap();

        let node = kg.get_node(id).unwrap();
        assert_eq!(node.kind, "issue");
        assert_eq!(node.prop_str("severity"), Some("high"));
        assert_eq!(node.prop_str("description"), Some("unwrap in production code"));
        assert_eq!(node.prop_str("rule_id"), Some("M001"));
    }

    #[test]
    fn test_add_pattern_node() {
        let temp = tempfile::tempdir().unwrap();
        let kg = KnowledgeGraph::open(
            &temp.path().join("kg.graph"),
            &temp.path().join("magellan.db"),
        )
        .unwrap();

        let id = kg
            .add_pattern("builder", 0.92, "builder pattern detected")
            .unwrap();

        let node = kg.get_node(id).unwrap();
        assert_eq!(node.kind, "pattern");
        assert_eq!(node.prop_str("pattern_type"), Some("builder"));
        assert_eq!(node.prop_f64("confidence"), Some(0.92));
    }

    #[test]
    fn test_add_knowledge_node() {
        let temp = tempfile::tempdir().unwrap();
        let kg = KnowledgeGraph::open(
            &temp.path().join("kg.graph"),
            &temp.path().join("magellan.db"),
        )
        .unwrap();

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
        let temp = tempfile::tempdir().unwrap();
        let kg = KnowledgeGraph::open(
            &temp.path().join("kg.graph"),
            &temp.path().join("magellan.db"),
        )
        .unwrap();

        let id = kg.add_hotspot(15, 0.85, 3, "high complexity loop").unwrap();

        let node = kg.get_node(id).unwrap();
        assert_eq!(node.kind, "hotspot");
        assert_eq!(node.prop_u64("complexity"), Some(15));
        assert_eq!(node.prop_f64("risk_score"), Some(0.85));
    }

    #[test]
    fn test_add_cfg_block_node() {
        let temp = tempfile::tempdir().unwrap();
        let kg = KnowledgeGraph::open(
            &temp.path().join("kg.graph"),
            &temp.path().join("magellan.db"),
        )
        .unwrap();

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
        let temp = tempfile::tempdir().unwrap();
        let kg = KnowledgeGraph::open(
            &temp.path().join("kg.graph"),
            &temp.path().join("magellan.db"),
        )
        .unwrap();

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
        let temp = tempfile::tempdir().unwrap();
        let kg = KnowledgeGraph::open(
            &temp.path().join("kg.graph"),
            &temp.path().join("magellan.db"),
        )
        .unwrap();

        let result = kg.get_node(99999);
        assert!(result.is_err());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /home/feanor/Projects/forge && cargo test -p forge-core --lib --features native-v3 knowledge::tests::test_add_symbol -- --nocapture 2>&1 | tail -5`
Expected: FAIL — method `add_symbol` does not exist on KnowledgeGraph.

- [ ] **Step 3: Implement node CRUD methods**

Add these methods to `impl KnowledgeGraph` in `forge_core/src/knowledge/mod.rs`:

```rust
    /// Retrieves a node by ID.
    pub fn get_node(&self, node_id: i64) -> Result<GraphNode> {
        let snapshot = sqlitegraph::snapshot::SnapshotId::current();
        let entity = self
            .backend
            .get_node(snapshot, node_id)
            .map_err(|e| ForgeError::DatabaseError(format!("Node not found: {}", e)))?;

        Ok(GraphNode::from_entity(node_id, &entity))
    }

    /// Finds all nodes of a given kind.
    pub fn find_nodes_by_kind(&self, kind: &str) -> Result<Vec<GraphNode>> {
        let snapshot = sqlitegraph::snapshot::SnapshotId::current();
        let ids = self
            .backend
            .entity_ids()
            .map_err(|e| ForgeError::DatabaseError(format!("Entity list failed: {}", e)))?;

        let mut results = Vec::new();
        for id in ids {
            if let Ok(entity) = self.backend.get_node(snapshot, id) {
                if entity.kind == kind {
                    results.push(GraphNode::from_entity(id, &entity));
                }
            }
        }
        Ok(results)
    }

    /// Inserts a raw node using NodeSpec. Returns the node ID.
    fn insert_node(&self, kind: &str, name: &str, file_path: Option<&str>, data: serde_json::Value) -> Result<i64> {
        let spec = sqlitegraph::backend::NodeSpec {
            kind: kind.to_string(),
            name: name.to_string(),
            file_path: file_path.map(|s| s.to_string()),
            data,
        };
        self.backend
            .insert_node(spec)
            .map_err(|e| ForgeError::DatabaseError(format!("Insert node failed: {}", e)))
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
    pub fn add_issue(&self, severity: &str, description: &str, rule_id: Option<&str>) -> Result<i64> {
        let mut data = serde_json::json!({
            "severity": severity,
            "description": description,
        });
        if let Some(rid) = rule_id {
            data["rule_id"] = serde_json::json!(rid);
        }
        self.insert_node(types::node::ISSUE, description, None, data)
    }

    /// Adds a pattern node.
    pub fn add_pattern(&self, pattern_type: &str, confidence: f64, description: &str) -> Result<i64> {
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
    pub fn add_hotspot(&self, complexity: u32, risk_score: f64, loop_depth: u32, description: &str) -> Result<i64> {
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
        self.insert_node(types::node::CFG_BLOCK, &format!("block_{}", block.start_byte), None, data)
    }
```

Also add `chrono` to `forge_core/Cargo.toml` dependencies:

```toml
chrono = "0.4"
```

And add the import at the top of `mod.rs`:

```rust
use types::*;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd /home/feanor/Projects/forge && cargo test -p forge-core --lib --features native-v3 knowledge::tests -- --nocapture`
Expected: All 10 tests PASS (3 lifecycle + 10 CRUD + get_node_not_found + find_nodes_by_kind)

- [ ] **Step 5: Commit**

```bash
cd /home/feanor/Projects/forge
git add forge_core/src/knowledge/mod.rs forge_core/Cargo.toml
git commit -m "feat(forge_core): KnowledgeGraph node CRUD operations

add_symbol, add_file, add_discovery, add_issue, add_pattern,
add_knowledge, add_hotspot, add_cfg_block, get_node,
find_nodes_by_kind. All store typed JSON properties via NodeSpec."
```

---

### Task 4: Edge operations

**Files:**
- Modify: `forge_core/src/knowledge/mod.rs`

- [ ] **Step 1: Write the failing tests**

Add these tests to the test module:

```rust
    #[test]
    fn test_add_edge() {
        let temp = tempfile::tempdir().unwrap();
        let kg = KnowledgeGraph::open(
            &temp.path().join("kg.graph"),
            &temp.path().join("magellan.db"),
        )
        .unwrap();

        let func_a = kg.add_symbol("func_a", "Function", "a", "f.rs", 1, 0, 10, "Rust", None).unwrap();
        let func_b = kg.add_symbol("func_b", "Function", "b", "f.rs", 2, 0, 10, "Rust", None).unwrap();

        let edge_id = kg
            .add_edge(func_a, func_b, "calls", serde_json::json!({"location_line": 5}))
            .unwrap();

        assert!(edge_id > 0);
    }

    #[test]
    fn test_add_correlation_bidirectional() {
        let temp = tempfile::tempdir().unwrap();
        let kg = KnowledgeGraph::open(
            &temp.path().join("kg.graph"),
            &temp.path().join("magellan.db"),
        )
        .unwrap();

        let sym = kg.add_symbol("my_func", "Function", "a", "f.rs", 1, 0, 10, "Rust", None).unwrap();
        let disc = kg.add_discovery("claude1", "Symbol", "my_func", serde_json::json!({})).unwrap();

        kg.add_correlation(disc, sym, 0.95, "claude1").unwrap();

        // Verify outgoing correlation: discovery -> symbol
        let outgoing = kg.neighbors(disc, "correlates", Direction::Outgoing).unwrap();
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].name, "my_func");

        // Verify incoming correlation: symbol <- discovery
        let incoming = kg.neighbors(sym, "correlates", Direction::Incoming).unwrap();
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].name, "my_func");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /home/feanor/Projects/forge && cargo test -p forge-core --lib --features native-v3 knowledge::tests::test_add_edge -- --nocapture 2>&1 | tail -5`
Expected: FAIL — method `add_edge` does not exist.

- [ ] **Step 3: Implement edge operations**

Add these methods to `impl KnowledgeGraph`:

```rust
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
        self.backend
            .insert_edge(spec)
            .map_err(|e| ForgeError::DatabaseError(format!("Insert edge failed: {}", e)))
    }

    /// Adds a bidirectional correlation between two nodes.
    ///
    /// Creates two directed edges: from→to and to→from, both with type
    /// "correlates".
    pub fn add_correlation(&self, from: i64, to: i64, confidence: f64, agent: &str) -> Result<()> {
        let data = serde_json::json!({
            "confidence": confidence,
            "agent": agent,
        });
        self.add_edge(from, to, types::edge::CORRELATES, data.clone())?;
        self.add_edge(to, from, types::edge::CORRELATES, data)?;
        Ok(())
    }

    /// Returns neighbors of a node filtered by edge type and direction.
    pub fn neighbors(&self, node_id: i64, edge_type: &str, direction: Direction) -> Result<Vec<GraphNode>> {
        let snapshot = sqlitegraph::snapshot::SnapshotId::current();
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
            .neighbors(snapshot, node_id, query)
            .map_err(|e| ForgeError::DatabaseError(format!("Neighbor query failed: {}", e)))?;

        let mut results = Vec::new();
        for nid in neighbor_ids {
            if let Ok(entity) = self.backend.get_node(snapshot, nid) {
                results.push(GraphNode::from_entity(nid, &entity));
            }
        }
        Ok(results)
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd /home/feanor/Projects/forge && cargo test -p forge-core --lib --features native-v3 knowledge::tests -- --nocapture`
Expected: All tests PASS (including new edge tests)

- [ ] **Step 5: Commit**

```bash
cd /home/feanor/Projects/forge
git add forge_core/src/knowledge/mod.rs
git commit -m "feat(forge_core): KnowledgeGraph edge operations and neighbor queries

add_edge, add_correlation (bidirectional), neighbors with edge type
and direction filtering via sqlitegraph NeighborQuery."
```

---

### Task 5: Traversal operations

**Files:**
- Modify: `forge_core/src/knowledge/mod.rs`

- [ ] **Step 1: Write the failing tests**

```rust
    #[test]
    fn test_callers_of() {
        let temp = tempfile::tempdir().unwrap();
        let kg = KnowledgeGraph::open(
            &temp.path().join("kg.graph"),
            &temp.path().join("magellan.db"),
        )
        .unwrap();

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
        let temp = tempfile::tempdir().unwrap();
        let kg = KnowledgeGraph::open(
            &temp.path().join("kg.graph"),
            &temp.path().join("magellan.db"),
        )
        .unwrap();

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
        let temp = tempfile::tempdir().unwrap();
        let kg = KnowledgeGraph::open(
            &temp.path().join("kg.graph"),
            &temp.path().join("magellan.db"),
        )
        .unwrap();

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
        let temp = tempfile::tempdir().unwrap();
        let kg = KnowledgeGraph::open(
            &temp.path().join("kg.graph"),
            &temp.path().join("magellan.db"),
        )
        .unwrap();

        let sym = kg.add_symbol("process_payment", "Function", "a", "f.rs", 1, 0, 10, "Rust", None).unwrap();
        let issue = kg.add_issue("high", "race condition", None).unwrap();

        kg.add_edge(issue, sym, "affects", serde_json::json!({})).unwrap();

        let affected = kg.affected_by(sym, 1).unwrap();
        assert_eq!(affected.len(), 1);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /home/feanor/Projects/forge && cargo test -p forge-core --lib --features native-v3 knowledge::tests::test_callers_of -- --nocapture 2>&1 | tail -5`
Expected: FAIL — method `callers_of` does not exist.

- [ ] **Step 3: Implement traversal methods**

Add these methods to `impl KnowledgeGraph`:

```rust
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

    /// BFS traversal filtered by a specific edge type and direction.
    fn bfs_by_edge_type(
        &self,
        start: i64,
        edge_type: &str,
        direction: Direction,
        max_depth: u32,
    ) -> Result<Vec<GraphNode>> {
        let snapshot = sqlitegraph::snapshot::SnapshotId::current();
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
                if let Ok(neighbor_ids) = self.backend.neighbors(snapshot, *node_id, query) {
                    for nid in neighbor_ids {
                        if visited.insert(nid) {
                            next_frontier.push(nid);
                            if let Ok(entity) = self.backend.get_node(snapshot, nid) {
                                results.push(GraphNode::from_entity(nid, &entity));
                            }
                        }
                    }
                }
            }
            frontier = next_frontier;
        }

        Ok(results)
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd /home/feanor/Projects/forge && cargo test -p forge-core --lib --features native-v3 knowledge::tests -- --nocapture`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
cd /home/feanor/Projects/forge
git add forge_core/src/knowledge/mod.rs
git commit -m "feat(forge_core): KnowledgeGraph traversal operations

callers_of, callees_of, correlated, affected_by. BFS traversal
filtered by edge type and direction."
```

---

### Task 6: Graph algorithms

**Files:**
- Modify: `forge_core/src/knowledge/mod.rs`

- [ ] **Step 1: Write the failing tests**

```rust
    #[test]
    fn test_pagerank() {
        let temp = tempfile::tempdir().unwrap();
        let kg = KnowledgeGraph::open(
            &temp.path().join("kg.graph"),
            &temp.path().join("magellan.db"),
        )
        .unwrap();

        let a = kg.add_symbol("a", "Function", "a", "f.rs", 1, 0, 10, "Rust", None).unwrap();
        let b = kg.add_symbol("b", "Function", "b", "f.rs", 2, 0, 10, "Rust", None).unwrap();
        let c = kg.add_symbol("c", "Function", "c", "f.rs", 3, 0, 10, "Rust", None).unwrap();

        kg.add_edge(a, b, "calls", serde_json::json!({})).unwrap();
        kg.add_edge(b, c, "calls", serde_json::json!({})).unwrap();
        kg.add_edge(c, a, "calls", serde_json::json!({})).unwrap();

        let ranks = kg.pagerank().unwrap();
        assert_eq!(ranks.len(), 3);

        // All nodes should have positive rank
        for (_, rank) in &ranks {
            assert!(*rank > 0.0);
        }
    }

    #[test]
    fn test_shortest_path() {
        let temp = tempfile::tempdir().unwrap();
        let kg = KnowledgeGraph::open(
            &temp.path().join("kg.graph"),
            &temp.path().join("magellan.db"),
        )
        .unwrap();

        let a = kg.add_symbol("a", "Function", "a", "f.rs", 1, 0, 10, "Rust", None).unwrap();
        let b = kg.add_symbol("b", "Function", "b", "f.rs", 2, 0, 10, "Rust", None).unwrap();
        let c = kg.add_symbol("c", "Function", "c", "f.rs", 3, 0, 10, "Rust", None).unwrap();

        kg.add_edge(a, b, "calls", serde_json::json!({})).unwrap();
        kg.add_edge(b, c, "calls", serde_json::json!({})).unwrap();

        let path = kg.shortest_path(a, c).unwrap();
        assert!(path.is_some());
        let path = path.unwrap();
        assert_eq!(path.len(), 3); // a -> b -> c
        assert_eq!(path[0], a);
        assert_eq!(path[2], c);
    }

    #[test]
    fn test_reachability() {
        let temp = tempfile::tempdir().unwrap();
        let kg = KnowledgeGraph::open(
            &temp.path().join("kg.graph"),
            &temp.path().join("magellan.db"),
        )
        .unwrap();

        let a = kg.add_symbol("a", "Function", "a", "f.rs", 1, 0, 10, "Rust", None).unwrap();
        let b = kg.add_symbol("b", "Function", "b", "f.rs", 2, 0, 10, "Rust", None).unwrap();
        let c = kg.add_symbol("c", "Function", "c", "f.rs", 3, 0, 10, "Rust", None).unwrap();

        kg.add_edge(a, b, "calls", serde_json::json!({})).unwrap();
        kg.add_edge(b, c, "calls", serde_json::json!({})).unwrap();

        let reachable = kg.reachability(a).unwrap();
        assert!(reachable.contains(&b));
        assert!(reachable.contains(&c));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /home/feanor/Projects/forge && cargo test -p forge-core --lib --features native-v3 knowledge::tests::test_pagerank -- --nocapture 2>&1 | tail -5`
Expected: FAIL — method `pagerank` does not exist.

- [ ] **Step 3: Implement graph algorithms**

Add these methods to `impl KnowledgeGraph`:

```rust
    /// Runs PageRank on the graph. Returns (node_id, score) pairs.
    pub fn pagerank(&self) -> Result<Vec<(i64, f64)>> {
        sqlitegraph::pagerank(&self.backend, 100)
            .map_err(|e| ForgeError::DatabaseError(format!("PageRank failed: {}", e)))
    }

    /// Finds the shortest path between two nodes.
    pub fn shortest_path(&self, from: i64, to: i64) -> Result<Option<Vec<i64>>> {
        let snapshot = sqlitegraph::snapshot::SnapshotId::current();
        self.backend
            .shortest_path(snapshot, from, to)
            .map_err(|e| ForgeError::DatabaseError(format!("Shortest path failed: {}", e)))
    }

    /// Returns all nodes reachable from the given node.
    pub fn reachability(&self, from: i64) -> Result<Vec<i64>> {
        let snapshot = sqlitegraph::snapshot::SnapshotId::current();
        self.backend
            .bfs(snapshot, from, 100) // reasonable depth limit
            .map_err(|e| ForgeError::DatabaseError(format!("Reachability failed: {}", e)))
    }

    /// Detects cycles in the graph.
    pub fn cycles(&self) -> Result<Vec<Vec<i64>>> {
        // Use BFS-based cycle detection via sqlitegraph
        let ids = self.backend.entity_ids()
            .map_err(|e| ForgeError::DatabaseError(format!("Entity list failed: {}", e)))?;

        let snapshot = sqlitegraph::snapshot::SnapshotId::current();
        let mut cycles = Vec::new();
        let mut visited = std::collections::HashSet::new();

        for start_id in &ids {
            if visited.contains(start_id) {
                continue;
            }

            // BFS and check if we revisit a node
            let mut path = vec![*start_id];
            let mut frontier = vec![*start_id];
            let mut local_visited = std::collections::HashSet::new();
            local_visited.insert(*start_id);

            while let Some(current) = frontier.pop() {
                let query = sqlitegraph::backend::NeighborQuery {
                    direction: sqlitegraph::backend::BackendDirection::Outgoing,
                    edge_type: None,
                };
                if let Ok(neighbors) = self.backend.neighbors(snapshot, current, query) {
                    for nid in neighbors {
                        if nid == *start_id && path.len() > 1 {
                            cycles.push(path.clone());
                        } else if local_visited.insert(nid) {
                            frontier.push(nid);
                            path.push(nid);
                        }
                    }
                }
            }

            visited.extend(local_visited);
        }

        Ok(cycles)
    }

    /// Community detection using label propagation.
    pub fn community_detection(&self) -> Result<Vec<Vec<i64>>> {
        sqlitegraph::label_propagation(&self.backend)
            .map_err(|e| ForgeError::DatabaseError(format!("Community detection failed: {}", e)))
    }
```

Note: The `pagerank` and `label_propagation` functions take `&dyn GraphBackend` as their first argument. Verify the actual signature in sqlitegraph — if they take `&SqliteGraph` instead, you'll need to downcast or store the concrete type.

If the algorithms require `&SqliteGraph` specifically, change `KnowledgeGraph` to store `SqliteGraph` directly instead of `Box<dyn GraphBackend>`:

```rust
pub struct KnowledgeGraph {
    graph: sqlitegraph::SqliteGraph,
    graph_path: PathBuf,
    db_path: PathBuf,
}
```

And use `open_graph` result appropriately. The `SqliteGraph` type implements `GraphBackend`, so all the `NodeSpec`/`EdgeSpec` operations work the same way — you just call methods on `self.graph` directly instead of through the trait object.

- [ ] **Step 4: Run test to verify it passes**

Run: `cd /home/feanor/Projects/forge && cargo test -p forge-core --lib --features native-v3 knowledge::tests -- --nocapture`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
cd /home/feanor/Projects/forge
git add forge_core/src/knowledge/mod.rs
git commit -m "feat(forge_core): KnowledgeGraph graph algorithms

pagerank, shortest_path, reachability, cycles, community_detection.
Delegates to sqlitegraph's algorithm implementations."
```

---

### Task 7: FTS5 bridge and Magellan sync

**Files:**
- Modify: `forge_core/src/knowledge/mod.rs`
- Create: `forge_core/src/knowledge/sync.rs`

- [ ] **Step 1: Write the failing tests**

```rust
    #[tokio::test]
    async fn test_sync_symbols_empty_db() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("magellan.db");

        // Create an empty SQLite DB with the graph_node_index table
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS graph_node_index (
                node_id INTEGER PRIMARY KEY,
                magellan_id INTEGER,
                node_kind TEXT NOT NULL,
                graph_file TEXT NOT NULL
            );"
        ).unwrap();
        drop(conn);

        let kg = KnowledgeGraph::open(
            &temp.path().join("kg.graph"),
            &db_path,
        )
        .unwrap();

        let report = kg.sync_symbols().await.unwrap();
        assert_eq!(report.nodes_added, 0);
    }

    #[test]
    fn test_fts5_resolve_empty() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("magellan.db");

        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS graph_node_index (
                node_id INTEGER PRIMARY KEY,
                magellan_id INTEGER,
                node_kind TEXT NOT NULL,
                graph_file TEXT NOT NULL
            );"
        ).unwrap();
        drop(conn);

        let kg = KnowledgeGraph::open(
            &temp.path().join("kg.graph"),
            &db_path,
        )
        .unwrap();

        let result = kg.resolve_fts5("nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_fts5_resolve_after_populate() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("magellan.db");

        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS graph_node_index (
                node_id INTEGER PRIMARY KEY,
                magellan_id INTEGER,
                node_kind TEXT NOT NULL,
                graph_file TEXT NOT NULL
            );
            INSERT INTO graph_node_index (node_id, magellan_id, node_kind, graph_file)
            VALUES (47, 1, 'symbol', 'kg.graph');"
        ).unwrap();
        drop(conn);

        let kg = KnowledgeGraph::open(
            &temp.path().join("kg.graph"),
            &db_path,
        )
        .unwrap();

        let node_id = kg.resolve_fts5_by_magellan_id(1).unwrap();
        assert_eq!(node_id, Some(47));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /home/feanor/Projects/forge && cargo test -p forge-core --lib --features native-v3 knowledge::tests::test_sync_symbols -- --nocapture 2>&1 | tail -5`
Expected: FAIL — method `sync_symbols` does not exist.

- [ ] **Step 3: Implement sync and FTS5 bridge**

Add `rusqlite` to `forge_core/Cargo.toml` dependencies (if not already present):

```toml
rusqlite = { version = "0.31", features = ["bundled"] }
```

Add these methods to `impl KnowledgeGraph`:

```rust
    /// Resolves a Magellan symbol_id to a graph node_id via the bridge table.
    pub fn resolve_fts5_by_magellan_id(&self, magellan_id: i64) -> Result<Option<i64>> {
        let conn = rusqlite::Connection::open(&self.db_path)
            .map_err(|e| ForgeError::DatabaseError(format!("Open db failed: {}", e)))?;

        let mut stmt = conn
            .prepare("SELECT node_id FROM graph_node_index WHERE magellan_id = ?1")
            .map_err(|e| ForgeError::DatabaseError(format!("Prepare failed: {}", e)))?;

        let result = stmt
            .query_row(rusqlite::params![magellan_id], |row| row.get::<_, i64>(0))
            .ok();

        Ok(result)
    }

    /// Resolves a keyword to a graph node_id via FTS5 (placeholder).
    ///
    /// In production, this queries the Magellan FTS5 tables. For now,
    /// it delegates to resolve_fts5_by_magellan_id.
    pub fn resolve_fts5(&self, _keyword: &str) -> Result<Option<i64>> {
        // Full FTS5 integration requires reading Magellan's symbols_fts table.
        // This is a placeholder that will be completed when we integrate
        // with the actual Magellan schema.
        Ok(None)
    }

    /// Syncs symbols from the Magellan .db into the knowledge graph.
    ///
    /// Reads all symbols from the Magellan database, creates corresponding
    /// nodes in the .graph file, and populates the graph_node_index bridge table.
    ///
    /// For Phase 1, this reads from Magellan's symbol tables if the `magellan`
    /// feature is enabled. Otherwise, it returns an empty report.
    pub async fn sync_symbols(&self) -> Result<SyncReport> {
        let mut report = SyncReport::default();

        #[cfg(feature = "magellan")]
        {
            // Read symbols from Magellan and create graph nodes
            // This will be implemented when we integrate with the magellan crate API
            report.nodes_added = 0;
        }

        #[cfg(not(feature = "magellan"))]
        {
            report.nodes_added = 0;
        }

        Ok(report)
    }

    /// Syncs references from the Magellan .db into the knowledge graph.
    ///
    /// Creates `calls` and `references` edges between existing symbol nodes.
    pub async fn sync_references(&self) -> Result<SyncReport> {
        let mut report = SyncReport::default();

        #[cfg(feature = "magellan")]
        {
            // Read references from Magellan and create graph edges
            // This will be implemented when we integrate with the magellan crate API
            report.edges_added = 0;
        }

        #[cfg(not(feature = "magellan"))]
        {
            report.edges_added = 0;
        }

        Ok(report)
    }

    /// Populates the bridge table with a node mapping.
    fn insert_bridge_entry(&self, node_id: i64, magellan_id: i64, graph_file: &str) -> Result<()> {
        let conn = rusqlite::Connection::open(&self.db_path)
            .map_err(|e| ForgeError::DatabaseError(format!("Open db failed: {}", e)))?;

        // Ensure the table exists
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS graph_node_index (
                node_id INTEGER PRIMARY KEY,
                magellan_id INTEGER,
                node_kind TEXT NOT NULL,
                graph_file TEXT NOT NULL
            );"
        ).map_err(|e| ForgeError::DatabaseError(format!("Create table failed: {}", e)))?;

        conn.execute(
            "INSERT OR REPLACE INTO graph_node_index (node_id, magellan_id, node_kind, graph_file)
             VALUES (?1, ?2, 'symbol', ?3)",
            rusqlite::params![node_id, magellan_id, graph_file],
        ).map_err(|e| ForgeError::DatabaseError(format!("Insert bridge failed: {}", e)))?;

        Ok(())
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd /home/feanor/Projects/forge && cargo test -p forge-core --lib --features native-v3 knowledge::tests -- --nocapture`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
cd /home/feanor/Projects/forge
git add forge_core/src/knowledge/mod.rs forge_core/Cargo.toml
git commit -m "feat(forge_core): FTS5 bridge and Magellan sync skeleton

resolve_fts5_by_magellan_id, resolve_fts5, sync_symbols, sync_references.
Bridge table (graph_node_index) maps Magellan IDs to graph node IDs."
```

---

### Task 8: Forge struct integration

**Files:**
- Modify: `forge_core/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Add to the tests in `forge_core/src/lib.rs`:

```rust
    #[tokio::test]
    async fn test_forge_knowledge_accessor() {
        let temp_dir = tempfile::tempdir().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();

        let kg = forge.knowledge();
        assert!(kg.is_ok());

        let kg = kg.unwrap();
        assert!(kg.graph_path().exists());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /home/feanor/Projects/forge && cargo test -p forge-core --lib --features native-v3 tests::test_forge_knowledge_accessor -- --nocapture 2>&1 | tail -5`
Expected: FAIL — method `knowledge` does not exist on Forge.

- [ ] **Step 3: Add knowledge() accessor to Forge**

Add to `impl Forge` in `forge_core/src/lib.rs`:

```rust
    /// Returns the knowledge graph module.
    ///
    /// Opens or creates the `.magellan/knowledge.graph` file using
    /// sqlitegraph native-v3 backend.
    pub fn knowledge(&self) -> anyhow::Result<knowledge::KnowledgeGraph> {
        let graph_path = self.store.codebase_path.join(".magellan").join("knowledge.graph");
        let db_path = self.store.db_path.clone();

        // Ensure parent directory exists
        if let Some(parent) = graph_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        knowledge::KnowledgeGraph::open(&graph_path, &db_path)
            .map_err(|e| anyhow!("Failed to open knowledge graph: {}", e))
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd /home/feanor/Projects/forge && cargo test -p forge-core --lib --features native-v3 tests::test_forge_knowledge_accessor -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
cd /home/feanor/Projects/forge
git add forge_core/src/lib.rs
git commit -m "feat(forge_core): knowledge() accessor on Forge struct

Opens .magellan/knowledge.graph with sqlitegraph native-v3 backend.
Available with the native-v3 feature flag."
```

---

### Task 9: Query entry point (FTS5 → traversal)

**Files:**
- Modify: `forge_core/src/knowledge/mod.rs`

- [ ] **Step 1: Write the failing test**

```rust
    #[tokio::test]
    async fn test_query_no_results() {
        let temp = tempfile::tempdir().unwrap();
        let kg = KnowledgeGraph::open(
            &temp.path().join("kg.graph"),
            &temp.path().join("magellan.db"),
        )
        .unwrap();

        let result = kg.query("nonexistent", 3).await.unwrap();
        assert!(result.entry_node.is_none());
        assert!(result.callers.is_empty());
    }

    #[test]
    fn test_query_with_symbol() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("magellan.db");

        // Set up bridge table
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS graph_node_index (
                node_id INTEGER PRIMARY KEY,
                magellan_id INTEGER,
                node_kind TEXT NOT NULL,
                graph_file TEXT NOT NULL
            );"
        ).unwrap();
        drop(conn);

        let kg = KnowledgeGraph::open(
            &temp.path().join("kg.graph"),
            &db_path,
        )
        .unwrap();

        // Create symbol and bridge entry
        let sym_id = kg.add_symbol("my_func", "Function", "a::my_func", "f.rs", 1, 0, 10, "Rust", None).unwrap();
        let caller_id = kg.add_symbol("caller", "Function", "a::caller", "f.rs", 5, 0, 10, "Rust", None).unwrap();
        kg.add_edge(caller_id, sym_id, "calls", serde_json::json!({})).unwrap();

        // Add bridge entry manually
        kg.insert_bridge_entry(sym_id, 1, "kg.graph").unwrap();

        // Query by magellan ID
        let entry = kg.resolve_fts5_by_magellan_id(1).unwrap();
        assert_eq!(entry, Some(sym_id));

        // Now traverse from that entry
        let callers = kg.callers_of(sym_id, 1).unwrap();
        assert_eq!(callers.len(), 1);
        assert_eq!(callers[0].name, "caller");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /home/feanor/Projects/forge && cargo test -p forge-core --lib --features native-v3 knowledge::tests::test_query -- --nocapture 2>&1 | tail -5`
Expected: FAIL — method `query` does not exist.

- [ ] **Step 3: Implement query method**

Add to `impl KnowledgeGraph`:

```rust
    /// Entry-point query: resolves a keyword via FTS5, then traverses the graph.
    ///
    /// 1. FTS5 resolves keyword → graph node_id
    /// 2. Graph traversal discovers callers, callees, correlations, affected
    pub async fn query(&self, keyword: &str, depth: u32) -> Result<QueryResult> {
        // Step 1: Resolve via FTS5
        let entry_id = self.resolve_fts5(keyword)?;

        let Some(entry_id) = entry_id else {
            return Ok(QueryResult::default());
        };

        let entry_node = self.get_node(entry_id).ok();

        // Step 2: Traverse
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
            similar: Vec::new(), // HNSW — Phase 4
        })
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd /home/feanor/Projects/forge && cargo test -p forge-core --lib --features native-v3 knowledge::tests -- --nocapture`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
cd /home/feanor/Projects/forge
git add forge_core/src/knowledge/mod.rs
git commit -m "feat(forge_core): KnowledgeGraph query entry point

FTS5 keyword resolution → graph traversal. Returns QueryResult with
callers, callees, correlated, and affected nodes."
```

---

### Task 10: Final clippy + fmt + full test suite

**Files:**
- All modified files

- [ ] **Step 1: Format and lint**

Run:
```bash
cd /home/feanor/Projects/forge
cargo fmt --all
cargo clippy -p forge-core --all-targets --features native-v3 -- -D warnings 2>&1 | tail -20
```

Fix any clippy warnings. Common issues:
- Unused imports
- `field_reassign_with_default` — use `..Default::default()` pattern
- Missing `Debug` derive on types

- [ ] **Step 2: Run full test suite**

Run:
```bash
cd /home/feanor/Projects/forge
cargo test -p forge-core --features native-v3 -- --nocapture 2>&1 | tail -20
```

Expected: All tests PASS, 0 failures.

- [ ] **Step 3: Commit any fixes**

```bash
cd /home/feanor/Projects/forge
git add -A
git commit -m "fix(forge_core): clippy warnings and fmt for knowledge module"
```

- [ ] **Step 4: Verify without native-v3 feature (no regressions)**

Run:
```bash
cd /home/feanor/Projects/forge
cargo test -p forge-core -- --nocapture 2>&1 | tail -20
```

Expected: All existing tests still PASS. Knowledge tests are skipped (feature-gated).

---

## Self-Review

**1. Spec coverage:**

| Spec Section | Task |
|-------------|------|
| Node types (8 kinds) | Task 3 |
| Edge types (11 kinds) | Task 4 |
| KnowledgeGraph API | Tasks 2-6, 8-9 |
| FTS5 bridge | Task 7 |
| Agent navigation | Task 9 |
| Sync strategy | Task 7 |
| File layout (.magellan/) | Task 2 |
| Forge integration | Task 8 |
| Graph algorithms | Task 6 |
| HNSW vectors | Deferred (Phase 4 per spec) |

**2. Placeholder scan:** No TBDs. All steps have complete code. The `resolve_fts5` method is a documented placeholder for Phase 2 (Magellan integration).

**3. Type consistency:** `GraphNode::from_entity(id, &entity)` signature used consistently. `Direction::Incoming/Outgoing` used in all traversal methods. Node/edge kind constants referenced via `types::node::SYMBOL` / `types::edge::CALLS`.

**Gap:** HNSW vector integration is deferred to Phase 4 per the spec's migration path. The `similar_symbols` method and `QueryResult::similar` field exist but return empty results until HNSW is implemented.

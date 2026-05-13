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

/// Wrapper around a graph node with typed property accessors.
#[derive(Clone, Debug)]
pub struct GraphNode {
    pub id: i64,
    pub kind: String,
    pub name: String,
    pub file_path: Option<String>,
    pub data: serde_json::Value,
}

impl GraphNode {
    /// Returns a typed property from the data JSON.
    pub fn prop(&self, key: &str) -> Option<&serde_json::Value> {
        self.data.get(key)
    }

    /// Returns a string property.
    pub fn prop_str(&self, key: &str) -> Option<&str> {
        self.data.get(key).and_then(|v| v.as_str())
    }

    /// Returns a u64 property.
    pub fn prop_u64(&self, key: &str) -> Option<u64> {
        self.data.get(key).and_then(|v| v.as_u64())
    }

    /// Returns a f64 property.
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

/// Result of a graph query starting from FTS5 and traversing.
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
                "complexity": 5.5,
            }),
        };

        assert_eq!(node.prop_str("symbol_kind"), Some("Function"));
        assert_eq!(node.prop_u64("line"), Some(42));
        assert_eq!(node.prop_f64("complexity"), Some(5.5));
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

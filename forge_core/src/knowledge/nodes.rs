use crate::error::{ForgeError, Result};
use crate::knowledge::types::{self, CfgBlockData, GraphNode};
use crate::knowledge::KnowledgeGraph;

impl KnowledgeGraph {
    pub fn get_node(&self, node_id: i64) -> Result<GraphNode> {
        let entity = self
            .backend
            .get_node(Self::snapshot(), node_id)
            .map_err(|e| ForgeError::DatabaseError(format!("Node not found: {}", e)))?;
        Ok(GraphNode {
            id: node_id,
            kind: entity.kind,
            name: entity.name,
            file_path: entity.file_path,
            data: entity.data,
        })
    }

    pub fn find_nodes_by_kind(&self, kind: &str) -> Result<Vec<GraphNode>> {
        let snap = Self::snapshot();
        let ids = self
            .backend
            .entity_ids()
            .map_err(|e| ForgeError::DatabaseError(format!("Entity list failed: {}", e)))?;
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

    #[allow(clippy::too_many_arguments)]
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

    pub fn add_file(&self, path: &str, language: &str, hash: &str) -> Result<i64> {
        let data = serde_json::json!({
            "path": path,
            "language": language,
            "hash": hash,
        });
        self.insert_node(types::node::FILE, path, None, data)
    }

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

    pub fn add_issue(
        &self,
        severity: &str,
        description: &str,
        rule_id: Option<&str>,
    ) -> Result<i64> {
        let mut data = serde_json::json!({"severity": severity, "description": description,});
        if let Some(rid) = rule_id {
            data["rule_id"] = serde_json::json!(rid);
        }
        self.insert_node(types::node::ISSUE, description, None, data)
    }

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
}

#[cfg(test)]
mod tests {
    use crate::knowledge::{open_kg, CfgBlockData};

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
            .expect("invariant: fresh graph accepts inserts");
        assert!(id > 0);
        let node = kg
            .get_node(id)
            .expect("invariant: just-inserted node is retrievable");
        assert_eq!(node.kind, "symbol");
        assert_eq!(node.name, "my_func");
        assert_eq!(node.prop_str("symbol_kind"), Some("Function"));
        assert_eq!(
            node.prop_str("qualified_name"),
            Some("crate::module::my_func")
        );
        assert_eq!(node.prop_str("file"), Some("src/lib.rs"));
        assert_eq!(node.prop_u64("line"), Some(42));
    }

    #[test]
    fn test_add_file_node() {
        let (_temp, kg) = open_kg();
        let id = kg
            .add_file("src/lib.rs", "Rust", "abc123")
            .expect("invariant: fresh graph accepts inserts");
        let node = kg
            .get_node(id)
            .expect("invariant: just-inserted node is retrievable");
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
            .expect("invariant: fresh graph accepts inserts");
        let node = kg
            .get_node(id)
            .expect("invariant: just-inserted node is retrievable");
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
            .expect("invariant: fresh graph accepts inserts");
        let node = kg
            .get_node(id)
            .expect("invariant: just-inserted node is retrievable");
        assert_eq!(node.kind, "issue");
        assert_eq!(node.prop_str("severity"), Some("high"));
        assert_eq!(node.prop_str("rule_id"), Some("M001"));
    }

    #[test]
    fn test_add_pattern_node() {
        let (_temp, kg) = open_kg();
        let id = kg
            .add_pattern("builder", 0.92, "builder pattern detected")
            .expect("invariant: fresh graph accepts inserts");
        let node = kg
            .get_node(id)
            .expect("invariant: just-inserted node is retrievable");
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
            .expect("invariant: fresh graph accepts inserts");
        let node = kg
            .get_node(id)
            .expect("invariant: just-inserted node is retrievable");
        assert_eq!(node.kind, "knowledge");
        assert_eq!(node.prop_str("source"), Some("wiki"));
        assert_eq!(node.prop_str("title"), Some("Auth Architecture"));
    }

    #[test]
    fn test_add_hotspot_node() {
        let (_temp, kg) = open_kg();
        let id = kg
            .add_hotspot(15, 0.85, 3, "high complexity loop")
            .expect("invariant: fresh graph accepts inserts");
        let node = kg
            .get_node(id)
            .expect("invariant: just-inserted node is retrievable");
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
        let id = kg
            .add_cfg_block(42, &block)
            .expect("invariant: fresh graph accepts inserts");
        let node = kg
            .get_node(id)
            .expect("invariant: just-inserted node is retrievable");
        assert_eq!(node.kind, "cfg_block");
        assert_eq!(node.prop_u64("function_id"), Some(42));
        assert_eq!(node.prop_str("block_kind"), Some("Basic"));
    }

    #[test]
    fn test_find_nodes_by_kind() {
        let (_temp, kg) = open_kg();
        kg.add_symbol("func_a", "Function", "a", "f.rs", 1, 0, 10, "Rust", None)
            .expect("invariant: fresh graph accepts inserts");
        kg.add_symbol("func_b", "Function", "b", "f.rs", 2, 0, 10, "Rust", None)
            .expect("invariant: fresh graph accepts inserts");
        kg.add_file("f.rs", "Rust", "hash")
            .expect("invariant: fresh graph accepts inserts");

        let symbols = kg
            .find_nodes_by_kind("symbol")
            .expect("invariant: query on valid graph succeeds");
        assert_eq!(symbols.len(), 2);
        let files = kg
            .find_nodes_by_kind("file")
            .expect("invariant: query on valid graph succeeds");
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn test_get_node_not_found() {
        let (_temp, kg) = open_kg();
        let result = kg.get_node(99999);
        assert!(result.is_err());
    }
}

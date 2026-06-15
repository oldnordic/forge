use crate::error::{ForgeError, Result};
use crate::knowledge::types::{self, Direction, GraphNode};
use crate::knowledge::KnowledgeGraph;

impl KnowledgeGraph {
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

    pub fn add_correlation(&self, from: i64, to: i64, confidence: f64, agent: &str) -> Result<()> {
        let data = serde_json::json!({"confidence": confidence, "agent": agent,});
        self.add_edge(from, to, types::edge::CORRELATES, data.clone())?;
        self.add_edge(to, from, types::edge::CORRELATES, data)?;
        Ok(())
    }

    pub fn callers_of(&self, symbol_id: i64, max_depth: u32) -> Result<Vec<GraphNode>> {
        self.bfs_by_edge_type(
            symbol_id,
            types::edge::CALLS,
            Direction::Incoming,
            max_depth,
        )
    }

    pub fn callees_of(&self, symbol_id: i64, max_depth: u32) -> Result<Vec<GraphNode>> {
        self.bfs_by_edge_type(
            symbol_id,
            types::edge::CALLS,
            Direction::Outgoing,
            max_depth,
        )
    }

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

    pub fn shortest_path(&self, from: i64, to: i64) -> Result<Option<Vec<i64>>> {
        self.backend
            .shortest_path(Self::snapshot(), from, to)
            .map_err(|e| ForgeError::DatabaseError(format!("Shortest path failed: {}", e)))
    }

    pub fn reachability(&self, from: i64) -> Result<Vec<i64>> {
        self.backend
            .bfs(Self::snapshot(), from, 100)
            .map_err(|e| ForgeError::DatabaseError(format!("Reachability failed: {}", e)))
    }

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
    use crate::knowledge::{open_kg, Direction, SourceSpan};

    #[test]
    fn test_add_edge() {
        let (_temp, kg) = open_kg();
        let a = kg
            .add_symbol(
                "func_a",
                "Function",
                "a",
                &SourceSpan::new("f.rs", 1, 0, 10),
                "Rust",
                None,
            )
            .expect("invariant: fresh graph accepts inserts");
        let b = kg
            .add_symbol(
                "func_b",
                "Function",
                "b",
                &SourceSpan::new("f.rs", 2, 0, 10),
                "Rust",
                None,
            )
            .expect("invariant: fresh graph accepts inserts");

        let edge_id = kg
            .add_edge(a, b, "calls", serde_json::json!({"location_line": 5}))
            .expect("invariant: fresh graph accepts edge inserts");
        assert!(edge_id > 0);
    }

    #[test]
    fn test_add_correlation_bidirectional() {
        let (_temp, kg) = open_kg();
        let sym = kg
            .add_symbol(
                "my_func",
                "Function",
                "a",
                &SourceSpan::new("f.rs", 1, 0, 10),
                "Rust",
                None,
            )
            .expect("invariant: fresh graph accepts inserts");
        let disc = kg
            .add_discovery("claude1", "Symbol", "my_func", serde_json::json!({}))
            .expect("invariant: fresh graph accepts inserts");

        kg.add_correlation(disc, sym, 0.95, "claude1")
            .expect("invariant: fresh graph accepts edge inserts");

        let outgoing = kg
            .neighbors(disc, "correlates", Direction::Outgoing)
            .expect("invariant: traversal on known graph succeeds");
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].name, "my_func");

        let incoming = kg
            .neighbors(sym, "correlates", Direction::Incoming)
            .expect("invariant: traversal on known graph succeeds");
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].name, "my_func");
    }

    #[test]
    fn test_callers_of() {
        let (_temp, kg) = open_kg();
        let target = kg
            .add_symbol(
                "target_func",
                "Function",
                "t",
                &SourceSpan::new("f.rs", 1, 0, 10),
                "Rust",
                None,
            )
            .expect("invariant: fresh graph accepts inserts");
        let caller_a = kg
            .add_symbol(
                "caller_a",
                "Function",
                "a",
                &SourceSpan::new("f.rs", 5, 0, 10),
                "Rust",
                None,
            )
            .expect("invariant: fresh graph accepts inserts");
        let caller_b = kg
            .add_symbol(
                "caller_b",
                "Function",
                "b",
                &SourceSpan::new("f.rs", 10, 0, 10),
                "Rust",
                None,
            )
            .expect("invariant: fresh graph accepts inserts");

        kg.add_edge(caller_a, target, "calls", serde_json::json!({}))
            .expect("invariant: fresh graph accepts edge inserts");
        kg.add_edge(caller_b, target, "calls", serde_json::json!({}))
            .expect("invariant: fresh graph accepts edge inserts");

        let callers = kg
            .callers_of(target, 1)
            .expect("invariant: traversal on known graph succeeds");
        assert_eq!(callers.len(), 2);
    }

    #[test]
    fn test_callees_of() {
        let (_temp, kg) = open_kg();
        let func = kg
            .add_symbol(
                "func",
                "Function",
                "f",
                &SourceSpan::new("f.rs", 1, 0, 10),
                "Rust",
                None,
            )
            .expect("invariant: fresh graph accepts inserts");
        let callee_a = kg
            .add_symbol(
                "callee_a",
                "Function",
                "a",
                &SourceSpan::new("f.rs", 5, 0, 10),
                "Rust",
                None,
            )
            .expect("invariant: fresh graph accepts inserts");
        let callee_b = kg
            .add_symbol(
                "callee_b",
                "Function",
                "b",
                &SourceSpan::new("f.rs", 10, 0, 10),
                "Rust",
                None,
            )
            .expect("invariant: fresh graph accepts inserts");

        kg.add_edge(func, callee_a, "calls", serde_json::json!({}))
            .expect("invariant: fresh graph accepts edge inserts");
        kg.add_edge(func, callee_b, "calls", serde_json::json!({}))
            .expect("invariant: fresh graph accepts edge inserts");

        let callees = kg
            .callees_of(func, 1)
            .expect("invariant: traversal on known graph succeeds");
        assert_eq!(callees.len(), 2);
    }

    #[test]
    fn test_correlated_nodes() {
        let (_temp, kg) = open_kg();
        let sym = kg
            .add_symbol(
                "my_func",
                "Function",
                "a",
                &SourceSpan::new("f.rs", 1, 0, 10),
                "Rust",
                None,
            )
            .expect("invariant: fresh graph accepts inserts");
        let disc1 = kg
            .add_discovery("agent1", "Symbol", "my_func", serde_json::json!({}))
            .expect("invariant: fresh graph accepts inserts");
        let disc2 = kg
            .add_discovery("agent2", "CFG", "my_func", serde_json::json!({}))
            .expect("invariant: fresh graph accepts inserts");

        kg.add_correlation(disc1, sym, 0.9, "agent1")
            .expect("invariant: fresh graph accepts edge inserts");
        kg.add_correlation(disc2, sym, 0.8, "agent2")
            .expect("invariant: fresh graph accepts edge inserts");

        let correlated = kg
            .correlated(sym)
            .expect("invariant: traversal on known graph succeeds");
        assert_eq!(correlated.len(), 2);
    }

    #[test]
    fn test_affected_by() {
        let (_temp, kg) = open_kg();
        let sym = kg
            .add_symbol(
                "process_payment",
                "Function",
                "a",
                &SourceSpan::new("f.rs", 1, 0, 10),
                "Rust",
                None,
            )
            .expect("invariant: fresh graph accepts inserts");
        let issue = kg
            .add_issue("high", "race condition", None)
            .expect("invariant: fresh graph accepts inserts");

        kg.add_edge(issue, sym, "affects", serde_json::json!({}))
            .expect("invariant: fresh graph accepts edge inserts");

        let affected = kg
            .affected_by(sym, 1)
            .expect("invariant: traversal on known graph succeeds");
        assert_eq!(affected.len(), 1);
    }

    #[test]
    fn test_shortest_path() {
        let (_temp, kg) = open_kg();
        let a = kg
            .add_symbol(
                "a",
                "Function",
                "a",
                &SourceSpan::new("f.rs", 1, 0, 10),
                "Rust",
                None,
            )
            .expect("invariant: fresh graph accepts inserts");
        let b = kg
            .add_symbol(
                "b",
                "Function",
                "b",
                &SourceSpan::new("f.rs", 2, 0, 10),
                "Rust",
                None,
            )
            .expect("invariant: fresh graph accepts inserts");
        let c = kg
            .add_symbol(
                "c",
                "Function",
                "c",
                &SourceSpan::new("f.rs", 3, 0, 10),
                "Rust",
                None,
            )
            .expect("invariant: fresh graph accepts inserts");

        kg.add_edge(a, b, "calls", serde_json::json!({}))
            .expect("invariant: fresh graph accepts edge inserts");
        kg.add_edge(b, c, "calls", serde_json::json!({}))
            .expect("invariant: fresh graph accepts edge inserts");

        let path = kg
            .shortest_path(a, c)
            .expect("invariant: algorithm on valid graph succeeds");
        assert!(path.is_some());
        let path = path.expect("invariant: connected nodes have a path");
        assert!(path.contains(&a));
        assert!(path.contains(&c));
    }

    #[test]
    fn test_reachability() {
        let (_temp, kg) = open_kg();
        let a = kg
            .add_symbol(
                "a",
                "Function",
                "a",
                &SourceSpan::new("f.rs", 1, 0, 10),
                "Rust",
                None,
            )
            .expect("invariant: fresh graph accepts inserts");
        let b = kg
            .add_symbol(
                "b",
                "Function",
                "b",
                &SourceSpan::new("f.rs", 2, 0, 10),
                "Rust",
                None,
            )
            .expect("invariant: fresh graph accepts inserts");
        let c = kg
            .add_symbol(
                "c",
                "Function",
                "c",
                &SourceSpan::new("f.rs", 3, 0, 10),
                "Rust",
                None,
            )
            .expect("invariant: fresh graph accepts inserts");

        kg.add_edge(a, b, "calls", serde_json::json!({}))
            .expect("invariant: fresh graph accepts edge inserts");
        kg.add_edge(b, c, "calls", serde_json::json!({}))
            .expect("invariant: fresh graph accepts edge inserts");

        let reachable = kg
            .reachability(a)
            .expect("invariant: algorithm on valid graph succeeds");
        assert!(reachable.contains(&b));
        assert!(reachable.contains(&c));
    }

    #[test]
    fn test_k_hop() {
        let (_temp, kg) = open_kg();
        let a = kg
            .add_symbol(
                "a",
                "Function",
                "a",
                &SourceSpan::new("f.rs", 1, 0, 10),
                "Rust",
                None,
            )
            .expect("invariant: fresh graph accepts inserts");
        let b = kg
            .add_symbol(
                "b",
                "Function",
                "b",
                &SourceSpan::new("f.rs", 2, 0, 10),
                "Rust",
                None,
            )
            .expect("invariant: fresh graph accepts inserts");
        let c = kg
            .add_symbol(
                "c",
                "Function",
                "c",
                &SourceSpan::new("f.rs", 3, 0, 10),
                "Rust",
                None,
            )
            .expect("invariant: fresh graph accepts inserts");

        kg.add_edge(a, b, "calls", serde_json::json!({}))
            .expect("invariant: fresh graph accepts edge inserts");
        kg.add_edge(b, c, "calls", serde_json::json!({}))
            .expect("invariant: fresh graph accepts edge inserts");

        let hop1 = kg
            .k_hop(a, 1, Direction::Outgoing)
            .expect("invariant: algorithm on valid graph succeeds");
        assert!(hop1.contains(&b));

        let hop2 = kg
            .k_hop(a, 2, Direction::Outgoing)
            .expect("invariant: algorithm on valid graph succeeds");
        assert!(hop2.contains(&c));
    }
}

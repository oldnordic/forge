//! Plan graph — stores all plan artifacts as sqlitegraph nodes and edges.
//!
//! Every artifact (requirement, plan, task, gate, gate result, semgrep finding,
//! approval, rejection, discovered knowledge) is a node. Every relationship
//! (decomposes into, validated by, approved by, informed by) is an edge.
//! Queryable via pattern matching for the dashboard.

use serde::{Deserialize, Serialize};

use super::explorer::DiscoveredKnowledge;
use super::gate::GateResult;
use super::semgrep::SemgrepFinding;

/// Node kinds for the plan graph.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum PlanNodeKind {
    Requirement,
    Plan,
    Task,
    Decision,
    Constraint,
    Gate,
    GateResult,
    SemgrepFinding,
    Approval,
    Rejection,
    DiscoveredKnowledge,
}

impl PlanNodeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Requirement => "Requirement",
            Self::Plan => "Plan",
            Self::Task => "Task",
            Self::Decision => "Decision",
            Self::Constraint => "Constraint",
            Self::Gate => "Gate",
            Self::GateResult => "GateResult",
            Self::SemgrepFinding => "SemgrepFinding",
            Self::Approval => "Approval",
            Self::Rejection => "Rejection",
            Self::DiscoveredKnowledge => "DiscoveredKnowledge",
        }
    }
}

/// Edge kinds for the plan graph.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum PlanEdgeKind {
    HasRequirement,
    DecomposesInto,
    Implements,
    DependsOn,
    ValidatedBy,
    AssignedTo,
    Approved,
    Rejected,
    FoundIn,
    DetectedBy,
    Checks,
    InformedBy,
    RelatedTo,
}

impl PlanEdgeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::HasRequirement => "HAS_REQUIREMENT",
            Self::DecomposesInto => "DECOMPOSES_INTO",
            Self::Implements => "IMPLEMENTS",
            Self::DependsOn => "DEPENDS_ON",
            Self::ValidatedBy => "VALIDATED_BY",
            Self::AssignedTo => "ASSIGNED_TO",
            Self::Approved => "APPROVED",
            Self::Rejected => "REJECTED",
            Self::FoundIn => "FOUND_IN",
            Self::DetectedBy => "DETECTED_BY",
            Self::Checks => "CHECKS",
            Self::InformedBy => "INFORMED_BY",
            Self::RelatedTo => "RELATED_TO",
        }
    }
}

/// Plan graph backed by sqlitegraph.
///
/// Stores every artifact as a node with kind = PlanNodeKind::as_str(),
/// and every relationship as an edge with edge_type = PlanEdgeKind::as_str().
#[cfg(feature = "sqlite")]
pub struct PlanGraph {
    graph: sqlitegraph::SqliteGraph,
}

#[cfg(feature = "sqlite")]
impl PlanGraph {
    /// Open a plan graph from a file path.
    pub fn open(path: &std::path::Path) -> anyhow::Result<Self> {
        let graph = sqlitegraph::SqliteGraph::open(path)?;
        Ok(Self { graph })
    }

    /// Create an in-memory plan graph (for testing).
    pub fn open_in_memory() -> anyhow::Result<Self> {
        let graph = sqlitegraph::SqliteGraph::open_in_memory()?;
        Ok(Self { graph })
    }

    /// Add a requirement node. Returns the node ID.
    pub fn add_requirement(&mut self, title: &str, description: &str) -> anyhow::Result<i64> {
        let entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: PlanNodeKind::Requirement.as_str().to_string(),
            name: title.to_string(),
            file_path: None,
            data: serde_json::json!({ "description": description }),
        };
        let id = self.graph.insert_entity(&entity)?;
        Ok(id)
    }

    /// Add a plan node and link it to requirements. Returns the plan node ID.
    pub fn add_plan(&mut self, name: &str, requirement_ids: &[i64]) -> anyhow::Result<i64> {
        let entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: PlanNodeKind::Plan.as_str().to_string(),
            name: name.to_string(),
            file_path: None,
            data: serde_json::json!({}),
        };
        let plan_id = self.graph.insert_entity(&entity)?;

        for req_id in requirement_ids {
            let edge = sqlitegraph::GraphEdge {
                id: 0,
                from_id: plan_id,
                to_id: *req_id,
                edge_type: PlanEdgeKind::HasRequirement.as_str().to_string(),
                data: serde_json::json!({}),
            };
            self.graph.insert_edge(&edge)?;
        }

        Ok(plan_id)
    }

    /// Record that a plan was informed by discovered knowledge.
    pub fn link_knowledge(
        &mut self,
        plan_id: i64,
        knowledge: &DiscoveredKnowledge,
    ) -> anyhow::Result<i64> {
        let entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: PlanNodeKind::DiscoveredKnowledge.as_str().to_string(),
            name: knowledge.title.clone(),
            file_path: None,
            data: serde_json::to_value(knowledge)?,
        };
        let knowledge_id = self.graph.insert_entity(&entity)?;

        let edge = sqlitegraph::GraphEdge {
            id: 0,
            from_id: plan_id,
            to_id: knowledge_id,
            edge_type: PlanEdgeKind::InformedBy.as_str().to_string(),
            data: serde_json::json!({}),
        };
        self.graph.insert_edge(&edge)?;

        Ok(knowledge_id)
    }

    /// Query all knowledge that informed a plan.
    /// Uses pattern matching to find INFORMED_BY edges from the plan.
    pub fn get_plan_knowledge(&self, plan_id: i64) -> anyhow::Result<Vec<DiscoveredKnowledge>> {
        let pattern = sqlitegraph::PatternTriple::new(PlanEdgeKind::InformedBy.as_str());
        let matches = self.graph.match_triples(&pattern)?;

        let mut results = Vec::new();
        for triple in matches {
            if triple.start_id == plan_id {
                if let Ok(entity) = self.graph.get_entity(triple.end_id) {
                    if entity.kind == PlanNodeKind::DiscoveredKnowledge.as_str() {
                        if let Ok(dk) = serde_json::from_value::<DiscoveredKnowledge>(entity.data) {
                            results.push(dk);
                        }
                    }
                }
            }
        }
        Ok(results)
    }

    /// Add a gate result node and link to a task.
    pub fn add_gate_result(&mut self, task_id: i64, result: &GateResult) -> anyhow::Result<i64> {
        let entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: PlanNodeKind::GateResult.as_str().to_string(),
            name: result.gate_name.clone(),
            file_path: None,
            data: serde_json::to_value(result)?,
        };
        let result_id = self.graph.insert_entity(&entity)?;

        let edge = sqlitegraph::GraphEdge {
            id: 0,
            from_id: task_id,
            to_id: result_id,
            edge_type: PlanEdgeKind::ValidatedBy.as_str().to_string(),
            data: serde_json::json!({}),
        };
        self.graph.insert_edge(&edge)?;

        Ok(result_id)
    }

    /// Add a semgrep finding node and link to a gate result.
    pub fn add_semgrep_finding(
        &mut self,
        gate_result_id: i64,
        finding: &SemgrepFinding,
    ) -> anyhow::Result<i64> {
        let entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: PlanNodeKind::SemgrepFinding.as_str().to_string(),
            name: format!("{}:{}", finding.check_id, finding.file),
            file_path: Some(finding.file.clone()),
            data: serde_json::to_value(finding)?,
        };
        let finding_id = self.graph.insert_entity(&entity)?;

        let edge = sqlitegraph::GraphEdge {
            id: 0,
            from_id: gate_result_id,
            to_id: finding_id,
            edge_type: PlanEdgeKind::FoundIn.as_str().to_string(),
            data: serde_json::json!({}),
        };
        self.graph.insert_edge(&edge)?;

        Ok(finding_id)
    }

    /// Approve a plan — creates an Approval node and APPROVED edge.
    pub fn approve(&mut self, plan_id: i64, approver: &str) -> anyhow::Result<i64> {
        let entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: PlanNodeKind::Approval.as_str().to_string(),
            name: format!("approved_by_{}", approver),
            file_path: None,
            data: serde_json::json!({ "approver": approver }),
        };
        let approval_id = self.graph.insert_entity(&entity)?;

        let edge = sqlitegraph::GraphEdge {
            id: 0,
            from_id: plan_id,
            to_id: approval_id,
            edge_type: PlanEdgeKind::Approved.as_str().to_string(),
            data: serde_json::json!({}),
        };
        self.graph.insert_edge(&edge)?;

        Ok(approval_id)
    }

    /// Reject a plan — creates a Rejection node and REJECTED edge.
    pub fn reject(&mut self, plan_id: i64, reason: &str) -> anyhow::Result<i64> {
        let entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: PlanNodeKind::Rejection.as_str().to_string(),
            name: "rejected".to_string(),
            file_path: None,
            data: serde_json::json!({ "reason": reason }),
        };
        let rejection_id = self.graph.insert_entity(&entity)?;

        let edge = sqlitegraph::GraphEdge {
            id: 0,
            from_id: plan_id,
            to_id: rejection_id,
            edge_type: PlanEdgeKind::Rejected.as_str().to_string(),
            data: serde_json::json!({}),
        };
        self.graph.insert_edge(&edge)?;

        Ok(rejection_id)
    }

    /// Get the underlying sqlitegraph reference.
    pub fn graph(&self) -> &sqlitegraph::SqliteGraph {
        &self.graph
    }

    /// Count all edges of a given type from a source node.
    /// Useful for verifying graph structure in tests.
    pub fn count_outgoing(&self, from_id: i64, edge_type: &str) -> anyhow::Result<usize> {
        let pattern = sqlitegraph::PatternTriple::new(edge_type);
        let matches = self.graph.match_triples(&pattern)?;
        let count = matches.iter().filter(|t| t.start_id == from_id).count();
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_discovered_knowledge() -> DiscoveredKnowledge {
        DiscoveredKnowledge {
            title: "Sparse inference pattern".to_string(),
            kind: "Knowledge".to_string(),
            summary: "4D spatial graph traversal pattern".to_string(),
            source: "wiki".to_string(),
            discovery_method: "semantic".to_string(),
            relevance: 0.92,
            related: vec!["memoria".to_string()],
            is_historical: false,
        }
    }

    fn make_gate_result(passed: bool) -> GateResult {
        GateResult {
            gate_name: "semgrep".to_string(),
            passed,
            exit_code: if passed { 0 } else { 1 },
            stdout: String::new(),
            structured_output: None,
            errors: if passed { 0 } else { 3 },
            warnings: 0,
            duration_ms: 500,
        }
    }

    fn make_semgrep_finding() -> SemgrepFinding {
        SemgrepFinding {
            check_id: "llm-sql-injection".to_string(),
            file: "src/db.py".to_string(),
            start_line: 10,
            end_line: 12,
            message: "SQL injection via string concatenation".to_string(),
            severity: "ERROR".to_string(),
            category: Some("security".to_string()),
        }
    }

    #[test]
    fn test_add_requirement_creates_node() -> anyhow::Result<()> {
        let mut pg = PlanGraph::open_in_memory()?;
        let id = pg.add_requirement("Auth system", "Implement OAuth2 login")?;
        assert!(id > 0);

        let entity = pg.graph().get_entity(id)?;
        assert_eq!(entity.kind, "Requirement");
        assert_eq!(entity.name, "Auth system");
        assert_eq!(entity.data["description"], "Implement OAuth2 login");
        Ok(())
    }

    #[test]
    fn test_add_plan_links_to_requirements() -> anyhow::Result<()> {
        let mut pg = PlanGraph::open_in_memory()?;
        let req1 = pg.add_requirement("Req 1", "First")?;
        let req2 = pg.add_requirement("Req 2", "Second")?;
        let plan_id = pg.add_plan("My plan", &[req1, req2])?;

        let entity = pg.graph().get_entity(plan_id)?;
        assert_eq!(entity.kind, "Plan");

        // Verify 2 HAS_REQUIREMENT edges from plan
        let count = pg.count_outgoing(plan_id, "HAS_REQUIREMENT")?;
        assert_eq!(count, 2);
        Ok(())
    }

    #[test]
    fn test_link_knowledge_creates_informed_by_edge() -> anyhow::Result<()> {
        let mut pg = PlanGraph::open_in_memory()?;
        let req = pg.add_requirement("Req", "Need")?;
        let plan_id = pg.add_plan("Plan", &[req])?;
        let dk = make_discovered_knowledge();
        let knowledge_id = pg.link_knowledge(plan_id, &dk)?;

        // Verify knowledge node
        let entity = pg.graph().get_entity(knowledge_id)?;
        assert_eq!(entity.kind, "DiscoveredKnowledge");
        assert_eq!(entity.name, "Sparse inference pattern");

        // Verify INFORMED_BY edge
        let count = pg.count_outgoing(plan_id, "INFORMED_BY")?;
        assert_eq!(count, 1);
        Ok(())
    }

    #[test]
    fn test_get_plan_knowledge_roundtrip() -> anyhow::Result<()> {
        let mut pg = PlanGraph::open_in_memory()?;
        let req = pg.add_requirement("Req", "Need")?;
        let plan_id = pg.add_plan("Plan", &[req])?;
        let dk = make_discovered_knowledge();
        pg.link_knowledge(plan_id, &dk)?;

        let retrieved = pg.get_plan_knowledge(plan_id)?;
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].title, "Sparse inference pattern");
        assert!((retrieved[0].relevance - 0.92).abs() < f64::EPSILON);
        Ok(())
    }

    #[test]
    fn test_gate_result_links_to_task() -> anyhow::Result<()> {
        let mut pg = PlanGraph::open_in_memory()?;
        // Simulate a task node
        let task_entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: "Task".to_string(),
            name: "Implement auth".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        };
        let task_id = pg.graph().insert_entity(&task_entity)?;

        let result = make_gate_result(true);
        let result_id = pg.add_gate_result(task_id, &result)?;

        let entity = pg.graph().get_entity(result_id)?;
        assert_eq!(entity.kind, "GateResult");
        assert_eq!(entity.name, "semgrep");

        let count = pg.count_outgoing(task_id, "VALIDATED_BY")?;
        assert_eq!(count, 1);
        Ok(())
    }

    #[test]
    fn test_semgrep_finding_links_to_gate_result() -> anyhow::Result<()> {
        let mut pg = PlanGraph::open_in_memory()?;
        let task_entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: "Task".to_string(),
            name: "Task".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        };
        let task_id = pg.graph().insert_entity(&task_entity)?;
        let gate_result = make_gate_result(false);
        let result_id = pg.add_gate_result(task_id, &gate_result)?;

        let finding = make_semgrep_finding();
        let finding_id = pg.add_semgrep_finding(result_id, &finding)?;

        let entity = pg.graph().get_entity(finding_id)?;
        assert_eq!(entity.kind, "SemgrepFinding");
        assert_eq!(entity.name, "llm-sql-injection:src/db.py");

        let count = pg.count_outgoing(result_id, "FOUND_IN")?;
        assert_eq!(count, 1);
        Ok(())
    }

    #[test]
    fn test_approve_creates_approval_edge() -> anyhow::Result<()> {
        let mut pg = PlanGraph::open_in_memory()?;
        let req = pg.add_requirement("Req", "Need")?;
        let plan_id = pg.add_plan("Plan", &[req])?;
        let approval_id = pg.approve(plan_id, "user")?;

        let entity = pg.graph().get_entity(approval_id)?;
        assert_eq!(entity.kind, "Approval");
        assert_eq!(entity.data["approver"], "user");

        let count = pg.count_outgoing(plan_id, "APPROVED")?;
        assert_eq!(count, 1);
        Ok(())
    }

    #[test]
    fn test_reject_creates_rejection_edge() -> anyhow::Result<()> {
        let mut pg = PlanGraph::open_in_memory()?;
        let req = pg.add_requirement("Req", "Need")?;
        let plan_id = pg.add_plan("Plan", &[req])?;
        let rejection_id = pg.reject(plan_id, "Too complex")?;

        let entity = pg.graph().get_entity(rejection_id)?;
        assert_eq!(entity.kind, "Rejection");
        assert_eq!(entity.data["reason"], "Too complex");

        let count = pg.count_outgoing(plan_id, "REJECTED")?;
        assert_eq!(count, 1);
        Ok(())
    }

    #[test]
    fn test_full_plan_graph_roundtrip() -> anyhow::Result<()> {
        let mut pg = PlanGraph::open_in_memory()?;

        // 1. Add requirement
        let req = pg.add_requirement("Auth", "OAuth2 login")?;

        // 2. Explore knowledge
        let dk = make_discovered_knowledge();

        // 3. Create plan informed by knowledge
        let plan_id = pg.add_plan("Auth plan", &[req])?;
        pg.link_knowledge(plan_id, &dk)?;

        // 4. Approve plan
        pg.approve(plan_id, "user")?;

        // 5. Create task
        let task_entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: "Task".to_string(),
            name: "Implement OAuth2".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        };
        let task_id = pg.graph().insert_entity(&task_entity)?;

        // 6. Run gate
        let gate_result = make_gate_result(false);
        let result_id = pg.add_gate_result(task_id, &gate_result)?;

        // 7. Record semgrep finding
        let finding = make_semgrep_finding();
        pg.add_semgrep_finding(result_id, &finding)?;

        // Verify graph structure
        let plan_entity = pg.graph().get_entity(plan_id)?;
        assert_eq!(plan_entity.kind, "Plan");

        // Plan should have: HAS_REQUIREMENT(1) + INFORMED_BY(1) + APPROVED(1) = 3 edges
        let has_req = pg.count_outgoing(plan_id, "HAS_REQUIREMENT")?;
        let informed = pg.count_outgoing(plan_id, "INFORMED_BY")?;
        let approved = pg.count_outgoing(plan_id, "APPROVED")?;
        assert_eq!(has_req, 1);
        assert_eq!(informed, 1);
        assert_eq!(approved, 1);

        // Verify we can retrieve the knowledge
        let knowledge = pg.get_plan_knowledge(plan_id)?;
        assert_eq!(knowledge.len(), 1);

        Ok(())
    }
}

//! Plan graph — stores all plan artifacts as sqlitegraph nodes and edges.
//!
//! Every artifact (requirement, plan, task, gate, gate result, semgrep finding,
//! approval, rejection, discovered knowledge) is a node. Every relationship
//! (decomposes into, validated by, approved by, informed by) is an edge.
//! Queryable via pattern matching for the dashboard.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::explorer::DiscoveredKnowledge;
use super::gate::GateResult;
use super::semgrep::SemgrepFinding;

// ─── Node kinds ─────────────────────────────────────────────────────────────

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
    // ─── Chain-of-custody variants ───
    PlanSection,
    SubagentRun,
    LogEntry,
    ToolCall,
    ReasoningStep,
    Deliverable,
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
            Self::PlanSection => "PlanSection",
            Self::SubagentRun => "SubagentRun",
            Self::LogEntry => "LogEntry",
            Self::ToolCall => "ToolCall",
            Self::ReasoningStep => "ReasoningStep",
            Self::Deliverable => "Deliverable",
        }
    }
}

// ─── Edge kinds ─────────────────────────────────────────────────────────────

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
    // ─── Chain-of-custody edges ───
    ExecutedBy,
    Logged,
    Called,
    Reasoned,
    Produced,
    AddressesIn,
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
            Self::ExecutedBy => "EXECUTED_BY",
            Self::Logged => "LOGGED",
            Self::Called => "CALLED",
            Self::Reasoned => "REASONED",
            Self::Produced => "PRODUCED",
            Self::AddressesIn => "ADDRESSES_IN",
        }
    }
}

// ─── Chain-of-custody data structs ──────────────────────────────────────────

/// Status of a subagent run.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SubagentStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Log severity.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    Debug,
}

/// Data stored on a PlanSection node.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanSectionData {
    pub order: usize,
    pub title: String,
    pub description: String,
}

/// Data stored on a SubagentRun node.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubagentRunData {
    pub run_id: String,
    pub agent_name: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: SubagentStatus,
    pub input_prompt: String,
    pub output_summary: Option<String>,
}

/// Data stored on a LogEntry node.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogEntryData {
    pub level: LogLevel,
    pub message: String,
    pub timestamp: DateTime<Utc>,
}

/// Data stored on a ToolCall node.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCallData {
    pub tool: String,
    pub args: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub timestamp: DateTime<Utc>,
}

/// Data stored on a ReasoningStep node.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReasoningStepData {
    pub thinking: String,
    pub decision: String,
    pub timestamp: DateTime<Utc>,
}

/// Data stored on a Deliverable node.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeliverableData {
    pub file_path: String,
    pub sha256: String,
    pub diff_summary: Option<String>,
    pub timestamp: DateTime<Utc>,
}

// ─── Query result types ─────────────────────────────────────────────────────

/// A single node in a custody chain.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustodyNode {
    pub id: i64,
    pub kind: String,
    pub data: serde_json::Value,
    pub timestamp: Option<DateTime<Utc>>,
}

/// Full chain from a requirement down through sections, tasks, runs, and deliverables.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustodyChain {
    pub requirement_id: i64,
    pub sections: Vec<CustodySection>,
}

/// A section with its contained tasks.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustodySection {
    pub section_id: i64,
    pub section: PlanSectionData,
    pub tasks: Vec<CustodyTask>,
}

/// A task with its subagent runs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustodyTask {
    pub task_id: i64,
    pub runs: Vec<CustodyRun>,
}

/// A single run with all its recorded artifacts.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustodyRun {
    pub run_id: i64,
    pub run: SubagentRunData,
    pub logs: Vec<LogEntryData>,
    pub calls: Vec<ToolCallData>,
    pub reasoning: Vec<ReasoningStepData>,
    pub deliverables: Vec<DeliverableData>,
}

// ─── PlanGraph ──────────────────────────────────────────────────────────────

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

    // ═══════════════════════════════════════════════════════════════════════════
    // Chain-of-custody methods
    // ═══════════════════════════════════════════════════════════════════════════

    // ── Section management ────────────────────────────────────────────────────

    /// Add a plan section (ordered) linked to a requirement. Returns section ID.
    pub fn add_section(
        &mut self,
        plan_id: i64,
        requirement_id: i64,
        data: &PlanSectionData,
    ) -> anyhow::Result<i64> {
        let entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: PlanNodeKind::PlanSection.as_str().to_string(),
            name: data.title.clone(),
            file_path: None,
            data: serde_json::to_value(data)?,
        };
        let section_id = self.graph.insert_entity(&entity)?;

        // Section → Requirement (ADDRESSES_IN)
        let edge = sqlitegraph::GraphEdge {
            id: 0,
            from_id: section_id,
            to_id: requirement_id,
            edge_type: PlanEdgeKind::AddressesIn.as_str().to_string(),
            data: serde_json::json!({}),
        };
        self.graph.insert_edge(&edge)?;

        // Plan → Section (DECOMPOSES_INTO)
        let edge2 = sqlitegraph::GraphEdge {
            id: 0,
            from_id: plan_id,
            to_id: section_id,
            edge_type: PlanEdgeKind::DecomposesInto.as_str().to_string(),
            data: serde_json::json!({ "order": data.order }),
        };
        self.graph.insert_edge(&edge2)?;

        Ok(section_id)
    }

    /// Return all sections linked to a plan, sorted by order.
    pub fn sections_in_order(&self, plan_id: i64) -> anyhow::Result<Vec<(i64, PlanSectionData)>> {
        let pattern = sqlitegraph::PatternTriple::new(PlanEdgeKind::DecomposesInto.as_str());
        let matches = self.graph.match_triples(&pattern)?;

        let mut sections = Vec::new();
        for triple in matches {
            if triple.start_id == plan_id {
                if let Ok(entity) = self.graph.get_entity(triple.end_id) {
                    if entity.kind == PlanNodeKind::PlanSection.as_str() {
                        if let Ok(data) = serde_json::from_value::<PlanSectionData>(entity.data) {
                            sections.push((triple.end_id, data));
                        }
                    }
                }
            }
        }
        sections.sort_by_key(|(_, d)| d.order);
        Ok(sections)
    }

    // ── Subagent run lifecycle ──────────────────────────────────────────────

    /// Begin a subagent run for a task. Returns run node ID.
    pub fn begin_subagent_run(
        &mut self,
        task_id: i64,
        data: &SubagentRunData,
    ) -> anyhow::Result<i64> {
        let entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: PlanNodeKind::SubagentRun.as_str().to_string(),
            name: format!("{}_{}", data.agent_name, data.run_id),
            file_path: None,
            data: serde_json::to_value(data)?,
        };
        let run_id = self.graph.insert_entity(&entity)?;

        let edge = sqlitegraph::GraphEdge {
            id: 0,
            from_id: task_id,
            to_id: run_id,
            edge_type: PlanEdgeKind::ExecutedBy.as_str().to_string(),
            data: serde_json::json!({}),
        };
        self.graph.insert_edge(&edge)?;

        Ok(run_id)
    }

    /// Mark a subagent run as completed or failed.
    pub fn complete_subagent_run(
        &mut self,
        run_node_id: i64,
        status: SubagentStatus,
        summary: &str,
    ) -> anyhow::Result<()> {
        let mut entity = self.graph.get_entity(run_node_id)?;
        let mut data: SubagentRunData = serde_json::from_value(entity.data)?;
        data.status = status;
        data.completed_at = Some(Utc::now());
        data.output_summary = Some(summary.to_string());
        entity.data = serde_json::to_value(data)?;
        self.graph.update_entity(&entity)?;
        Ok(())
    }

    // ── Recording artifacts within a run ────────────────────────────────────

    /// Record a log entry from a subagent run.
    pub fn record_log(&mut self, run_node_id: i64, data: &LogEntryData) -> anyhow::Result<i64> {
        let entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: PlanNodeKind::LogEntry.as_str().to_string(),
            name: format!("{:?}_{}", data.level, data.timestamp.timestamp()),
            file_path: None,
            data: serde_json::to_value(data)?,
        };
        let node_id = self.graph.insert_entity(&entity)?;

        let edge = sqlitegraph::GraphEdge {
            id: 0,
            from_id: run_node_id,
            to_id: node_id,
            edge_type: PlanEdgeKind::Logged.as_str().to_string(),
            data: serde_json::json!({}),
        };
        self.graph.insert_edge(&edge)?;

        Ok(node_id)
    }

    /// Record a tool call from a subagent run.
    pub fn record_tool_call(
        &mut self,
        run_node_id: i64,
        data: &ToolCallData,
    ) -> anyhow::Result<i64> {
        let entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: PlanNodeKind::ToolCall.as_str().to_string(),
            name: data.tool.clone(),
            file_path: None,
            data: serde_json::to_value(data)?,
        };
        let node_id = self.graph.insert_entity(&entity)?;

        let edge = sqlitegraph::GraphEdge {
            id: 0,
            from_id: run_node_id,
            to_id: node_id,
            edge_type: PlanEdgeKind::Called.as_str().to_string(),
            data: serde_json::json!({}),
        };
        self.graph.insert_edge(&edge)?;

        Ok(node_id)
    }

    /// Record a reasoning step from a subagent run.
    pub fn record_reasoning(
        &mut self,
        run_node_id: i64,
        data: &ReasoningStepData,
    ) -> anyhow::Result<i64> {
        let entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: PlanNodeKind::ReasoningStep.as_str().to_string(),
            name: data.decision.clone(),
            file_path: None,
            data: serde_json::to_value(data)?,
        };
        let node_id = self.graph.insert_entity(&entity)?;

        let edge = sqlitegraph::GraphEdge {
            id: 0,
            from_id: run_node_id,
            to_id: node_id,
            edge_type: PlanEdgeKind::Reasoned.as_str().to_string(),
            data: serde_json::json!({}),
        };
        self.graph.insert_edge(&edge)?;

        Ok(node_id)
    }

    /// Record a deliverable from a subagent run.
    pub fn record_deliverable(
        &mut self,
        run_node_id: i64,
        data: &DeliverableData,
    ) -> anyhow::Result<i64> {
        let entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: PlanNodeKind::Deliverable.as_str().to_string(),
            name: data.file_path.clone(),
            file_path: Some(data.file_path.clone()),
            data: serde_json::to_value(data)?,
        };
        let node_id = self.graph.insert_entity(&entity)?;

        let edge = sqlitegraph::GraphEdge {
            id: 0,
            from_id: run_node_id,
            to_id: node_id,
            edge_type: PlanEdgeKind::Produced.as_str().to_string(),
            data: serde_json::json!({}),
        };
        self.graph.insert_edge(&edge)?;

        Ok(node_id)
    }

    // ── Traversal queries ───────────────────────────────────────────────────

    /// Get all children of a node via a specific edge type (outgoing).
    fn children(
        &self,
        parent_id: i64,
        edge_type: &str,
        node_kind: &str,
    ) -> anyhow::Result<Vec<CustodyNode>> {
        let pattern = sqlitegraph::PatternTriple::new(edge_type);
        let matches = self.graph.match_triples(&pattern)?;
        let mut results = Vec::new();
        for triple in matches {
            if triple.start_id == parent_id {
                if let Ok(entity) = self.graph.get_entity(triple.end_id) {
                    if entity.kind == node_kind {
                        let ts = entity
                            .data
                            .get("timestamp")
                            .and_then(|v| serde_json::from_value::<DateTime<Utc>>(v.clone()).ok());
                        // Also try started_at / completed_at for subagent runs
                        let ts = ts.or_else(|| {
                            entity.data.get("started_at").and_then(|v| {
                                serde_json::from_value::<DateTime<Utc>>(v.clone()).ok()
                            })
                        });
                        results.push(CustodyNode {
                            id: triple.end_id,
                            kind: entity.kind,
                            data: entity.data,
                            timestamp: ts,
                        });
                    }
                }
            }
        }
        Ok(results)
    }

    /// Get the first parent of a node via a specific edge type (reverse lookup).
    fn parent(
        &self,
        child_id: i64,
        edge_type: &str,
        node_kind: &str,
    ) -> anyhow::Result<Option<CustodyNode>> {
        let pattern = sqlitegraph::PatternTriple::new(edge_type);
        let matches = self.graph.match_triples(&pattern)?;
        for triple in matches {
            if triple.end_id == child_id {
                if let Ok(entity) = self.graph.get_entity(triple.start_id) {
                    if entity.kind == node_kind {
                        let ts = entity
                            .data
                            .get("timestamp")
                            .and_then(|v| serde_json::from_value::<DateTime<Utc>>(v.clone()).ok());
                        return Ok(Some(CustodyNode {
                            id: triple.start_id,
                            kind: entity.kind,
                            data: entity.data,
                            timestamp: ts,
                        }));
                    }
                }
            }
        }
        Ok(None)
    }

    /// Walk backward from any node to its originating requirement.
    pub fn trace_backward(&self, node_id: i64) -> anyhow::Result<Vec<CustodyNode>> {
        let mut path = Vec::new();
        let mut current = node_id;

        loop {
            // Try to find parent of current
            let parent_node = self
                .parent(current, PlanEdgeKind::ExecutedBy.as_str(), "Task")
                .ok()
                .flatten()
                .or_else(|| {
                    self.parent(
                        current,
                        PlanEdgeKind::DecomposesInto.as_str(),
                        "PlanSection",
                    )
                    .ok()
                    .flatten()
                })
                .or_else(|| {
                    self.parent(current, PlanEdgeKind::HasRequirement.as_str(), "Plan")
                        .ok()
                        .flatten()
                })
                .or_else(|| {
                    self.parent(current, PlanEdgeKind::AddressesIn.as_str(), "Requirement")
                        .ok()
                        .flatten()
                })
                .or_else(|| {
                    self.parent(current, PlanEdgeKind::Logged.as_str(), "SubagentRun")
                        .ok()
                        .flatten()
                })
                .or_else(|| {
                    self.parent(current, PlanEdgeKind::Called.as_str(), "SubagentRun")
                        .ok()
                        .flatten()
                })
                .or_else(|| {
                    self.parent(current, PlanEdgeKind::Reasoned.as_str(), "SubagentRun")
                        .ok()
                        .flatten()
                })
                .or_else(|| {
                    self.parent(current, PlanEdgeKind::Produced.as_str(), "SubagentRun")
                        .ok()
                        .flatten()
                });

            if let Some(p) = parent_node {
                current = p.id;
                path.push(p);
            } else {
                break;
            }
        }

        path.reverse();
        Ok(path)
    }

    /// Collect all nodes under a root (by walking all custody edges), sorted by timestamp.
    pub fn timeline(&self, root_id: i64) -> anyhow::Result<Vec<CustodyNode>> {
        let edge_types: &[&str] = &[
            PlanEdgeKind::HasRequirement.as_str(),
            PlanEdgeKind::DecomposesInto.as_str(),
            PlanEdgeKind::Implements.as_str(),
            PlanEdgeKind::DependsOn.as_str(),
            PlanEdgeKind::ValidatedBy.as_str(),
            PlanEdgeKind::AssignedTo.as_str(),
            PlanEdgeKind::Approved.as_str(),
            PlanEdgeKind::Rejected.as_str(),
            PlanEdgeKind::FoundIn.as_str(),
            PlanEdgeKind::DetectedBy.as_str(),
            PlanEdgeKind::Checks.as_str(),
            PlanEdgeKind::InformedBy.as_str(),
            PlanEdgeKind::RelatedTo.as_str(),
            PlanEdgeKind::ExecutedBy.as_str(),
            PlanEdgeKind::Logged.as_str(),
            PlanEdgeKind::Called.as_str(),
            PlanEdgeKind::Reasoned.as_str(),
            PlanEdgeKind::Produced.as_str(),
            PlanEdgeKind::AddressesIn.as_str(),
        ];

        let mut visited = std::collections::HashSet::new();
        let mut queue = vec![root_id];
        let mut results = Vec::new();

        while let Some(id) = queue.pop() {
            if !visited.insert(id) {
                continue;
            }

            if let Ok(entity) = self.graph.get_entity(id) {
                let ts = entity
                    .data
                    .get("timestamp")
                    .and_then(|v| serde_json::from_value::<DateTime<Utc>>(v.clone()).ok());
                let ts = ts.or_else(|| {
                    entity
                        .data
                        .get("started_at")
                        .and_then(|v| serde_json::from_value::<DateTime<Utc>>(v.clone()).ok())
                });
                results.push(CustodyNode {
                    id,
                    kind: entity.kind.clone(),
                    data: entity.data,
                    timestamp: ts,
                });

                for et in edge_types {
                    let pattern = sqlitegraph::PatternTriple::new(*et);
                    let matches = self.graph.match_triples(&pattern)?;
                    for triple in matches {
                        if triple.start_id == id && !visited.contains(&triple.end_id) {
                            queue.push(triple.end_id);
                        }
                    }
                }
            }
        }

        results.sort_by(|a, b| match (a.timestamp, b.timestamp) {
            (Some(ta), Some(tb)) => ta.cmp(&tb),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });
        Ok(results)
    }
    pub fn find_gaps(&self, plan_id: i64) -> anyhow::Result<Vec<i64>> {
        // Get all tasks under this plan
        let pattern = sqlitegraph::PatternTriple::new(PlanEdgeKind::DecomposesInto.as_str());
        let all = self.graph.match_triples(&pattern)?;
        let mut gaps = Vec::new();

        // Find sections under plan, then tasks under sections
        for triple in &all {
            if triple.start_id == plan_id {
                // triple.end_id is a section — find tasks under it
                let task_pattern =
                    sqlitegraph::PatternTriple::new(PlanEdgeKind::DecomposesInto.as_str());
                let task_matches = self.graph.match_triples(&task_pattern)?;
                for task_triple in task_matches {
                    if task_triple.start_id == triple.end_id {
                        if let Ok(entity) = self.graph.get_entity(task_triple.end_id) {
                            if entity.kind == PlanNodeKind::Task.as_str() {
                                let exec = self.count_outgoing(
                                    task_triple.end_id,
                                    PlanEdgeKind::ExecutedBy.as_str(),
                                )?;
                                if exec == 0 {
                                    gaps.push(task_triple.end_id);
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(gaps)
    }

    /// Full forward trace from a requirement through all sections, tasks, runs, deliverables.
    pub fn trace_forward(&self, requirement_id: i64) -> anyhow::Result<CustodyChain> {
        let mut chain = CustodyChain {
            requirement_id,
            sections: Vec::new(),
        };

        // Find plans linked to this requirement (reverse of HAS_REQUIREMENT)
        let plan_pattern = sqlitegraph::PatternTriple::new(PlanEdgeKind::HasRequirement.as_str());
        let plans = self.graph.match_triples(&plan_pattern)?;
        let plan_ids: Vec<i64> = plans
            .iter()
            .filter(|t| t.end_id == requirement_id)
            .map(|t| t.start_id)
            .collect();

        // For each plan, get sections
        for plan_id in plan_ids {
            let section_nodes = self.children(
                plan_id,
                PlanEdgeKind::DecomposesInto.as_str(),
                PlanNodeKind::PlanSection.as_str(),
            )?;
            for section_node in section_nodes {
                let section_data: PlanSectionData = serde_json::from_value(section_node.data)?;

                let mut section = CustodySection {
                    section_id: section_node.id,
                    section: section_data,
                    tasks: Vec::new(),
                };

                // Tasks under this section
                let task_nodes = self.children(
                    section_node.id,
                    PlanEdgeKind::DecomposesInto.as_str(),
                    PlanNodeKind::Task.as_str(),
                )?;
                for task_node in task_nodes {
                    let mut task = CustodyTask {
                        task_id: task_node.id,
                        runs: Vec::new(),
                    };

                    // Runs under this task
                    let run_nodes = self.children(
                        task_node.id,
                        PlanEdgeKind::ExecutedBy.as_str(),
                        PlanNodeKind::SubagentRun.as_str(),
                    )?;
                    for run_node in run_nodes {
                        let run_data: SubagentRunData = serde_json::from_value(run_node.data)?;

                        let logs: Vec<LogEntryData> = self
                            .children(
                                run_node.id,
                                PlanEdgeKind::Logged.as_str(),
                                PlanNodeKind::LogEntry.as_str(),
                            )?
                            .into_iter()
                            .filter_map(|n| serde_json::from_value(n.data).ok())
                            .collect();

                        let calls: Vec<ToolCallData> = self
                            .children(
                                run_node.id,
                                PlanEdgeKind::Called.as_str(),
                                PlanNodeKind::ToolCall.as_str(),
                            )?
                            .into_iter()
                            .filter_map(|n| serde_json::from_value(n.data).ok())
                            .collect();

                        let reasoning: Vec<ReasoningStepData> = self
                            .children(
                                run_node.id,
                                PlanEdgeKind::Reasoned.as_str(),
                                PlanNodeKind::ReasoningStep.as_str(),
                            )?
                            .into_iter()
                            .filter_map(|n| serde_json::from_value(n.data).ok())
                            .collect();

                        let deliverables: Vec<DeliverableData> = self
                            .children(
                                run_node.id,
                                PlanEdgeKind::Produced.as_str(),
                                PlanNodeKind::Deliverable.as_str(),
                            )?
                            .into_iter()
                            .filter_map(|n| serde_json::from_value(n.data).ok())
                            .collect();

                        task.runs.push(CustodyRun {
                            run_id: run_node.id,
                            run: run_data,
                            logs,
                            calls,
                            reasoning,
                            deliverables,
                        });
                    }

                    section.tasks.push(task);
                }

                chain.sections.push(section);
            }
        }

        // Sort sections by order
        chain.sections.sort_by_key(|s| s.section.order);
        Ok(chain)
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

    // ═══════════════════════════════════════════════════════════════════════
    // Existing tests
    // ═══════════════════════════════════════════════════════════════════════

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

    // ═══════════════════════════════════════════════════════════════════════
    // Chain-of-custody tests (Task A / B / C)
    // ═══════════════════════════════════════════════════════════════════════

    fn make_section(order: usize) -> PlanSectionData {
        PlanSectionData {
            order,
            title: format!("Section {}", order),
            description: "Test section".to_string(),
        }
    }

    fn make_run(agent: &str, run_id: &str) -> SubagentRunData {
        SubagentRunData {
            run_id: run_id.to_string(),
            agent_name: agent.to_string(),
            started_at: Utc::now(),
            completed_at: None,
            status: SubagentStatus::Running,
            input_prompt: "do work".to_string(),
            output_summary: None,
        }
    }

    fn make_log() -> LogEntryData {
        LogEntryData {
            level: LogLevel::Info,
            message: "Starting operation".to_string(),
            timestamp: Utc::now(),
        }
    }

    fn make_tool_call() -> ToolCallData {
        ToolCallData {
            tool: "sed".to_string(),
            args: serde_json::json!({ "file": "src/lib.rs" }),
            result: Some(serde_json::json!({ "success": true })),
            exit_code: 0,
            duration_ms: 12,
            timestamp: Utc::now(),
        }
    }

    fn make_reasoning() -> ReasoningStepData {
        ReasoningStepData {
            thinking: "I should check the graph first".to_string(),
            decision: "Query magellan before editing".to_string(),
            timestamp: Utc::now(),
        }
    }

    fn make_deliverable(path: &str, hash: &str) -> DeliverableData {
        DeliverableData {
            file_path: path.to_string(),
            sha256: hash.to_string(),
            diff_summary: Some("+10 -2 lines".to_string()),
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_enum_variants_exist() {
        assert_eq!(PlanNodeKind::PlanSection.as_str(), "PlanSection");
        assert_eq!(PlanNodeKind::SubagentRun.as_str(), "SubagentRun");
        assert_eq!(PlanNodeKind::LogEntry.as_str(), "LogEntry");
        assert_eq!(PlanNodeKind::ToolCall.as_str(), "ToolCall");
        assert_eq!(PlanNodeKind::ReasoningStep.as_str(), "ReasoningStep");
        assert_eq!(PlanNodeKind::Deliverable.as_str(), "Deliverable");

        assert_eq!(PlanEdgeKind::ExecutedBy.as_str(), "EXECUTED_BY");
        assert_eq!(PlanEdgeKind::Logged.as_str(), "LOGGED");
        assert_eq!(PlanEdgeKind::Called.as_str(), "CALLED");
        assert_eq!(PlanEdgeKind::Reasoned.as_str(), "REASONED");
        assert_eq!(PlanEdgeKind::Produced.as_str(), "PRODUCED");
        assert_eq!(PlanEdgeKind::AddressesIn.as_str(), "ADDRESSES_IN");
    }

    #[test]
    fn test_add_section_creates_ordered_nodes() -> anyhow::Result<()> {
        let mut pg = PlanGraph::open_in_memory()?;
        let req = pg.add_requirement("Req", "Need")?;
        let plan_id = pg.add_plan("Plan", &[req])?;

        let s1 = pg.add_section(plan_id, req, &make_section(2))?;
        let s0 = pg.add_section(plan_id, req, &make_section(1))?;

        assert!(s1 > 0);
        assert!(s0 > 0);

        let sections = pg.sections_in_order(plan_id)?;
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].1.order, 1);
        assert_eq!(sections[1].1.order, 2);
        assert_eq!(sections[0].1.title, "Section 1");

        // Verify edges
        let dec_count = pg.count_outgoing(plan_id, "DECOMPOSES_INTO")?;
        assert_eq!(dec_count, 2);
        Ok(())
    }

    #[test]
    fn test_begin_and_complete_subagent_run() -> anyhow::Result<()> {
        let mut pg = PlanGraph::open_in_memory()?;
        let task_entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: "Task".to_string(),
            name: "Implement".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        };
        let task_id = pg.graph().insert_entity(&task_entity)?;

        let run_data = make_run("claude", "run-001");
        let run_node_id = pg.begin_subagent_run(task_id, &run_data)?;

        let entity = pg.graph().get_entity(run_node_id)?;
        assert_eq!(entity.kind, "SubagentRun");
        assert_eq!(entity.name, "claude_run-001");

        let exec_count = pg.count_outgoing(task_id, "EXECUTED_BY")?;
        assert_eq!(exec_count, 1);

        pg.complete_subagent_run(run_node_id, SubagentStatus::Completed, "Done")?;
        let updated = pg.graph().get_entity(run_node_id)?;
        let updated_data: SubagentRunData = serde_json::from_value(updated.data)?;
        assert_eq!(updated_data.status, SubagentStatus::Completed);
        assert_eq!(updated_data.output_summary, Some("Done".to_string()));
        assert!(updated_data.completed_at.is_some());
        Ok(())
    }

    #[test]
    fn test_record_log_tool_call_reasoning_deliverable() -> anyhow::Result<()> {
        let mut pg = PlanGraph::open_in_memory()?;
        let task_entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: "Task".to_string(),
            name: "Task".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        };
        let task_id = pg.graph().insert_entity(&task_entity)?;

        let run_data = make_run("gpt-4", "run-002");
        let run_id = pg.begin_subagent_run(task_id, &run_data)?;

        let log_id = pg.record_log(run_id, &make_log())?;
        let tool_id = pg.record_tool_call(run_id, &make_tool_call())?;
        let reason_id = pg.record_reasoning(run_id, &make_reasoning())?;
        let deliv_id = pg.record_deliverable(run_id, &make_deliverable("src/lib.rs", "abc123"))?;

        assert!(log_id > 0);
        assert!(tool_id > 0);
        assert!(reason_id > 0);
        assert!(deliv_id > 0);

        let log_count = pg.count_outgoing(run_id, "LOGGED")?;
        let tool_count = pg.count_outgoing(run_id, "CALLED")?;
        let reason_count = pg.count_outgoing(run_id, "REASONED")?;
        let deliv_count = pg.count_outgoing(run_id, "PRODUCED")?;
        assert_eq!(log_count, 1);
        assert_eq!(tool_count, 1);
        assert_eq!(reason_count, 1);
        assert_eq!(deliv_count, 1);

        // Verify node kinds
        assert_eq!(pg.graph().get_entity(log_id)?.kind, "LogEntry");
        assert_eq!(pg.graph().get_entity(tool_id)?.kind, "ToolCall");
        assert_eq!(pg.graph().get_entity(reason_id)?.kind, "ReasoningStep");
        assert_eq!(pg.graph().get_entity(deliv_id)?.kind, "Deliverable");
        Ok(())
    }

    #[test]
    fn test_trace_backward_from_tool_call_to_task() -> anyhow::Result<()> {
        let mut pg = PlanGraph::open_in_memory()?;
        let task_entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: "Task".to_string(),
            name: "Task".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        };
        let task_id = pg.graph().insert_entity(&task_entity)?;

        let run_data = make_run("agent", "run-003");
        let run_id = pg.begin_subagent_run(task_id, &run_data)?;
        let tool_id = pg.record_tool_call(run_id, &make_tool_call())?;

        let path = pg.trace_backward(tool_id)?;
        assert!(!path.is_empty());
        // Path: Task -> SubagentRun -> ToolCall (backward, so first node is Task)
        assert_eq!(path[0].kind, "Task");
        assert_eq!(path[0].id, task_id);
        Ok(())
    }

    #[test]
    fn test_timeline_sorted() -> anyhow::Result<()> {
        let mut pg = PlanGraph::open_in_memory()?;
        let req = pg.add_requirement("Req", "Desc")?;
        let plan_id = pg.add_plan("Plan", &[req])?;
        let _s = pg.add_section(plan_id, req, &make_section(1))?;

        let timeline = pg.timeline(plan_id)?;
        assert!(!timeline.is_empty());
        // Plan is the root, plus section
        assert!(timeline.iter().any(|n| n.kind == "Plan"));
        assert!(timeline.iter().any(|n| n.kind == "PlanSection"));
        Ok(())
    }

    #[test]
    fn test_find_gaps_returns_unrun_tasks() -> anyhow::Result<()> {
        let mut pg = PlanGraph::open_in_memory()?;
        let req = pg.add_requirement("Req", "Need")?;
        let plan_id = pg.add_plan("Plan", &[req])?;
        let section_id = pg.add_section(plan_id, req, &make_section(1))?;

        // Create a task under the section
        let task_entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: "Task".to_string(),
            name: "Unrun task".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        };
        let task_id = pg.graph().insert_entity(&task_entity)?;
        let edge = sqlitegraph::GraphEdge {
            id: 0,
            from_id: section_id,
            to_id: task_id,
            edge_type: PlanEdgeKind::DecomposesInto.as_str().to_string(),
            data: serde_json::json!({}),
        };
        pg.graph().insert_edge(&edge)?;

        let gaps = pg.find_gaps(plan_id)?;
        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0], task_id);

        // Now run it — gap should disappear
        let run_data = make_run("agent", "run-004");
        pg.begin_subagent_run(task_id, &run_data)?;
        let gaps2 = pg.find_gaps(plan_id)?;
        assert!(gaps2.is_empty());
        Ok(())
    }

    #[test]
    fn test_full_custody_chain_roundtrip() -> anyhow::Result<()> {
        let mut pg = PlanGraph::open_in_memory()?;

        // 1. Requirement + Plan + Section
        let req = pg.add_requirement("Auth", "OAuth2")?;
        let plan_id = pg.add_plan("Auth Plan", &[req])?;
        let section_id = pg.add_section(plan_id, req, &make_section(1))?;

        // 2. Task under section
        let task_entity = sqlitegraph::GraphEntity {
            id: 0,
            kind: "Task".to_string(),
            name: "Implement login".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        };
        let task_id = pg.graph().insert_entity(&task_entity)?;
        let edge = sqlitegraph::GraphEdge {
            id: 0,
            from_id: section_id,
            to_id: task_id,
            edge_type: PlanEdgeKind::DecomposesInto.as_str().to_string(),
            data: serde_json::json!({}),
        };
        pg.graph().insert_edge(&edge)?;

        // 3. Subagent run
        let run_data = make_run("claude", "run-auth");
        let run_id = pg.begin_subagent_run(task_id, &run_data)?;
        pg.record_log(run_id, &make_log())?;
        pg.record_tool_call(run_id, &make_tool_call())?;
        pg.record_reasoning(run_id, &make_reasoning())?;
        pg.record_deliverable(run_id, &make_deliverable("src/auth.rs", "sha256abc"))?;

        // 4. Forward trace
        let chain = pg.trace_forward(req)?;
        assert_eq!(chain.requirement_id, req);
        assert_eq!(chain.sections.len(), 1);
        assert_eq!(chain.sections[0].section.order, 1);
        assert_eq!(chain.sections[0].tasks.len(), 1);
        assert_eq!(chain.sections[0].tasks[0].task_id, task_id);
        assert_eq!(chain.sections[0].tasks[0].runs.len(), 1);

        let run = &chain.sections[0].tasks[0].runs[0];
        assert_eq!(run.run.agent_name, "claude");
        assert_eq!(run.logs.len(), 1);
        assert_eq!(run.calls.len(), 1);
        assert_eq!(run.reasoning.len(), 1);
        assert_eq!(run.deliverables.len(), 1);
        assert_eq!(run.deliverables[0].file_path, "src/auth.rs");

        // 5. Backward trace from deliverable
        let _deliv_id = run.run_id; // wrong, need actual deliverable id
        let deliv_nodes: Vec<_> = pg.children(
            run_id,
            PlanEdgeKind::Produced.as_str(),
            PlanNodeKind::Deliverable.as_str(),
        )?;
        assert_eq!(deliv_nodes.len(), 1);
        let path = pg.trace_backward(deliv_nodes[0].id)?;
        assert!(path.iter().any(|n| n.kind == "Task"));

        Ok(())
    }
}

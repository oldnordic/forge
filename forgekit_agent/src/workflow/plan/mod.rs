#[cfg(feature = "sqlite")]
mod graph;

#[cfg(feature = "sqlite")]
pub use graph::PlanGraph;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::explorer::DiscoveredKnowledge;
use super::gate::GateResult;
use super::semgrep::SemgrepFinding;

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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SubagentStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    Debug,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanSectionData {
    pub order: usize,
    pub title: String,
    pub description: String,
}

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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogEntryData {
    pub level: LogLevel,
    pub message: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCallData {
    pub tool: String,
    pub args: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReasoningStepData {
    pub thinking: String,
    pub decision: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeliverableData {
    pub file_path: String,
    pub sha256: String,
    pub diff_summary: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustodyNode {
    pub id: i64,
    pub kind: String,
    pub data: serde_json::Value,
    pub timestamp: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustodyChain {
    pub requirement_id: i64,
    pub sections: Vec<CustodySection>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustodySection {
    pub section_id: i64,
    pub section: PlanSectionData,
    pub tasks: Vec<CustodyTask>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustodyTask {
    pub task_id: i64,
    pub runs: Vec<CustodyRun>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustodyRun {
    pub run_id: i64,
    pub run: SubagentRunData,
    pub logs: Vec<LogEntryData>,
    pub calls: Vec<ToolCallData>,
    pub reasoning: Vec<ReasoningStepData>,
    pub deliverables: Vec<DeliverableData>,
}

#[cfg(test)]
mod tests;

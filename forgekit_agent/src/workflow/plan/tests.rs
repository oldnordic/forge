use super::*;
use crate::workflow::explorer::DiscoveredKnowledge;
use crate::workflow::gate::GateResult;
use crate::workflow::semgrep::SemgrepFinding;
use chrono::Utc;

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

    let entity = pg.graph().get_entity(knowledge_id)?;
    assert_eq!(entity.kind, "DiscoveredKnowledge");
    assert_eq!(entity.name, "Sparse inference pattern");

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

    let req = pg.add_requirement("Auth", "OAuth2 login")?;

    let dk = make_discovered_knowledge();

    let plan_id = pg.add_plan("Auth plan", &[req])?;
    pg.link_knowledge(plan_id, &dk)?;

    pg.approve(plan_id, "user")?;

    let task_entity = sqlitegraph::GraphEntity {
        id: 0,
        kind: "Task".to_string(),
        name: "Implement OAuth2".to_string(),
        file_path: None,
        data: serde_json::json!({}),
    };
    let task_id = pg.graph().insert_entity(&task_entity)?;

    let gate_result = make_gate_result(false);
    let result_id = pg.add_gate_result(task_id, &gate_result)?;

    let finding = make_semgrep_finding();
    pg.add_semgrep_finding(result_id, &finding)?;

    let plan_entity = pg.graph().get_entity(plan_id)?;
    assert_eq!(plan_entity.kind, "Plan");

    let has_req = pg.count_outgoing(plan_id, "HAS_REQUIREMENT")?;
    let informed = pg.count_outgoing(plan_id, "INFORMED_BY")?;
    let approved = pg.count_outgoing(plan_id, "APPROVED")?;
    assert_eq!(has_req, 1);
    assert_eq!(informed, 1);
    assert_eq!(approved, 1);

    let knowledge = pg.get_plan_knowledge(plan_id)?;
    assert_eq!(knowledge.len(), 1);

    Ok(())
}

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

    let run_data = make_run("agent", "run-004");
    pg.begin_subagent_run(task_id, &run_data)?;
    let gaps2 = pg.find_gaps(plan_id)?;
    assert!(gaps2.is_empty());
    Ok(())
}

#[test]
fn test_full_custody_chain_roundtrip() -> anyhow::Result<()> {
    let mut pg = PlanGraph::open_in_memory()?;

    let req = pg.add_requirement("Auth", "OAuth2")?;
    let plan_id = pg.add_plan("Auth Plan", &[req])?;
    let section_id = pg.add_section(plan_id, req, &make_section(1))?;

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

    let run_data = make_run("claude", "run-auth");
    let run_id = pg.begin_subagent_run(task_id, &run_data)?;
    pg.record_log(run_id, &make_log())?;
    pg.record_tool_call(run_id, &make_tool_call())?;
    pg.record_reasoning(run_id, &make_reasoning())?;
    pg.record_deliverable(run_id, &make_deliverable("src/auth.rs", "sha256abc"))?;

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

    let _deliv_id = run.run_id;
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

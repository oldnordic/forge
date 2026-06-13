use super::*;

fn fresh_db() -> (tempfile::TempDir, std::path::PathBuf) {
    let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");
    let db_path = temp.path().join("atheneum.db");
    let _ = atheneum::AtheneumGraph::open(&db_path).expect("invariant: DB creation succeeds");
    (temp, db_path)
}

#[test]
fn test_atheneum_tool_definition() {
    let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");
    let tool = AtheneumTool::new(temp.path().join("atheneum.db"), "test-agent");
    let def = tool.definition();
    assert_eq!(def.name, "atheneum");
}

#[tokio::test]
async fn test_atheneum_store_and_query() {
    let (_temp, db_path) = fresh_db();
    let tool = AtheneumTool::new(&db_path, "test-agent");

    let result = tool
        .call(serde_json::json!({
            "command": "store_discovery",
            "target": "my_symbol",
            "discovery_type": "Symbol",
            "metadata": {"file": "src/lib.rs", "line": 42}
        }))
        .await
        .expect("invariant: store succeeds");
    assert!(result.contains("Stored discovery"));

    let result = tool
        .call(serde_json::json!({
            "command": "query_knowledge",
            "target": "my_symbol"
        }))
        .await
        .expect("invariant: query succeeds");
    assert!(result.contains("my_symbol"));
}

#[tokio::test]
async fn test_atheneum_handoff_round_trip() {
    let (_temp, db_path) = fresh_db();
    let tool = AtheneumTool::new(&db_path, "agent-a");

    let result = tool
        .call(serde_json::json!({
            "command": "store_handoff",
            "to_agent": "agent-b",
            "manifest": {"body": "finish the refactor"}
        }))
        .await
        .expect("invariant: store succeeds");
    assert!(result.contains("Stored handoff"));

    let tool_b = AtheneumTool::new(&db_path, "agent-b");

    let result = tool_b
        .call(serde_json::json!({"command": "get_pending_handoff"}))
        .await
        .expect("invariant: get succeeds");
    assert!(result.contains("agent-a"));

    let result = tool_b
        .call(serde_json::json!({
            "command": "claim_handoff",
            "handoff_id": 1
        }))
        .await
        .expect("invariant: claim succeeds");
    assert!(result.contains("Claimed handoff"));
}

#[tokio::test]
async fn test_atheneum_query_empty() {
    let (_temp, db_path) = fresh_db();
    let tool = AtheneumTool::new(&db_path, "test-agent");
    let result = tool
        .call(serde_json::json!({
            "command": "query_knowledge",
            "target": "nonexistent"
        }))
        .await
        .expect("invariant: succeeds");
    assert!(result.contains("No knowledge found"));
}

#[tokio::test]
async fn test_atheneum_unknown_command() {
    let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");
    let tool = AtheneumTool::new(temp.path().join("atheneum.db"), "test-agent");
    let result = tool.call(serde_json::json!({"command": "delete"})).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unknown atheneum command"));
}

#[tokio::test]
async fn test_atheneum_session_round_trip() {
    let (_temp, db_path) = fresh_db();
    let tool = AtheneumTool::new(&db_path, "test-agent");

    let result = tool
        .call(serde_json::json!({
            "command": "record_session",
            "session_id": "sess-001",
            "project": "forge",
            "tool": "forge-agent"
        }))
        .await
        .expect("invariant: record_session succeeds");
    assert!(result.contains("Recorded session"));

    let result = tool
        .call(serde_json::json!({
            "command": "end_session",
            "session_id": "sess-001",
            "exit_status": "ok",
            "prompt_count": 5,
            "tool_call_count": 3
        }))
        .await
        .expect("invariant: end_session succeeds");
    assert!(result.contains("Session ended"));
}

#[tokio::test]
async fn test_atheneum_evidence_tool_call() {
    let (_temp, db_path) = fresh_db();

    let tool = AtheneumTool::new(&db_path, "test-agent");
    tool.call(serde_json::json!({
        "command": "record_session",
        "session_id": "sess-002",
        "project": "forge"
    }))
    .await
    .expect("invariant: session setup");

    let result = tool
        .call(serde_json::json!({
            "command": "record_evidence_tool_call",
            "session_id": "sess-002",
            "tool_name": "graph_query",
            "tool_category": "grounded_query",
            "exit_status": "ok",
            "latency_ms": 42
        }))
        .await
        .expect("invariant: tool call evidence succeeds");
    assert!(result.contains("Recorded tool call evidence"));
}

#[tokio::test]
async fn test_atheneum_planning_task_lifecycle() {
    let (_temp, db_path) = fresh_db();
    let tool = AtheneumTool::new(&db_path, "test-agent");

    let result = tool
        .call(serde_json::json!({
            "command": "create_task",
            "title": "Implement X",
            "description": "Build feature X",
            "project_id": "forge"
        }))
        .await
        .expect("invariant: create_task succeeds");
    assert!(result.contains("Created task"));

    let result = tool
        .call(serde_json::json!({
            "command": "find_task",
            "title": "Implement X",
            "project_id": "forge"
        }))
        .await
        .expect("invariant: find_task succeeds");
    assert!(result.contains("Found task"));

    let result = tool
        .call(serde_json::json!({
            "command": "update_task_status",
            "task_id": 1,
            "status": "IN_PROGRESS"
        }))
        .await
        .expect("invariant: update succeeds");
    assert!(result.contains("updated to"));

    let result = tool
        .call(serde_json::json!({
            "command": "list_tasks",
            "status": "IN_PROGRESS",
            "project_id": "forge"
        }))
        .await
        .expect("invariant: list succeeds");
    assert!(result.contains("Implement X"));

    let result = tool
        .call(serde_json::json!({
            "command": "add_requirement",
            "task_id": 1,
            "statement": "Tests must pass",
            "verification_method": "cargo test"
        }))
        .await
        .expect("invariant: add_requirement succeeds");
    assert!(result.contains("Added requirement"));

    let result = tool
        .call(serde_json::json!({
            "command": "add_blocker",
            "task_id": 1,
            "description": "Waiting on upstream API",
            "blocker_type": "DEPENDENCY"
        }))
        .await
        .expect("invariant: add_blocker succeeds");
    assert!(result.contains("Added blocker"));

    let result = tool
        .call(serde_json::json!({
            "command": "get_task_details",
            "task_id": 1
        }))
        .await
        .expect("invariant: get_task_details succeeds");
    assert!(result.contains("Implement X"));
    assert!(result.contains("Tests must pass"));
    assert!(result.contains("Waiting on upstream API"));
}

#[tokio::test]
async fn test_atheneum_query_events() {
    let (_temp, db_path) = fresh_db();
    let tool = AtheneumTool::new(&db_path, "test-agent");

    tool.call(serde_json::json!({
        "command": "record_session",
        "session_id": "sess-ev",
        "project": "forge"
    }))
    .await
    .expect("invariant: session setup");

    let result = tool
        .call(serde_json::json!({
            "command": "query_events",
            "session_id": "sess-ev"
        }))
        .await
        .expect("invariant: query_events succeeds");
    assert!(result.contains("Events"));
}

#[tokio::test]
async fn test_parse_kanban_status_invalid() {
    let result = handlers::parse_kanban_status("INVALID");
    assert!(result.is_err());
}

#[tokio::test]
async fn test_parse_blocker_type_invalid() {
    let result = handlers::parse_blocker_type("INVALID");
    assert!(result.is_err());
}

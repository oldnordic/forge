use atheneum::AtheneumGraph;

pub(super) fn handle_store_discovery(
    graph: &AtheneumGraph,
    agent_name: &str,
    arguments: &serde_json::Value,
) -> Result<String, String> {
    let target = require_str(arguments, "target")?;
    let discovery_type = require_str(arguments, "discovery_type")?;
    let metadata = arguments
        .get("metadata")
        .cloned()
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

    let id = graph
        .store_discovery(agent_name, &discovery_type, &target, metadata)
        .map_err(|e| format!("store_discovery failed: {e}"))?;

    Ok(format!(
        "Stored discovery {} (type={}, target={})",
        id, discovery_type, target
    ))
}

pub(super) fn handle_query_knowledge(
    graph: &AtheneumGraph,
    arguments: &serde_json::Value,
) -> Result<String, String> {
    let target = require_str(arguments, "target")?;
    let result = graph
        .query_knowledge(&target, None)
        .map_err(|e| format!("query_knowledge failed: {e}"))?;

    format_knowledge_response(&target, &result)
}

pub(super) fn handle_query_knowledge_in_project(
    graph: &AtheneumGraph,
    arguments: &serde_json::Value,
) -> Result<String, String> {
    let target = require_str(arguments, "target")?;
    let project_id = optional_str(arguments, "project_id");
    let result = graph
        .query_knowledge_in_project(&target, project_id, None)
        .map_err(|e| format!("query_knowledge_in_project failed: {e}"))?;

    format_knowledge_response(&target, &result)
}

pub(super) fn handle_store_handoff(
    graph: &AtheneumGraph,
    agent_name: &str,
    arguments: &serde_json::Value,
) -> Result<String, String> {
    let to_agent = require_str(arguments, "to_agent")?;
    let manifest = arguments
        .get("manifest")
        .cloned()
        .unwrap_or(serde_json::json!({"body": "no details"}));

    let id = graph
        .store_handoff(agent_name, &to_agent, manifest)
        .map_err(|e| format!("store_handoff failed: {e}"))?;

    Ok(format!(
        "Stored handoff {} (from={} to={})",
        id, agent_name, to_agent
    ))
}

pub(super) fn handle_get_pending_handoff(
    graph: &AtheneumGraph,
    agent_name: &str,
) -> Result<String, String> {
    let handoff = graph
        .get_pending_handoff(agent_name)
        .map_err(|e| format!("get_pending_handoff failed: {e}"))?;

    match handoff {
        Some(h) => {
            let from = h
                .data
                .get("from_agent")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let manifest = h
                .data
                .get("manifest")
                .cloned()
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            Ok(format!(
                "Pending handoff {} from {}: {:?}",
                h.id, from, manifest
            ))
        }
        None => Ok("No pending handoffs.".to_string()),
    }
}

pub(super) fn handle_claim_handoff(
    graph: &AtheneumGraph,
    arguments: &serde_json::Value,
) -> Result<String, String> {
    let handoff_id = optional_i64(arguments, "handoff_id")
        .ok_or_else(|| "Missing 'handoff_id' parameter".to_string())?;

    graph
        .mark_handoff_claimed(handoff_id)
        .map_err(|e| format!("claim_handoff failed: {e}"))?;

    Ok(format!("Claimed handoff {}", handoff_id))
}

pub(super) fn handle_record_session(
    graph: &AtheneumGraph,
    agent_name: &str,
    arguments: &serde_json::Value,
) -> Result<String, String> {
    let session_id = require_str(arguments, "session_id")?;
    let project = require_str(arguments, "project")?;
    let tool = arguments
        .get("tool")
        .and_then(|v| v.as_str())
        .unwrap_or("forge");
    let trigger = arguments
        .get("trigger")
        .and_then(|v| v.as_str())
        .unwrap_or("manual");

    let params = atheneum::graph::SessionParams {
        session_id,
        agent_name: agent_name.to_string(),
        project,
        tool: tool.to_string(),
        trigger: trigger.to_string(),
        model: optional_str(arguments, "model").map(|s| s.to_string()),
        git_branch: optional_str(arguments, "git_branch").map(|s| s.to_string()),
        git_head: optional_str(arguments, "git_head").map(|s| s.to_string()),
        parent_session_id: optional_str(arguments, "parent_session_id").map(|s| s.to_string()),
        relations: Vec::new(),
    };

    graph
        .record_session(params)
        .map_err(|e| format!("record_session failed: {e}"))?;

    Ok(format!("Recorded session for {}", agent_name))
}

pub(super) fn handle_end_session(
    graph: &AtheneumGraph,
    arguments: &serde_json::Value,
) -> Result<String, String> {
    let session_id = require_str(arguments, "session_id")?;
    let exit_status = require_str(arguments, "exit_status")?;

    let params = atheneum::graph::EndSessionParams {
        session_id,
        exit_status,
        prompt_count: optional_i64(arguments, "prompt_count").unwrap_or(0),
        tool_call_count: optional_i64(arguments, "tool_call_count").unwrap_or(0),
        file_write_count: optional_i64(arguments, "file_write_count").unwrap_or(0),
        commit_count: optional_i64(arguments, "commit_count").unwrap_or(0),
        test_run_count: optional_i64(arguments, "test_run_count").unwrap_or(0),
        total_input_tokens: optional_i64(arguments, "total_input_tokens").unwrap_or(0),
        total_output_tokens: optional_i64(arguments, "total_output_tokens").unwrap_or(0),
        total_cost_usd: optional_f64(arguments, "total_cost_usd").unwrap_or(0.0),
    };

    graph
        .end_session(params)
        .map_err(|e| format!("end_session failed: {e}"))?;

    Ok("Session ended.".to_string())
}

pub(super) fn handle_record_evidence_prompt(
    graph: &AtheneumGraph,
    arguments: &serde_json::Value,
) -> Result<String, String> {
    let session_id = require_str(arguments, "session_id")?;
    let role = require_str(arguments, "role")?;
    let sequence = optional_i64(arguments, "sequence").unwrap_or(0);
    let input_hash = require_str(arguments, "input_hash")?;

    let params = atheneum::graph::PromptParams {
        session_id,
        role,
        sequence,
        content_summary: None,
        source: None,
        input_hash,
        input_tokens: optional_i64(arguments, "input_tokens"),
        output_hash: optional_str(arguments, "output_hash").map(|s| s.to_string()),
        output_tokens: optional_i64(arguments, "output_tokens"),
        latency_ms: optional_i64(arguments, "latency_ms"),
        model: optional_str(arguments, "model").map(|s| s.to_string()),
        cost_usd: optional_f64(arguments, "cost_usd"),
        relations: Vec::new(),
    };

    graph
        .record_evidence_prompt(params)
        .map_err(|e| format!("record_evidence_prompt failed: {e}"))?;

    Ok("Recorded prompt evidence.".to_string())
}

pub(super) fn handle_record_evidence_tool_call(
    graph: &AtheneumGraph,
    arguments: &serde_json::Value,
) -> Result<String, String> {
    let session_id = require_str(arguments, "session_id")?;
    let tool_name = require_str(arguments, "tool_name")?;
    let exit_status = arguments
        .get("exit_status")
        .and_then(|v| v.as_str())
        .unwrap_or("ok");
    let tool_category = arguments
        .get("tool_category")
        .and_then(|v| v.as_str())
        .unwrap_or("other");

    let params = atheneum::graph::ToolCallParams {
        session_id,
        tool_name: tool_name.to_string(),
        sequence: None,
        source: None,
        tool_version: optional_str(arguments, "tool_version").map(|s| s.to_string()),
        input_hash: optional_str(arguments, "input_hash").map(|s| s.to_string()),
        input_summary: optional_str(arguments, "input_summary").map(|s| s.to_string()),
        output_hash: optional_str(arguments, "output_hash").map(|s| s.to_string()),
        output_summary: optional_str(arguments, "output_summary").map(|s| s.to_string()),
        exit_status: exit_status.to_string(),
        latency_ms: optional_i64(arguments, "latency_ms").unwrap_or(0),
        input_tokens_est: optional_i64(arguments, "input_tokens_est"),
        tool_category: tool_category.to_string(),
        relations: Vec::new(),
    };

    let tool_name_out = params.tool_name.clone();
    graph
        .record_evidence_tool_call(params)
        .map_err(|e| format!("record_evidence_tool_call failed: {e}"))?;

    Ok(format!("Recorded tool call evidence for {}", tool_name_out))
}

pub(super) fn handle_record_evidence_file_write(
    graph: &AtheneumGraph,
    arguments: &serde_json::Value,
) -> Result<String, String> {
    let session_id = require_str(arguments, "session_id")?;
    let file_path = require_str(arguments, "file_path")?;

    let params = atheneum::graph::FileWriteParams {
        session_id,
        file_path: file_path.to_string(),
        sequence: None,
        file_id: optional_str(arguments, "file_id").map(|s| s.to_string()),
        before_hash: optional_str(arguments, "before_hash").map(|s| s.to_string()),
        after_hash: optional_str(arguments, "after_hash").map(|s| s.to_string()),
        lines_added: optional_i64(arguments, "lines_added").unwrap_or(0),
        lines_deleted: optional_i64(arguments, "lines_deleted").unwrap_or(0),
        lines_changed: optional_i64(arguments, "lines_changed").unwrap_or(0),
        write_type: arguments
            .get("write_type")
            .and_then(|v| v.as_str())
            .unwrap_or("edit")
            .to_string(),
        relations: Vec::new(),
    };

    let file_path_out = params.file_path.clone();
    graph
        .record_evidence_file_write(params)
        .map_err(|e| format!("record_evidence_file_write failed: {e}"))?;

    Ok(format!(
        "Recorded file write evidence for {}",
        file_path_out
    ))
}

pub(super) fn handle_record_evidence_commit(
    graph: &AtheneumGraph,
    arguments: &serde_json::Value,
) -> Result<String, String> {
    let session_id = require_str(arguments, "session_id")?;
    let commit_sha = require_str(arguments, "commit_sha")?;
    let message = require_str(arguments, "message")?;
    let author = require_str(arguments, "author")?;

    let params = atheneum::graph::CommitParams {
        session_id,
        commit_sha,
        parent_sha: optional_str(arguments, "parent_sha").map(|s| s.to_string()),
        message,
        author,
        files_changed: optional_i64(arguments, "files_changed").unwrap_or(0),
        lines_inserted: optional_i64(arguments, "lines_inserted").unwrap_or(0),
        lines_deleted: optional_i64(arguments, "lines_deleted").unwrap_or(0),
        commit_type: arguments
            .get("commit_type")
            .and_then(|v| v.as_str())
            .unwrap_or("feature")
            .to_string(),
        feature_tag: optional_str(arguments, "feature_tag").map(|s| s.to_string()),
        relations: Vec::new(),
    };

    graph
        .record_evidence_commit(params)
        .map_err(|e| format!("record_evidence_commit failed: {e}"))?;

    Ok("Recorded commit evidence.".to_string())
}

pub(super) fn handle_record_evidence_test_run(
    graph: &AtheneumGraph,
    arguments: &serde_json::Value,
) -> Result<String, String> {
    let session_id = require_str(arguments, "session_id")?;
    let test_name = require_str(arguments, "test_name")?;
    let result_str = require_str(arguments, "result")?;

    let params = atheneum::graph::TestRunParams {
        session_id,
        test_name: test_name.to_string(),
        test_suite: optional_str(arguments, "test_suite").map(|s| s.to_string()),
        test_command: optional_str(arguments, "test_command").map(|s| s.to_string()),
        result: result_str,
        duration_ms: optional_i64(arguments, "duration_ms").unwrap_or(0),
        logs_summary: optional_str(arguments, "logs_summary").map(|s| s.to_string()),
        commit_sha: optional_str(arguments, "commit_sha").map(|s| s.to_string()),
        relations: Vec::new(),
    };

    let test_name_out = params.test_name.clone();
    let test_result_out = params.result.clone();
    graph
        .record_evidence_test_run(params)
        .map_err(|e| format!("record_evidence_test_run failed: {e}"))?;

    Ok(format!(
        "Recorded test run evidence: {} ({})",
        test_name_out, test_result_out
    ))
}

pub(super) fn handle_record_evidence_fix_chain(
    graph: &AtheneumGraph,
    arguments: &serde_json::Value,
) -> Result<String, String> {
    let session_id = require_str(arguments, "session_id")?;
    let bug_commit_sha = require_str(arguments, "bug_commit_sha")?;
    let fix_commit_sha = require_str(arguments, "fix_commit_sha")?;

    let params = atheneum::graph::FixChainParams {
        session_id,
        bug_commit_sha,
        fix_commit_sha,
        fix_type: arguments
            .get("fix_type")
            .and_then(|v| v.as_str())
            .unwrap_or("compile_error")
            .to_string(),
        severity: arguments
            .get("severity")
            .and_then(|v| v.as_str())
            .unwrap_or("medium")
            .to_string(),
        cycles_to_fix: optional_i64(arguments, "cycles_to_fix").unwrap_or(1),
        time_to_fix_ms: optional_i64(arguments, "time_to_fix_ms").unwrap_or(0),
        relations: Vec::new(),
    };

    graph
        .record_evidence_fix_chain(params)
        .map_err(|e| format!("record_evidence_fix_chain failed: {e}"))?;

    Ok("Recorded fix chain evidence.".to_string())
}

pub(super) fn handle_record_evidence_bench_run(
    graph: &AtheneumGraph,
    arguments: &serde_json::Value,
) -> Result<String, String> {
    let session_id = require_str(arguments, "session_id")?;
    let bench_name = require_str(arguments, "bench_name")?;
    let is_regression = arguments
        .get("is_regression")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    graph
        .record_evidence_bench_run(
            session_id,
            bench_name,
            optional_i64(arguments, "mean_ns"),
            optional_i64(arguments, "median_ns"),
            optional_i64(arguments, "p95_ns"),
            is_regression,
        )
        .map_err(|e| format!("record_evidence_bench_run failed: {e}"))?;

    Ok("Recorded bench run evidence.".to_string())
}

pub(super) fn handle_query_events(
    graph: &AtheneumGraph,
    arguments: &serde_json::Value,
) -> Result<String, String> {
    let session_id = optional_str(arguments, "session_id");
    let event_type = optional_str(arguments, "event_type");
    let limit = optional_i64(arguments, "limit").unwrap_or(50) as usize;

    let events = graph
        .query_events(session_id, event_type, limit)
        .map_err(|e| format!("query_events failed: {e}"))?;

    if events.is_empty() {
        return Ok("No events found.".to_string());
    }

    let lines: Vec<String> = events
        .iter()
        .map(|e| {
            let et = e.get("event_type").and_then(|v| v.as_str()).unwrap_or("?");
            let ts = e.get("timestamp").and_then(|v| v.as_str()).unwrap_or("?");
            format!("- {} @ {}", et, ts)
        })
        .collect();
    Ok(format!("Events ({}):\n{}", events.len(), lines.join("\n")))
}

pub(super) fn handle_create_task(
    graph: &AtheneumGraph,
    arguments: &serde_json::Value,
) -> Result<String, String> {
    let title = require_str(arguments, "title")?;
    let description = optional_str(arguments, "description");
    let project_id = optional_str(arguments, "project_id");

    let id = graph
        .create_task(&title, description, project_id)
        .map_err(|e| format!("create_task failed: {e}"))?;

    Ok(format!("Created task {} ({})", id, title))
}

pub(super) fn handle_update_task_status(
    graph: &AtheneumGraph,
    arguments: &serde_json::Value,
) -> Result<String, String> {
    let task_id = optional_i64(arguments, "task_id")
        .ok_or_else(|| "Missing 'task_id' parameter".to_string())?;
    let status_str = require_str(arguments, "status")?;
    let status = parse_kanban_status(&status_str)?;

    graph
        .update_task_status(task_id, status)
        .map_err(|e| format!("update_task_status failed: {e}"))?;

    Ok(format!("Task {} updated to {}", task_id, status_str))
}

pub(super) fn handle_find_task(
    graph: &AtheneumGraph,
    arguments: &serde_json::Value,
) -> Result<String, String> {
    let title = require_str(arguments, "title")?;
    let project_id = optional_str(arguments, "project_id");

    let result = graph
        .find_task_by_title(&title, project_id)
        .map_err(|e| format!("find_task failed: {e}"))?;

    match result {
        Some(id) => Ok(format!("Found task {} ({})", id, title)),
        None => Ok(format!("No task found with title '{}'", title)),
    }
}

pub(super) fn handle_list_tasks(
    graph: &AtheneumGraph,
    arguments: &serde_json::Value,
) -> Result<String, String> {
    let status_str = require_str(arguments, "status")?;
    let status = parse_kanban_status(&status_str)?;
    let project_id = optional_str(arguments, "project_id");

    let tasks = graph
        .list_tasks_by_status(status, project_id)
        .map_err(|e| format!("list_tasks failed: {e}"))?;

    if tasks.is_empty() {
        return Ok(format!("No {} tasks.", status_str));
    }

    let lines: Vec<String> = tasks
        .iter()
        .map(|t| {
            let title = t.data.get("title").and_then(|v| v.as_str()).unwrap_or("?");
            format!("- [{}] {}", t.id, title)
        })
        .collect();
    Ok(format!(
        "{} tasks ({}):\n{}",
        status_str,
        tasks.len(),
        lines.join("\n")
    ))
}

pub(super) fn handle_add_requirement(
    graph: &AtheneumGraph,
    arguments: &serde_json::Value,
) -> Result<String, String> {
    let task_id = optional_i64(arguments, "task_id")
        .ok_or_else(|| "Missing 'task_id' parameter".to_string())?;
    let statement = require_str(arguments, "statement")?;
    let verification_method = optional_str(arguments, "verification_method");

    let id = graph
        .add_requirement(task_id, &statement, verification_method)
        .map_err(|e| format!("add_requirement failed: {e}"))?;

    Ok(format!("Added requirement {} to task {}", id, task_id))
}

pub(super) fn handle_mark_requirement_met(
    graph: &AtheneumGraph,
    arguments: &serde_json::Value,
) -> Result<String, String> {
    let req_id = optional_i64(arguments, "req_id")
        .ok_or_else(|| "Missing 'req_id' parameter".to_string())?;

    graph
        .mark_requirement_met(req_id)
        .map_err(|e| format!("mark_requirement_met failed: {e}"))?;

    Ok(format!("Marked requirement {} as met", req_id))
}

pub(super) fn handle_add_blocker(
    graph: &AtheneumGraph,
    arguments: &serde_json::Value,
) -> Result<String, String> {
    let task_id = optional_i64(arguments, "task_id")
        .ok_or_else(|| "Missing 'task_id' parameter".to_string())?;
    let description = require_str(arguments, "description")?;
    let blocker_type_str = arguments
        .get("blocker_type")
        .and_then(|v| v.as_str())
        .unwrap_or("dependency");
    let blocker_type = parse_blocker_type(blocker_type_str)?;

    let id = graph
        .add_blocker(task_id, &description, blocker_type)
        .map_err(|e| format!("add_blocker failed: {e}"))?;

    Ok(format!("Added blocker {} to task {}", id, task_id))
}

pub(super) fn handle_resolve_blocker(
    graph: &AtheneumGraph,
    arguments: &serde_json::Value,
) -> Result<String, String> {
    let blocker_id = optional_i64(arguments, "blocker_id")
        .ok_or_else(|| "Missing 'blocker_id' parameter".to_string())?;

    graph
        .resolve_blocker(blocker_id)
        .map_err(|e| format!("resolve_blocker failed: {e}"))?;

    Ok(format!("Resolved blocker {}", blocker_id))
}

pub(super) fn handle_get_task_details(
    graph: &AtheneumGraph,
    arguments: &serde_json::Value,
) -> Result<String, String> {
    let task_id = optional_i64(arguments, "task_id")
        .ok_or_else(|| "Missing 'task_id' parameter".to_string())?;

    let detail = graph
        .get_task_with_details(task_id)
        .map_err(|e| format!("get_task_details failed: {e}"))?;

    let title = detail
        .task
        .data
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let status = detail
        .task
        .data
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("?");

    let mut lines = vec![format!("Task {} [{}]: {}", detail.task.id, status, title)];

    if !detail.requirements.is_empty() {
        lines.push(format!("  Requirements ({}):", detail.requirements.len()));
        for r in &detail.requirements {
            let stmt = r
                .data
                .get("statement")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let st = r.data.get("status").and_then(|v| v.as_str()).unwrap_or("?");
            lines.push(format!("    - [{}] {} ({})", r.id, stmt, st));
        }
    }

    if !detail.blockers.is_empty() {
        lines.push(format!("  Blockers ({}):", detail.blockers.len()));
        for b in &detail.blockers {
            let desc = b
                .data
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let resolved = if b.data.get("resolved_at").is_some() {
                "resolved"
            } else {
                "active"
            };
            lines.push(format!("    - [{}] {} ({})", b.id, desc, resolved));
        }
    }

    Ok(lines.join("\n"))
}

fn require_str(args: &serde_json::Value, key: &str) -> Result<String, String> {
    args[key]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| format!("Missing '{}' parameter", key))
}

fn optional_str<'a>(args: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    args[key].as_str()
}

fn optional_i64(args: &serde_json::Value, key: &str) -> Option<i64> {
    args[key].as_i64()
}

fn optional_f64(args: &serde_json::Value, key: &str) -> Option<f64> {
    args[key].as_f64()
}

fn format_knowledge_response(target: &str, result: &serde_json::Value) -> Result<String, String> {
    let discoveries = result
        .get("discoveries")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if discoveries.is_empty() {
        return Ok(format!("No knowledge found for '{}'.", target));
    }

    let lines: Vec<String> = discoveries
        .iter()
        .map(|d| {
            let dtype = d
                .get("discovery_type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let agent = d.get("agent").and_then(|v| v.as_str()).unwrap_or("unknown");
            format!("- {} by {}: {}", dtype, agent, d)
        })
        .collect();
    Ok(format!(
        "Knowledge for '{}' ({} entries):\n{}",
        target,
        discoveries.len(),
        lines.join("\n")
    ))
}

pub(super) fn parse_kanban_status(s: &str) -> Result<atheneum::KanbanStatus, String> {
    match s.to_ascii_uppercase().as_str() {
        "TODO" => Ok(atheneum::KanbanStatus::Todo),
        "IN_PROGRESS" | "IN-PROGRESS" | "INPROGRESS" => Ok(atheneum::KanbanStatus::InProgress),
        "DONE" => Ok(atheneum::KanbanStatus::Done),
        "BLOCKED" => Ok(atheneum::KanbanStatus::Blocked),
        _ => Err(format!(
            "Invalid status '{}'. Must be TODO, IN_PROGRESS, DONE, or BLOCKED",
            s
        )),
    }
}

pub(super) fn parse_blocker_type(s: &str) -> Result<atheneum::graph::BlockerType, String> {
    match s.to_ascii_uppercase().as_str() {
        "DEPENDENCY" => Ok(atheneum::graph::BlockerType::Dependency),
        "BUG" => Ok(atheneum::graph::BlockerType::Bug),
        "INFO_GAP" => Ok(atheneum::graph::BlockerType::InfoGap),
        _ => Err(format!(
            "Invalid blocker_type '{}'. Must be DEPENDENCY, BUG, or INFO_GAP",
            s
        )),
    }
}

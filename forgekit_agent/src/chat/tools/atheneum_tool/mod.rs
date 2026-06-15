mod handlers;
#[cfg(test)]
mod tests;

use crate::chat::tools::registry::AsyncTool;
use crate::chat::tools::types::ToolDef;
use async_trait::async_trait;
use std::path::PathBuf;

pub struct AtheneumTool {
    db_path: PathBuf,
    agent_name: String,
}

impl AtheneumTool {
    pub fn new(db_path: impl Into<PathBuf>, agent_name: impl Into<String>) -> Self {
        AtheneumTool {
            db_path: db_path.into(),
            agent_name: agent_name.into(),
        }
    }

    fn open_graph(&self) -> Result<atheneum::AtheneumGraph, String> {
        atheneum::AtheneumGraph::open(&self.db_path).map_err(|e| {
            format!(
                "Failed to open atheneum DB at {}: {e}",
                self.db_path.display()
            )
        })
    }
}

const COMMAND_LIST: &str = "\
store_discovery, query_knowledge, query_knowledge_in_project, \
store_handoff, get_pending_handoff, claim_handoff, \
record_session, end_session, \
record_evidence_prompt, record_evidence_tool_call, record_evidence_file_write, \
record_evidence_commit, record_evidence_test_run, record_evidence_fix_chain, \
record_evidence_bench_run, query_events, \
create_task, update_task_status, find_task, list_tasks, \
add_requirement, mark_requirement_met, add_blocker, resolve_blocker, \
get_task_details";

#[async_trait]
impl AsyncTool for AtheneumTool {
    async fn call(&self, arguments: serde_json::Value) -> Result<String, String> {
        let command = arguments
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'command' parameter".to_string())?;
        let graph = self.open_graph()?;

        match command {
            "store_discovery" => {
                handlers::handle_store_discovery(&graph, &self.agent_name, &arguments)
            }
            "query_knowledge" => handlers::handle_query_knowledge(&graph, &arguments),
            "query_knowledge_in_project" => {
                handlers::handle_query_knowledge_in_project(&graph, &arguments)
            }
            "store_handoff" => handlers::handle_store_handoff(&graph, &self.agent_name, &arguments),
            "get_pending_handoff" => handlers::handle_get_pending_handoff(&graph, &self.agent_name),
            "claim_handoff" => handlers::handle_claim_handoff(&graph, &arguments),
            "record_session" => {
                handlers::handle_record_session(&graph, &self.agent_name, &arguments)
            }
            "end_session" => handlers::handle_end_session(&graph, &arguments),
            "record_evidence_prompt" => handlers::handle_record_evidence_prompt(&graph, &arguments),
            "record_evidence_tool_call" => {
                handlers::handle_record_evidence_tool_call(&graph, &arguments)
            }
            "record_evidence_file_write" => {
                handlers::handle_record_evidence_file_write(&graph, &arguments)
            }
            "record_evidence_commit" => handlers::handle_record_evidence_commit(&graph, &arguments),
            "record_evidence_test_run" => {
                handlers::handle_record_evidence_test_run(&graph, &arguments)
            }
            "record_evidence_fix_chain" => {
                handlers::handle_record_evidence_fix_chain(&graph, &arguments)
            }
            "record_evidence_bench_run" => {
                handlers::handle_record_evidence_bench_run(&graph, &arguments)
            }
            "query_events" => handlers::handle_query_events(&graph, &arguments),
            "create_task" => handlers::handle_create_task(&graph, &arguments),
            "update_task_status" => handlers::handle_update_task_status(&graph, &arguments),
            "find_task" => handlers::handle_find_task(&graph, &arguments),
            "list_tasks" => handlers::handle_list_tasks(&graph, &arguments),
            "add_requirement" => handlers::handle_add_requirement(&graph, &arguments),
            "mark_requirement_met" => handlers::handle_mark_requirement_met(&graph, &arguments),
            "add_blocker" => handlers::handle_add_blocker(&graph, &arguments),
            "resolve_blocker" => handlers::handle_resolve_blocker(&graph, &arguments),
            "get_task_details" => handlers::handle_get_task_details(&graph, &arguments),
            _ => Err(format!(
                "Unknown atheneum command: '{}'. Available: {}",
                command, COMMAND_LIST
            )),
        }
    }

    fn definition(&self) -> ToolDef {
        ToolDef::new(
            "atheneum",
            "Knowledge graph, evidence tracking, and task planning. \
             Discovery: store_discovery, query_knowledge, query_knowledge_in_project. \
             Handoff: store_handoff, get_pending_handoff, claim_handoff. \
             Evidence: record_session, end_session, record_evidence_prompt, record_evidence_tool_call, \
             record_evidence_file_write, record_evidence_commit, record_evidence_test_run, \
             record_evidence_fix_chain, record_evidence_bench_run, query_events. \
             Planning: create_task, update_task_status, find_task, list_tasks, \
             add_requirement, mark_requirement_met, add_blocker, resolve_blocker, get_task_details.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The atheneum command to execute"
                    },
                    "target": {
                        "type": "string",
                        "description": "Target symbol/concept (store_discovery, query_knowledge)"
                    },
                    "discovery_type": {
                        "type": "string",
                        "description": "Type of discovery (store_discovery)"
                    },
                    "metadata": {
                        "type": "object",
                        "description": "Arbitrary metadata (store_discovery)"
                    },
                    "project_id": {
                        "type": "string",
                        "description": "Project scope (query_knowledge_in_project, planning commands)"
                    },
                    "to_agent": {
                        "type": "string",
                        "description": "Recipient agent name (store_handoff)"
                    },
                    "manifest": {
                        "type": "object",
                        "description": "Handoff manifest data (store_handoff)"
                    },
                    "handoff_id": {
                        "type": "integer",
                        "description": "Handoff ID to claim (claim_handoff)"
                    },
                    "session_id": {
                        "type": "string",
                        "description": "Session identifier (evidence commands)"
                    },
                    "project": {
                        "type": "string",
                        "description": "Project name (record_session)"
                    },
                    "title": {
                        "type": "string",
                        "description": "Task title (create_task, find_task)"
                    },
                    "description": {
                        "type": "string",
                        "description": "Description (create_task, add_blocker)"
                    },
                    "task_id": {
                        "type": "integer",
                        "description": "Task ID (update_task_status, add_requirement, add_blocker, get_task_details)"
                    },
                    "status": {
                        "type": "string",
                        "enum": ["TODO", "IN_PROGRESS", "DONE", "BLOCKED"],
                        "description": "Task status (update_task_status, list_tasks)"
                    },
                    "statement": {
                        "type": "string",
                        "description": "Requirement statement (add_requirement)"
                    },
                    "verification_method": {
                        "type": "string",
                        "description": "How to verify (add_requirement)"
                    },
                    "req_id": {
                        "type": "integer",
                        "description": "Requirement ID (mark_requirement_met)"
                    },
                    "blocker_id": {
                        "type": "integer",
                        "description": "Blocker ID (resolve_blocker)"
                    },
                    "blocker_type": {
                        "type": "string",
                        "enum": ["DEPENDENCY", "BUG", "INFO_GAP"],
                        "description": "Blocker type (add_blocker)"
                    }
                },
                "required": ["command"]
            }),
        )
    }
}

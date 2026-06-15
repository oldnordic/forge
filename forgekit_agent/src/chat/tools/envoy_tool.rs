use crate::chat::tools::registry::AsyncTool;
use crate::chat::tools::types::ToolDef;
use crate::envoy::EnvoyClient;
use async_trait::async_trait;
use std::sync::Arc;

const COMMAND_LIST: &str = "\
send_message, poll_messages, \
store_discovery, query_discoveries, query_knowledge, \
store_handoff, get_pending_handoff, claim_handoff, \
record_evidence_prompt, record_evidence_tool_call, record_evidence_file_write, \
record_evidence_commit, record_evidence_test_run, record_evidence_fix_chain, \
record_evidence_bench_run, query_events";

pub struct EnvoyTool {
    client: Arc<EnvoyClient>,
}

impl EnvoyTool {
    pub fn new(client: Arc<EnvoyClient>) -> Self {
        EnvoyTool { client }
    }

    fn str_field(args: &serde_json::Value, key: &str) -> Option<String> {
        args.get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    fn u64_field(args: &serde_json::Value, key: &str) -> Option<u64> {
        args.get(key).and_then(|v| v.as_u64())
    }

    fn i64_field(args: &serde_json::Value, key: &str) -> Option<i64> {
        args.get(key).and_then(|v| v.as_i64())
    }

    fn f64_field(args: &serde_json::Value, key: &str) -> Option<f64> {
        args.get(key).and_then(|v| v.as_f64())
    }

    fn bool_field(args: &serde_json::Value, key: &str) -> Option<bool> {
        args.get(key).and_then(|v| v.as_bool())
    }
}

#[async_trait]
impl AsyncTool for EnvoyTool {
    async fn call(&self, arguments: serde_json::Value) -> Result<String, String> {
        let command = arguments["command"]
            .as_str()
            .ok_or_else(|| "Missing 'command' parameter".to_string())?;

        match command {
            // ── Messaging ─────────────────────────────────────────────────────
            "send_message" => {
                let to = arguments["to"]
                    .as_str()
                    .ok_or_else(|| "Missing 'to' parameter".to_string())?;
                let content = arguments
                    .get("content")
                    .cloned()
                    .unwrap_or(serde_json::Value::String(String::new()));
                self.client
                    .send_message(&self.client.config.agent_name, to, content)
                    .await?;
                Ok(format!("Message sent to {}", to))
            }

            "poll_messages" => {
                let since = arguments["since"].as_i64();
                let messages = self.client.poll_messages(since).await?;
                if messages.is_empty() {
                    return Ok("No new messages.".to_string());
                }
                let lines: Vec<String> = messages
                    .iter()
                    .map(|m| {
                        serde_json::to_string(m).unwrap_or_else(|_| "<unparseable>".to_string())
                    })
                    .collect();
                Ok(format!(
                    "Messages ({}):\n{}",
                    messages.len(),
                    lines.join("\n")
                ))
            }

            // ── Discovery ─────────────────────────────────────────────────────
            "store_discovery" => {
                let discovery_type = arguments["discovery_type"]
                    .as_str()
                    .ok_or_else(|| "Missing 'discovery_type' parameter".to_string())?;
                let target = arguments["target"]
                    .as_str()
                    .ok_or_else(|| "Missing 'target' parameter".to_string())?;
                let metadata = arguments
                    .get("metadata")
                    .cloned()
                    .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                let id = self
                    .client
                    .store_discovery(discovery_type, target, metadata)
                    .await?;
                Ok(format!("Stored discovery {}", id))
            }

            "query_discoveries" => {
                let target = arguments["target"]
                    .as_str()
                    .ok_or_else(|| "Missing 'target' parameter".to_string())?;
                let results = self.client.query_discoveries(target).await?;
                if results.is_empty() {
                    return Ok(format!("No discoveries found for '{}'.", target));
                }
                let lines: Vec<String> = results
                    .iter()
                    .map(|d| {
                        serde_json::to_string(d).unwrap_or_else(|_| "<unparseable>".to_string())
                    })
                    .collect();
                Ok(format!(
                    "Discoveries for '{}' ({}):\n{}",
                    target,
                    results.len(),
                    lines.join("\n")
                ))
            }

            // ── Knowledge ─────────────────────────────────────────────────────
            "query_knowledge" => {
                let target = arguments["target"]
                    .as_str()
                    .ok_or_else(|| "Missing 'target' parameter".to_string())?;
                let results = self.client.query_knowledge(target).await?;
                if results.is_empty() {
                    return Ok(format!("No knowledge found for '{}'.", target));
                }
                let lines: Vec<String> = results
                    .iter()
                    .map(|d| {
                        serde_json::to_string(d).unwrap_or_else(|_| "<unparseable>".to_string())
                    })
                    .collect();
                Ok(format!(
                    "Knowledge for '{}' ({}):\n{}",
                    target,
                    results.len(),
                    lines.join("\n")
                ))
            }

            // ── Handoff ───────────────────────────────────────────────────────
            "store_handoff" => {
                let to_agent = arguments["to_agent"]
                    .as_str()
                    .ok_or_else(|| "Missing 'to_agent' parameter".to_string())?;
                let manifest = arguments
                    .get("manifest")
                    .cloned()
                    .unwrap_or(serde_json::json!({"body": "no details"}));
                let id = self.client.store_handoff(to_agent, manifest).await?;
                Ok(format!("Stored handoff {}", id))
            }

            "get_pending_handoff" => {
                let handoff = self.client.get_pending_handoff().await?;
                match handoff {
                    Some(h) => Ok(format!(
                        "Pending handoff: {}",
                        serde_json::to_string(&h).unwrap_or_else(|_| "<unparseable>".to_string())
                    )),
                    None => Ok("No pending handoffs.".to_string()),
                }
            }

            "claim_handoff" => {
                let handoff_id = arguments["handoff_id"]
                    .as_i64()
                    .ok_or_else(|| "Missing 'handoff_id' parameter".to_string())?;
                self.client.claim_handoff(handoff_id).await?;
                Ok(format!("Claimed handoff {}", handoff_id))
            }

            // ── Evidence: Prompt ──────────────────────────────────────────────
            "record_evidence_prompt" => {
                let session_id = arguments["session_id"]
                    .as_str()
                    .ok_or_else(|| "Missing 'session_id' parameter".to_string())?;
                let record = crate::evidence::PromptRecord {
                    role: Self::str_field(&arguments, "role").unwrap_or_default(),
                    sequence: Self::u64_field(&arguments, "sequence").unwrap_or(0) as u32,
                    input_hash: Self::str_field(&arguments, "input_hash").unwrap_or_default(),
                    input_tokens: Self::u64_field(&arguments, "input_tokens"),
                    output_hash: Self::str_field(&arguments, "output_hash"),
                    output_tokens: Self::u64_field(&arguments, "output_tokens"),
                    latency_ms: Self::u64_field(&arguments, "latency_ms"),
                    model: Self::str_field(&arguments, "model"),
                    cost_usd: Self::f64_field(&arguments, "cost_usd"),
                };
                self.client.forge_prompt(session_id, &record).await?;
                Ok("Recorded prompt evidence.".to_string())
            }

            // ── Evidence: Tool Call ────────────────────────────────────────────
            "record_evidence_tool_call" => {
                let session_id = arguments["session_id"]
                    .as_str()
                    .ok_or_else(|| "Missing 'session_id' parameter".to_string())?;
                let tool_category_str = Self::str_field(&arguments, "tool_category")
                    .unwrap_or_else(|| "other".to_string());
                let tool_category: crate::evidence::types::ToolCategory =
                    serde_json::from_value(serde_json::Value::String(tool_category_str))
                        .unwrap_or_default();
                let record = crate::evidence::ToolCallEvidence {
                    tool_name: Self::str_field(&arguments, "tool_name").unwrap_or_default(),
                    tool_version: Self::str_field(&arguments, "tool_version"),
                    input_hash: Self::str_field(&arguments, "input_hash").unwrap_or_default(),
                    input_summary: Self::str_field(&arguments, "input_summary").unwrap_or_default(),
                    output_hash: Self::str_field(&arguments, "output_hash"),
                    output_summary: Self::str_field(&arguments, "output_summary"),
                    exit_status: Self::str_field(&arguments, "exit_status")
                        .unwrap_or_else(|| "ok".to_string()),
                    latency_ms: Self::u64_field(&arguments, "latency_ms").unwrap_or(0),
                    input_tokens_est: Self::u64_field(&arguments, "input_tokens_est"),
                    tool_category,
                };
                let tool_name = record.tool_name.clone();
                self.client.forge_tool_call(session_id, &record).await?;
                Ok(format!("Recorded tool call evidence for {}", tool_name))
            }

            // ── Evidence: File Write ───────────────────────────────────────────
            "record_evidence_file_write" => {
                let session_id = arguments["session_id"]
                    .as_str()
                    .ok_or_else(|| "Missing 'session_id' parameter".to_string())?;
                let record = crate::evidence::FileWriteRecord {
                    file_path: Self::str_field(&arguments, "file_path").unwrap_or_default(),
                    file_id: Self::str_field(&arguments, "file_id").unwrap_or_default(),
                    before_hash: Self::str_field(&arguments, "before_hash"),
                    after_hash: Self::str_field(&arguments, "after_hash").unwrap_or_default(),
                    lines_added: Self::u64_field(&arguments, "lines_added").unwrap_or(0),
                    lines_deleted: Self::u64_field(&arguments, "lines_deleted").unwrap_or(0),
                    lines_changed: Self::u64_field(&arguments, "lines_changed").unwrap_or(0),
                    write_type: Self::str_field(&arguments, "write_type")
                        .unwrap_or_else(|| "edit".to_string()),
                };
                let file_path = record.file_path.clone();
                self.client.forge_file_write(session_id, &record).await?;
                Ok(format!("Recorded file write evidence for {}", file_path))
            }

            // ── Evidence: Commit ──────────────────────────────────────────────
            "record_evidence_commit" => {
                let session_id = arguments["session_id"]
                    .as_str()
                    .ok_or_else(|| "Missing 'session_id' parameter".to_string())?;
                let commit_msg = Self::str_field(&arguments, "message").unwrap_or_default();
                let record = crate::evidence::CommitRecord {
                    commit_sha: Self::str_field(&arguments, "commit_sha").unwrap_or_default(),
                    parent_sha: Self::str_field(&arguments, "parent_sha"),
                    message: commit_msg.clone(),
                    author: Self::str_field(&arguments, "author").unwrap_or_default(),
                    files_changed: Self::u64_field(&arguments, "files_changed").unwrap_or(0),
                    lines_inserted: Self::u64_field(&arguments, "lines_inserted").unwrap_or(0),
                    lines_deleted: Self::u64_field(&arguments, "lines_deleted").unwrap_or(0),
                    commit_type: crate::evidence::types::CommitType::classify(&commit_msg),
                    feature_tag: Self::str_field(&arguments, "feature_tag"),
                };
                self.client.forge_commit(session_id, &record).await?;
                Ok("Recorded commit evidence.".to_string())
            }

            // ── Evidence: Test Run ────────────────────────────────────────────
            "record_evidence_test_run" => {
                let session_id = arguments["session_id"]
                    .as_str()
                    .ok_or_else(|| "Missing 'session_id' parameter".to_string())?;
                let record = crate::evidence::TestRunRecord {
                    test_name: Self::str_field(&arguments, "test_name").unwrap_or_default(),
                    test_suite: Self::str_field(&arguments, "test_suite"),
                    test_command: Self::str_field(&arguments, "test_command").unwrap_or_default(),
                    result: Self::str_field(&arguments, "result")
                        .unwrap_or_else(|| "ok".to_string()),
                    duration_ms: Self::u64_field(&arguments, "duration_ms").unwrap_or(0),
                    logs_summary: Self::str_field(&arguments, "logs_summary"),
                };
                let test_name = record.test_name.clone();
                let test_result = record.result.clone();
                self.client.forge_test_run(session_id, &record).await?;
                Ok(format!(
                    "Recorded test run evidence: {} ({})",
                    test_name, test_result
                ))
            }

            // ── Evidence: Fix Chain ───────────────────────────────────────────
            "record_evidence_fix_chain" => {
                let session_id = arguments["session_id"]
                    .as_str()
                    .ok_or_else(|| "Missing 'session_id' parameter".to_string())?;
                let fix_type_str = Self::str_field(&arguments, "fix_type")
                    .unwrap_or_else(|| "compile_error".to_string());
                let severity_str =
                    Self::str_field(&arguments, "severity").unwrap_or_else(|| "medium".to_string());
                let record = crate::evidence::FixChainRecord {
                    bug_commit_sha: Self::str_field(&arguments, "bug_commit_sha")
                        .unwrap_or_default(),
                    fix_commit_sha: Self::str_field(&arguments, "fix_commit_sha")
                        .unwrap_or_default(),
                    fix_type: serde_json::from_value(serde_json::Value::String(fix_type_str))
                        .unwrap_or_default(),
                    severity: serde_json::from_value(serde_json::Value::String(severity_str))
                        .unwrap_or_default(),
                    cycles_to_fix: Self::u64_field(&arguments, "cycles_to_fix").unwrap_or(1) as u32,
                    time_to_fix_ms: Self::u64_field(&arguments, "time_to_fix_ms").unwrap_or(0),
                };
                self.client.forge_fix_chain(session_id, &record).await?;
                Ok("Recorded fix chain evidence.".to_string())
            }

            // ── Evidence: Bench Run ───────────────────────────────────────────
            "record_evidence_bench_run" => {
                let session_id = arguments["session_id"]
                    .as_str()
                    .ok_or_else(|| "Missing 'session_id' parameter".to_string())?;
                let bench_name = arguments["bench_name"]
                    .as_str()
                    .ok_or_else(|| "Missing 'bench_name' parameter".to_string())?;
                let is_regression = Self::bool_field(&arguments, "is_regression").unwrap_or(false);
                self.client
                    .forge_bench_run(
                        session_id,
                        bench_name,
                        Self::i64_field(&arguments, "mean_ns"),
                        Self::i64_field(&arguments, "median_ns"),
                        Self::i64_field(&arguments, "p95_ns"),
                        is_regression,
                    )
                    .await?;
                Ok("Recorded bench run evidence.".to_string())
            }

            // ── Evidence: Query Events ────────────────────────────────────────
            "query_events" => {
                let session_id = arguments["session_id"].as_str();
                let event_type = arguments["event_type"].as_str();
                let limit = arguments["limit"].as_i64();
                let events = self
                    .client
                    .query_events(session_id, event_type, limit)
                    .await?;
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

            _ => Err(format!(
                "Unknown envoy command: '{}'. Available: {}",
                command, COMMAND_LIST
            )),
        }
    }

    fn definition(&self) -> ToolDef {
        ToolDef::new(
            "envoy",
            "Multi-agent coordination and evidence tracking via envoy. \
             Messaging: send_message, poll_messages. \
             Discovery: store_discovery, query_discoveries, query_knowledge. \
             Handoff: store_handoff, get_pending_handoff, claim_handoff. \
             Evidence: record_evidence_prompt, record_evidence_tool_call, record_evidence_file_write, \
             record_evidence_commit, record_evidence_test_run, record_evidence_fix_chain, \
             record_evidence_bench_run, query_events.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The envoy command to execute"
                    },
                    "to": {
                        "type": "string",
                        "description": "Recipient agent name (send_message)"
                    },
                    "content": {
                        "description": "Message content (send_message)"
                    },
                    "since": {
                        "type": "integer",
                        "description": "Timestamp to filter messages from (poll_messages)"
                    },
                    "discovery_type": {
                        "type": "string",
                        "description": "Type of discovery (store_discovery)"
                    },
                    "target": {
                        "type": "string",
                        "description": "Target symbol/concept (store_discovery, query_discoveries, query_knowledge)"
                    },
                    "metadata": {
                        "type": "object",
                        "description": "Arbitrary metadata (store_discovery)"
                    },
                    "to_agent": {
                        "type": "string",
                        "description": "Recipient agent name (store_handoff)"
                    },
                    "manifest": {
                        "type": "object",
                        "description": "Handoff manifest (store_handoff)"
                    },
                    "handoff_id": {
                        "type": "integer",
                        "description": "Handoff ID to claim (claim_handoff)"
                    },
                    "session_id": {
                        "type": "string",
                        "description": "Session identifier (evidence commands)"
                    }
                },
                "required": ["command"]
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envoy::EnvoyConfig;

    fn test_client() -> Arc<EnvoyClient> {
        Arc::new(EnvoyClient::new(EnvoyConfig {
            url: "http://localhost:19999".to_string(),
            agent_name: "test-forge".to_string(),
        }))
    }

    #[test]
    fn test_envoy_tool_definition() {
        let tool = EnvoyTool::new(test_client());
        let def = tool.definition();
        assert_eq!(def.name, "envoy");
    }
}

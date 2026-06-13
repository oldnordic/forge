use super::types::HookConfig;
use super::types::{HookEvent, HookSpec};
use std::process::Stdio;
use tokio::process::Command;

#[derive(Clone, Debug)]
pub struct HookContext {
    pub tool_name: Option<String>,
    pub command: Option<String>,
    pub iteration: Option<usize>,
    pub answer: Option<String>,
}

impl HookContext {
    pub fn for_tool_call(tool_name: &str, command: Option<&str>) -> Self {
        HookContext {
            tool_name: Some(tool_name.to_string()),
            command: command.map(|s| s.to_string()),
            iteration: None,
            answer: None,
        }
    }

    pub fn for_session_start() -> Self {
        HookContext {
            tool_name: None,
            command: None,
            iteration: None,
            answer: None,
        }
    }

    pub fn for_stop(answer: Option<&str>) -> Self {
        HookContext {
            tool_name: None,
            command: None,
            iteration: None,
            answer: answer.map(|s| s.to_string()),
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        let mut obj = serde_json::Map::new();
        if let Some(ref name) = self.tool_name {
            obj.insert(
                "tool_name".to_string(),
                serde_json::Value::String(name.clone()),
            );
        }
        if let Some(ref cmd) = self.command {
            obj.insert(
                "command".to_string(),
                serde_json::Value::String(cmd.clone()),
            );
        }
        if let Some(iter) = self.iteration {
            obj.insert(
                "iteration".to_string(),
                serde_json::Value::Number(iter.into()),
            );
        }
        if let Some(ref answer) = self.answer {
            obj.insert(
                "answer".to_string(),
                serde_json::Value::String(answer.clone()),
            );
        }
        serde_json::Value::Object(obj)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HookResult {
    Allowed,
    Blocked(String),
}

pub struct HookRunner {
    config: HookConfig,
}

impl HookRunner {
    pub fn new(config: HookConfig) -> Self {
        HookRunner { config }
    }

    pub fn empty() -> Self {
        HookRunner {
            config: HookConfig::empty(),
        }
    }

    pub fn config(&self) -> &HookConfig {
        &self.config
    }

    pub async fn run_hooks(&self, event: &HookEvent, context: &HookContext) -> Vec<HookResult> {
        let groups = self.config.groups_for(event);
        let mut results = Vec::new();

        for group in groups {
            let matcher_matches = match (&group.matcher, &context.tool_name) {
                (Some(matcher), Some(tool_name)) => match regex::Regex::new(matcher) {
                    Ok(re) => re.is_match(tool_name),
                    Err(_) => false,
                },
                (Some(_), None) => false,
                (None, _) => true,
            };

            if !matcher_matches {
                continue;
            }

            for spec in &group.hooks {
                let result = self.run_single_hook(spec, context).await;
                results.push(result);
            }
        }

        results
    }

    pub async fn check_allowed(&self, event: &HookEvent, context: &HookContext) -> bool {
        let results = self.run_hooks(event, context).await;
        !results.iter().any(|r| matches!(r, HookResult::Blocked(_)))
    }

    async fn run_single_hook(&self, spec: &HookSpec, context: &HookContext) -> HookResult {
        if spec.hook_type != "command" {
            return HookResult::Allowed;
        }

        let timeout_secs = spec.timeout.unwrap_or(30);
        let input = context.to_json().to_string();

        let spawn_result = Command::new("sh")
            .arg("-c")
            .arg(&spec.command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();

        let mut child = match spawn_result {
            Ok(c) => c,
            Err(e) => return HookResult::Blocked(format!("Failed to spawn hook: {e}")),
        };

        if let Some(ref mut stdin) = child.stdin {
            use tokio::io::AsyncWriteExt;
            let _ = stdin.write_all(input.as_bytes()).await;
        }

        let timeout_result = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            child.wait_with_output(),
        )
        .await;

        match timeout_result {
            Ok(Ok(output)) => {
                if output.status.code() == Some(2) {
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    HookResult::Blocked(if stderr.is_empty() {
                        "Hook blocked execution".to_string()
                    } else {
                        stderr
                    })
                } else {
                    HookResult::Allowed
                }
            }
            Ok(Err(e)) => HookResult::Blocked(format!("Hook failed: {e}")),
            Err(_) => HookResult::Blocked(format!("Hook timed out after {timeout_secs}s")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::hooks::types::{HookGroup, HookSpec};

    #[tokio::test]
    async fn test_empty_runner_always_allows() {
        let runner = HookRunner::empty();
        let ctx = HookContext::for_session_start();
        assert!(runner.check_allowed(&HookEvent::SessionStart, &ctx).await);
        assert!(runner.check_allowed(&HookEvent::PreToolUse, &ctx).await);
    }

    #[tokio::test]
    async fn test_hook_exit_zero_allows() {
        let mut config = HookConfig::empty();
        config.add_group(
            HookEvent::PreToolUse,
            HookGroup {
                matcher: None,
                hooks: vec![HookSpec {
                    hook_type: "command".to_string(),
                    command: "exit 0".to_string(),
                    timeout: Some(5),
                    status_message: None,
                }],
            },
        );
        let runner = HookRunner::new(config);
        let ctx = HookContext::for_tool_call("file_read", None);
        assert!(runner.check_allowed(&HookEvent::PreToolUse, &ctx).await);
    }

    #[tokio::test]
    async fn test_hook_exit_two_blocks() {
        let mut config = HookConfig::empty();
        config.add_group(
            HookEvent::PreToolUse,
            HookGroup {
                matcher: None,
                hooks: vec![HookSpec {
                    hook_type: "command".to_string(),
                    command: "echo 'blocked by policy' >&2; exit 2".to_string(),
                    timeout: Some(5),
                    status_message: None,
                }],
            },
        );
        let runner = HookRunner::new(config);
        let ctx = HookContext::for_tool_call("file_read", None);
        assert!(!runner.check_allowed(&HookEvent::PreToolUse, &ctx).await);
    }

    #[tokio::test]
    async fn test_hook_matcher_filters() {
        let mut config = HookConfig::empty();
        config.add_group(
            HookEvent::PreToolUse,
            HookGroup {
                matcher: Some("Bash".to_string()),
                hooks: vec![HookSpec {
                    hook_type: "command".to_string(),
                    command: "exit 2".to_string(),
                    timeout: Some(5),
                    status_message: None,
                }],
            },
        );
        let runner = HookRunner::new(config);

        let ctx_bash = HookContext::for_tool_call("Bash", None);
        assert!(
            !runner
                .check_allowed(&HookEvent::PreToolUse, &ctx_bash)
                .await
        );

        let ctx_read = HookContext::for_tool_call("file_read", None);
        assert!(
            runner
                .check_allowed(&HookEvent::PreToolUse, &ctx_read)
                .await
        );
    }

    #[tokio::test]
    async fn test_hook_matcher_regex() {
        let mut config = HookConfig::empty();
        config.add_group(
            HookEvent::PreToolUse,
            HookGroup {
                matcher: Some("Write|Edit".to_string()),
                hooks: vec![HookSpec {
                    hook_type: "command".to_string(),
                    command: "exit 2".to_string(),
                    timeout: Some(5),
                    status_message: None,
                }],
            },
        );
        let runner = HookRunner::new(config);

        let ctx_write = HookContext::for_tool_call("Write", None);
        assert!(
            !runner
                .check_allowed(&HookEvent::PreToolUse, &ctx_write)
                .await
        );

        let ctx_edit = HookContext::for_tool_call("Edit", None);
        assert!(
            !runner
                .check_allowed(&HookEvent::PreToolUse, &ctx_edit)
                .await
        );

        let ctx_read = HookContext::for_tool_call("Read", None);
        assert!(
            runner
                .check_allowed(&HookEvent::PreToolUse, &ctx_read)
                .await
        );
    }

    #[tokio::test]
    async fn test_hook_timeout() {
        let mut config = HookConfig::empty();
        config.add_group(
            HookEvent::PreToolUse,
            HookGroup {
                matcher: None,
                hooks: vec![HookSpec {
                    hook_type: "command".to_string(),
                    command: "sleep 10".to_string(),
                    timeout: Some(1),
                    status_message: None,
                }],
            },
        );
        let runner = HookRunner::new(config);
        let ctx = HookContext::for_tool_call("file_read", None);
        let results = runner.run_hooks(&HookEvent::PreToolUse, &ctx).await;
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0], HookResult::Blocked(ref msg) if msg.contains("timed out")));
    }

    #[tokio::test]
    async fn test_session_start_hook_receives_context() {
        let mut config = HookConfig::empty();
        config.add_group(
            HookEvent::SessionStart,
            HookGroup {
                matcher: None,
                hooks: vec![HookSpec {
                    hook_type: "command".to_string(),
                    command: "cat".to_string(),
                    timeout: Some(5),
                    status_message: None,
                }],
            },
        );
        let runner = HookRunner::new(config);
        let ctx = HookContext::for_session_start();
        let results = runner.run_hooks(&HookEvent::SessionStart, &ctx).await;
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0], HookResult::Allowed));
    }

    #[test]
    fn test_hook_context_json() {
        let ctx = HookContext::for_tool_call("Bash", Some("rm -rf /"));
        let json = ctx.to_json();
        assert_eq!(json["tool_name"], "Bash");
        assert_eq!(json["command"], "rm -rf /");

        let ctx_start = HookContext::for_session_start();
        let json_start = ctx_start.to_json();
        assert!(json_start.as_object().unwrap().is_empty());

        let ctx_stop = HookContext::for_stop(Some("the answer"));
        assert_eq!(ctx_stop.to_json()["answer"], "the answer");
    }

    #[tokio::test]
    async fn test_multiple_hooks_first_block_stops() {
        let mut config = HookConfig::empty();
        config.add_group(
            HookEvent::PreToolUse,
            HookGroup {
                matcher: None,
                hooks: vec![
                    HookSpec {
                        hook_type: "command".to_string(),
                        command: "exit 0".to_string(),
                        timeout: Some(5),
                        status_message: None,
                    },
                    HookSpec {
                        hook_type: "command".to_string(),
                        command: "echo 'nope' >&2; exit 2".to_string(),
                        timeout: Some(5),
                        status_message: None,
                    },
                ],
            },
        );
        let runner = HookRunner::new(config);
        let ctx = HookContext::for_tool_call("file_read", None);
        assert!(!runner.check_allowed(&HookEvent::PreToolUse, &ctx).await);
    }
}

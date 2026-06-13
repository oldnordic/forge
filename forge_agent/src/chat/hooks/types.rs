use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[non_exhaustive]
pub enum HookEvent {
    SessionStart,
    PreToolUse,
    PostToolUse,
    Stop,
    SubagentStop,
}

impl HookEvent {
    pub fn all() -> Vec<HookEvent> {
        vec![
            HookEvent::SessionStart,
            HookEvent::PreToolUse,
            HookEvent::PostToolUse,
            HookEvent::Stop,
            HookEvent::SubagentStop,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            HookEvent::SessionStart => "SessionStart",
            HookEvent::PreToolUse => "PreToolUse",
            HookEvent::PostToolUse => "PostToolUse",
            HookEvent::Stop => "Stop",
            HookEvent::SubagentStop => "SubagentStop",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct HookSpec {
    #[serde(rename = "type")]
    pub hook_type: String,
    pub command: String,
    #[serde(default)]
    pub timeout: Option<u64>,
    #[serde(default)]
    pub status_message: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct HookGroup {
    #[serde(default)]
    pub matcher: Option<String>,
    pub hooks: Vec<HookSpec>,
}

#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct HookConfig {
    pub groups: HashMap<HookEvent, Vec<HookGroup>>,
}

impl HookConfig {
    pub fn empty() -> Self {
        HookConfig {
            groups: HashMap::new(),
        }
    }

    pub fn groups_for(&self, event: &HookEvent) -> &[HookGroup] {
        self.groups.get(event).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn add_group(&mut self, event: HookEvent, group: HookGroup) {
        self.groups.entry(event).or_default().push(group);
    }

    pub fn is_empty(&self) -> bool {
        self.groups.values().all(|v| v.is_empty())
    }

    pub fn from_toml_section(value: &toml::Value) -> Result<Self, String> {
        let table = value
            .as_table()
            .ok_or_else(|| "hooks section must be a table".to_string())?;

        let mut config = HookConfig::empty();

        for (key, val) in table {
            let event = match key.as_str() {
                "SessionStart" => HookEvent::SessionStart,
                "PreToolUse" => HookEvent::PreToolUse,
                "PostToolUse" => HookEvent::PostToolUse,
                "Stop" => HookEvent::Stop,
                "SubagentStop" => HookEvent::SubagentStop,
                _ => {
                    continue;
                }
            };

            let groups = parse_hook_groups(val)?;
            config.groups.insert(event, groups);
        }

        Ok(config)
    }
}

fn parse_hook_groups(value: &toml::Value) -> Result<Vec<HookGroup>, String> {
    let arr = value
        .as_array()
        .ok_or_else(|| "hook event value must be an array of groups".to_string())?;

    let mut groups = Vec::new();
    for item in arr {
        let table = item
            .as_table()
            .ok_or_else(|| "each hook group must be a table".to_string())?;

        let matcher = table
            .get("matcher")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let hooks_val = table
            .get("hooks")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "each hook group must have a 'hooks' array".to_string())?;

        let mut hooks = Vec::new();
        for hook_val in hooks_val {
            let ht = hook_val
                .as_table()
                .ok_or_else(|| "each hook must be a table".to_string())?;

            let hook_type = ht
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("command")
                .to_string();

            let command = ht
                .get("command")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "each hook must have a 'command' field".to_string())?
                .to_string();

            let timeout = ht
                .get("timeout")
                .and_then(|v| v.as_integer())
                .map(|t| t as u64);

            let status_message = ht
                .get("statusMessage")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            hooks.push(HookSpec {
                hook_type,
                command,
                timeout,
                status_message,
            });
        }

        groups.push(HookGroup { matcher, hooks });
    }

    Ok(groups)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_config() {
        let config = HookConfig::empty();
        assert!(config.is_empty());
        assert!(config.groups_for(&HookEvent::SessionStart).is_empty());
    }

    #[test]
    fn test_add_group() {
        let mut config = HookConfig::empty();
        config.add_group(
            HookEvent::PreToolUse,
            HookGroup {
                matcher: Some("Bash".to_string()),
                hooks: vec![HookSpec {
                    hook_type: "command".to_string(),
                    command: "echo check".to_string(),
                    timeout: Some(5),
                    status_message: None,
                }],
            },
        );
        assert!(!config.is_empty());
        assert_eq!(config.groups_for(&HookEvent::PreToolUse).len(), 1);
        assert_eq!(config.groups_for(&HookEvent::Stop).len(), 0);
    }

    #[test]
    fn test_from_toml_section() {
        let toml_str = r#"
[[PreToolUse]]
matcher = "Bash"
hooks = [{type = "command", command = "echo check", timeout = 5}]
"#;
        let value: toml::Value = toml::from_str(toml_str).expect("invariant: valid TOML");
        let config = HookConfig::from_toml_section(&value).expect("invariant: valid config");
        assert_eq!(config.groups_for(&HookEvent::PreToolUse).len(), 1);
    }

    #[test]
    fn test_from_toml_multiple_events() {
        let toml_str = r#"
[[SessionStart]]
hooks = [{type = "command", command = "echo start", timeout = 15}]

[[PreToolUse]]
matcher = "Bash"
hooks = [{type = "command", command = "echo check", timeout = 5}]

[[Stop]]
hooks = [{type = "command", command = "echo stop"}]
"#;
        let value: toml::Value = toml::from_str(toml_str).expect("invariant: valid TOML");
        let config = HookConfig::from_toml_section(&value).expect("invariant: valid config");
        assert_eq!(config.groups_for(&HookEvent::SessionStart).len(), 1);
        assert_eq!(config.groups_for(&HookEvent::PreToolUse).len(), 1);
        assert_eq!(config.groups_for(&HookEvent::Stop).len(), 1);
    }

    #[test]
    fn test_unknown_event_keys_ignored() {
        let toml_str = r#"
[[UnknownEvent]]
hooks = [{type = "command", command = "echo hi"}]

[[Stop]]
hooks = [{type = "command", command = "echo stop"}]
"#;
        let value: toml::Value = toml::from_str(toml_str).expect("invariant: valid TOML");
        let config = HookConfig::from_toml_section(&value).expect("invariant: valid config");
        assert_eq!(config.groups_for(&HookEvent::Stop).len(), 1);
        assert_eq!(config.groups_for(&HookEvent::SessionStart).len(), 0);
    }
}

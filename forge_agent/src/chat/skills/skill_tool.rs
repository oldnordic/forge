use super::loader::SkillLoader;
use super::registry::SkillRegistry;
use crate::chat::tools::registry::AsyncTool;
use crate::chat::tools::types::ToolDef;
use async_trait::async_trait;
use std::sync::Arc;

pub struct SkillTool {
    registry: Arc<SkillRegistry>,
}

impl SkillTool {
    pub fn new(registry: Arc<SkillRegistry>) -> Self {
        SkillTool { registry }
    }

    pub fn with_loader(loader: SkillLoader) -> Self {
        let registry = Arc::new(SkillRegistry::new(loader));
        SkillTool { registry }
    }

    pub fn registry(&self) -> Arc<SkillRegistry> {
        self.registry.clone()
    }
}

#[async_trait]
impl AsyncTool for SkillTool {
    async fn call(&self, arguments: serde_json::Value) -> Result<String, String> {
        let command = arguments["command"]
            .as_str()
            .ok_or_else(|| "Missing 'command' parameter".to_string())?;

        match command {
            "list" => {
                let skills = self.registry.available_skills();
                if skills.is_empty() {
                    return Ok("No skills available.".to_string());
                }
                let lines: Vec<String> = skills
                    .iter()
                    .map(|s| {
                        let triggers = if s.triggers.is_empty() {
                            String::new()
                        } else {
                            format!(" (triggers: {})", s.triggers.join(", "))
                        };
                        format!("- {}{}: {}", s.name, triggers, s.description)
                    })
                    .collect();
                Ok(format!(
                    "Available skills ({}):\n{}",
                    skills.len(),
                    lines.join("\n")
                ))
            }
            "load" => {
                let name = arguments["name"]
                    .as_str()
                    .ok_or_else(|| "Missing 'name' parameter for load".to_string())?;

                let content = self
                    .registry
                    .load(name)
                    .await
                    .ok_or_else(|| format!("Skill '{name}' not found"))?;

                Ok(content.system_prompt_fragment())
            }
            "search" => {
                let query = arguments["query"]
                    .as_str()
                    .ok_or_else(|| "Missing 'query' parameter for search".to_string())?;

                let matches = self.registry.find_matching(query);
                if matches.is_empty() {
                    return Ok(format!("No skills matching '{query}'."));
                }
                let lines: Vec<String> = matches
                    .iter()
                    .map(|s| format!("- {}: {}", s.name, s.description))
                    .collect();
                Ok(format!(
                    "Skills matching '{}' ({}):\n{}",
                    query,
                    matches.len(),
                    lines.join("\n")
                ))
            }
            _ => Err(format!(
                "Unknown skill command: '{command}'. Available: list, load, search"
            )),
        }
    }

    fn definition(&self) -> ToolDef {
        ToolDef::new(
            "skill",
            "Manage and load skills. Commands: list (show available skills), load (load a skill's instructions into context), search (find skills matching a query). Skills provide specialized workflows and instructions.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "enum": ["list", "load", "search"],
                        "description": "The skill command to execute"
                    },
                    "name": {
                        "type": "string",
                        "description": "Skill name (required for load)"
                    },
                    "query": {
                        "type": "string",
                        "description": "Search query (required for search)"
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

    fn setup_test_env() -> (tempfile::TempDir, SkillTool) {
        let temp = tempfile::tempdir().expect("invariant: tempdir creation succeeds");
        let skill_dir = temp.path().join(".forge").join("skills").join("test-skill");
        std::fs::create_dir_all(&skill_dir).expect("invariant: dir creation succeeds");
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "# Test Skill\n\nA skill for testing.\nTriggers: testing, verify",
        )
        .expect("invariant: write succeeds");
        let loader =
            SkillLoader::with_search_paths(vec![temp.path().join(".forge").join("skills")]);
        let tool = SkillTool::with_loader(loader);
        (temp, tool)
    }

    #[tokio::test]
    async fn test_skill_list() {
        let (_temp, tool) = setup_test_env();
        let result = tool
            .call(serde_json::json!({"command": "list"}))
            .await
            .expect("invariant: succeeds");
        assert!(result.contains("test-skill"));
        assert!(result.contains("testing"));
    }

    #[tokio::test]
    async fn test_skill_load() {
        let (_temp, tool) = setup_test_env();
        let result = tool
            .call(serde_json::json!({"command": "load", "name": "test-skill"}))
            .await
            .expect("invariant: succeeds");
        assert!(result.contains("Test Skill"));
        assert!(result.contains("A skill for testing."));
    }

    #[tokio::test]
    async fn test_skill_load_not_found() {
        let (_temp, tool) = setup_test_env();
        let result = tool
            .call(serde_json::json!({"command": "load", "name": "nonexistent"}))
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[tokio::test]
    async fn test_skill_search() {
        let (_temp, tool) = setup_test_env();
        let result = tool
            .call(serde_json::json!({"command": "search", "query": "verify"}))
            .await
            .expect("invariant: succeeds");
        assert!(result.contains("test-skill"));
    }

    #[tokio::test]
    async fn test_skill_search_no_match() {
        let (_temp, tool) = setup_test_env();
        let result = tool
            .call(serde_json::json!({"command": "search", "query": "cooking"}))
            .await
            .expect("invariant: succeeds");
        assert!(result.contains("No skills matching"));
    }

    #[tokio::test]
    async fn test_skill_unknown_command() {
        let (_temp, tool) = setup_test_env();
        let result = tool.call(serde_json::json!({"command": "delete"})).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown skill command"));
    }

    #[test]
    fn test_skill_tool_definition() {
        let (_temp, tool) = setup_test_env();
        let def = tool.definition();
        assert_eq!(def.name, "skill");
    }
}

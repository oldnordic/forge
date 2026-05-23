//! Context composition — codebase-level framing for observer and planner prompts.

use serde::{Deserialize, Serialize};

/// Codebase-level context passed into the observer and planner phases.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentContext {
    /// Human-readable project name derived from the codebase directory name.
    pub project_name: String,
    /// Primary programming language of the project.
    pub language: String,
}

impl AgentContext {
    /// Build context from a codebase path.
    pub fn from_path(path: &std::path::Path) -> Self {
        let project_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        Self {
            project_name,
            language: "rust".to_string(),
        }
    }

    /// Short prefix injected into LLM prompts to frame the codebase.
    pub fn context_prefix(&self) -> String {
        format!(
            "[Project: {}, Language: {}]",
            self.project_name, self.language
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_path_uses_dir_name() {
        let path = std::path::Path::new("/home/user/my-project");
        let ctx = AgentContext::from_path(path);
        assert_eq!(ctx.project_name, "my-project");
        assert_eq!(ctx.language, "rust");
    }

    #[test]
    fn test_context_prefix_format() {
        let ctx = AgentContext {
            project_name: "forge".to_string(),
            language: "rust".to_string(),
        };
        let prefix = ctx.context_prefix();
        assert!(
            prefix.contains("forge"),
            "prefix should contain project name"
        );
        assert!(prefix.contains("rust"), "prefix should contain language");
    }
}

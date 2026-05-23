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
        let language = Self::detect_language(path);
        Self {
            project_name,
            language,
        }
    }

    fn detect_language(path: &std::path::Path) -> String {
        if path.join("Cargo.toml").exists() {
            return "rust".to_string();
        }
        if path.join("go.mod").exists() {
            return "go".to_string();
        }
        if path.join("package.json").exists() {
            if path.join("tsconfig.json").exists() {
                return "typescript".to_string();
            }
            return "javascript".to_string();
        }
        if path.join("pyproject.toml").exists() || path.join("setup.py").exists() {
            return "python".to_string();
        }
        if path.join("pom.xml").exists() || path.join("build.gradle").exists() {
            return "java".to_string();
        }
        "unknown".to_string()
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
    use std::fs::File;
    use tempfile::TempDir;

    #[test]
    fn test_from_path_uses_dir_name() {
        let path = std::path::Path::new("/home/user/my-project");
        let ctx = AgentContext::from_path(path);
        assert_eq!(ctx.project_name, "my-project");
        assert_eq!(ctx.language, "unknown");
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

    #[test]
    fn test_detect_language_unknown_when_no_markers() {
        let dir = TempDir::new().unwrap();
        let ctx = AgentContext::from_path(dir.path());
        assert_eq!(ctx.language, "unknown");
    }

    #[test]
    fn test_detect_language_rust_from_cargo_toml() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("Cargo.toml")).unwrap();
        let ctx = AgentContext::from_path(dir.path());
        assert_eq!(ctx.language, "rust");
    }

    #[test]
    fn test_detect_language_go_from_go_mod() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("go.mod")).unwrap();
        let ctx = AgentContext::from_path(dir.path());
        assert_eq!(ctx.language, "go");
    }

    #[test]
    fn test_detect_language_javascript_from_package_json() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("package.json")).unwrap();
        let ctx = AgentContext::from_path(dir.path());
        assert_eq!(ctx.language, "javascript");
    }

    #[test]
    fn test_detect_language_typescript_from_tsconfig() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("package.json")).unwrap();
        File::create(dir.path().join("tsconfig.json")).unwrap();
        let ctx = AgentContext::from_path(dir.path());
        assert_eq!(ctx.language, "typescript");
    }

    #[test]
    fn test_detect_language_python_from_pyproject_toml() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("pyproject.toml")).unwrap();
        let ctx = AgentContext::from_path(dir.path());
        assert_eq!(ctx.language, "python");
    }

    #[test]
    fn test_detect_language_python_from_setup_py() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("setup.py")).unwrap();
        let ctx = AgentContext::from_path(dir.path());
        assert_eq!(ctx.language, "python");
    }
}

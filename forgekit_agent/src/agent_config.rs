//! Configuration-driven agent construction.
//!
//! Loads an `[agent]` section from `.forge.toml` (or any TOML file) and
//! produces an `AgentConfig` that can be applied to an `Agent` builder chain.
//!
//! ## Example `.forge.toml`
//!
//! ```toml
//! [llm]
//! provider = "ollama"
//! model = "qwen3.5-agent:latest"
//! url = "http://localhost:11434"
//!
//! [agent]
//! max_iterations = 20
//! step_retries = 3
//! retrieval_top_k = 10
//! system_prompt = "You are an expert Rust developer."
//! tools = ["file_read", "file_write", "shell_exec", "graph_query"]
//! ```

use std::path::Path;

/// Default maximum ReAct loop iterations.
const DEFAULT_MAX_ITERATIONS: usize = 10;

/// Default consecutive error retries per step.
const DEFAULT_STEP_RETRIES: usize = 2;

/// Default number of RAG retrieval results.
const DEFAULT_RETRIEVAL_TOP_K: usize = 5;

/// Agent configuration loaded from `.forge.toml`.
///
/// All fields are optional — the agent uses built-in defaults for anything
/// not specified in the config file.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct AgentConfig {
    /// Maximum ReAct loop iterations (default 10).
    #[serde(default)]
    pub max_iterations: Option<usize>,

    /// Maximum consecutive LLM errors before failing (default 2).
    #[serde(default)]
    pub step_retries: Option<usize>,

    /// Number of retrieval results to inject for RAG (default 5).
    #[serde(default)]
    pub retrieval_top_k: Option<usize>,

    /// Custom system prompt replacing the default.
    #[serde(default)]
    pub system_prompt: Option<String>,

    /// Tool allowlist. When set, only these tool names are registered.
    /// When absent, all builtin tools are available.
    #[serde(default)]
    pub tools: Option<Vec<String>>,

    /// Tool denylist. Always takes precedence over the allowlist.
    /// Tools listed here are never registered regardless of `tools`.
    #[serde(default)]
    pub denied_tools: Option<Vec<String>>,

    /// Shell command patterns to block (regex). Applied to shell_exec commands.
    #[serde(default)]
    pub blocked_commands: Option<Vec<String>>,

    /// File patterns to block from read/write (regex). Applied to file_read/file_write.
    #[serde(default)]
    pub blocked_paths: Option<Vec<String>>,

    /// Temperature override for LLM calls (requires `[llm]` section).
    #[serde(default)]
    pub temperature: Option<f32>,

    /// Maximum tokens for LLM responses (requires `[llm]` section).
    #[serde(default)]
    pub max_tokens: Option<u32>,
}

impl AgentConfig {
    /// Load agent config from a `.forge.toml` file.
    ///
    /// Returns `Ok(None)` if the file does not exist or has no `[agent]` section.
    /// Returns an error only for I/O or parse failures on an existing file.
    pub fn from_file(path: &Path) -> std::io::Result<Option<Self>> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        Self::parse_toml(&text)
    }

    /// Parse agent config from a TOML string.
    ///
    /// Returns `Ok(None)` if the string has no `[agent]` section.
    pub fn parse_toml(toml_text: &str) -> std::io::Result<Option<Self>> {
        #[derive(serde::Deserialize)]
        struct ForgeToml {
            #[serde(default)]
            agent: Option<AgentConfig>,
        }
        let parsed: ForgeToml =
            toml::from_str(toml_text).map_err(|e| std::io::Error::other(e.to_string()))?;
        Ok(parsed.agent)
    }

    /// Resolved `max_iterations` with default fallback.
    pub fn max_iterations(&self) -> usize {
        self.max_iterations.unwrap_or(DEFAULT_MAX_ITERATIONS)
    }

    /// Resolved `step_retries` with default fallback.
    pub fn step_retries(&self) -> usize {
        self.step_retries.unwrap_or(DEFAULT_STEP_RETRIES)
    }

    /// Resolved `retrieval_top_k` with default fallback.
    pub fn retrieval_top_k(&self) -> usize {
        self.retrieval_top_k.unwrap_or(DEFAULT_RETRIEVAL_TOP_K)
    }

    /// Returns true if the given tool name is allowed by the config.
    ///
    /// Deny list takes precedence over allow list. If neither is set,
    /// all tools are allowed.
    pub fn is_tool_allowed(&self, tool_name: &str) -> bool {
        if let Some(ref denied) = self.denied_tools {
            if denied.iter().any(|t| t == tool_name) {
                return false;
            }
        }
        match &self.tools {
            Some(allowed) => allowed.iter().any(|t| t == tool_name),
            None => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn from_str_no_agent_section() {
        let result = AgentConfig::parse_toml("[llm]\nprovider = \"ollama\"\n").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn from_str_empty_agent_section() {
        let result = AgentConfig::parse_toml("[agent]\n").unwrap();
        let config = result.expect("should parse empty [agent]");
        assert_eq!(config.max_iterations(), 10);
        assert_eq!(config.step_retries(), 2);
        assert_eq!(config.retrieval_top_k(), 5);
        assert!(config.system_prompt.is_none());
        assert!(config.tools.is_none());
    }

    #[test]
    fn from_str_full_config() {
        let toml = r#"
[agent]
max_iterations = 20
step_retries = 3
retrieval_top_k = 10
system_prompt = "You are a helpful coding assistant."
tools = ["file_read", "graph_query"]
temperature = 0.7
max_tokens = 4096
"#;
        let config = AgentConfig::parse_toml(toml)
            .unwrap()
            .expect("should parse [agent]");
        assert_eq!(config.max_iterations(), 20);
        assert_eq!(config.step_retries(), 3);
        assert_eq!(config.retrieval_top_k(), 10);
        assert_eq!(
            config.system_prompt.as_deref(),
            Some("You are a helpful coding assistant.")
        );
        let tools = config.tools.as_ref().unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0], "file_read");
        assert_eq!(tools[1], "graph_query");
        assert_eq!(config.temperature.unwrap(), 0.7);
        assert_eq!(config.max_tokens.unwrap(), 4096);
    }

    #[test]
    fn is_tool_allowed_no_allowlist() {
        let config = AgentConfig::default();
        assert!(config.is_tool_allowed("file_read"));
        assert!(config.is_tool_allowed("anything"));
    }

    #[test]
    fn is_tool_allowed_with_allowlist() {
        let config = AgentConfig {
            tools: Some(vec!["file_read".to_string(), "graph_query".to_string()]),
            ..Default::default()
        };
        assert!(config.is_tool_allowed("file_read"));
        assert!(config.is_tool_allowed("graph_query"));
        assert!(!config.is_tool_allowed("shell_exec"));
        assert!(!config.is_tool_allowed("file_write"));
    }

    #[test]
    fn deny_overrides_allow() {
        let config = AgentConfig {
            tools: Some(vec!["file_read".to_string(), "shell_exec".to_string()]),
            denied_tools: Some(vec!["shell_exec".to_string()]),
            ..Default::default()
        };
        assert!(config.is_tool_allowed("file_read"));
        assert!(!config.is_tool_allowed("shell_exec"));
    }

    #[test]
    fn deny_without_allow() {
        let config = AgentConfig {
            denied_tools: Some(vec!["shell_exec".to_string()]),
            ..Default::default()
        };
        assert!(config.is_tool_allowed("file_read"));
        assert!(!config.is_tool_allowed("shell_exec"));
        assert!(config.is_tool_allowed("file_write"));
    }

    #[test]
    fn parse_blocked_commands_and_paths() {
        let toml = r#"
[agent]
blocked_commands = ["sudo.*", "rm\\s+-rf"]
blocked_paths = ["\\.env", "credentials"]
"#;
        let config = AgentConfig::parse_toml(toml).unwrap().unwrap();
        assert_eq!(config.blocked_commands.as_ref().unwrap().len(), 2);
        assert_eq!(config.blocked_paths.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn from_file_missing_file() {
        let result = AgentConfig::from_file(Path::new("/nonexistent/.forge.toml")).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn from_file_with_agent_section() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".forge.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        write!(f, "[agent]\nmax_iterations = 15\nstep_retries = 5\n").unwrap();
        let config = AgentConfig::from_file(&path).unwrap().unwrap();
        assert_eq!(config.max_iterations(), 15);
        assert_eq!(config.step_retries(), 5);
    }

    #[test]
    fn from_file_without_agent_section() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".forge.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        write!(f, "[llm]\nprovider = \"ollama\"\nmodel = \"test\"\n").unwrap();
        let result = AgentConfig::from_file(&path).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn from_str_invalid_toml() {
        let result = AgentConfig::parse_toml("not valid toml [[[");
        assert!(result.is_err());
    }

    #[test]
    fn from_str_partial_config() {
        let config = AgentConfig::parse_toml("[agent]\nmax_iterations = 42\n")
            .unwrap()
            .unwrap();
        assert_eq!(config.max_iterations(), 42);
        assert_eq!(config.step_retries(), 2);
        assert_eq!(config.retrieval_top_k(), 5);
    }
}

//! Code generation from natural language descriptions.

use crate::llm::LlmProvider;
use crate::observe::Observer;
use crate::AgentError;
use forge_core::Forge;
use std::path::PathBuf;
use std::sync::Arc;

/// Generates new code from a natural language description.
///
/// Queries the graph for relevant context (existing symbols, patterns),
/// builds a prompt, and calls the LLM.
pub struct Generator {
    forge: Arc<Forge>,
    llm: Arc<dyn LlmProvider>,
}

impl Generator {
    pub fn new(forge: Arc<Forge>, llm: Arc<dyn LlmProvider>) -> Self {
        Self { forge, llm }
    }

    /// Generate code matching `description`.
    ///
    /// Returns `GeneratedCode` with the LLM output and an optional
    /// suggested file path (populated when the LLM returns a JSON
    /// `{"path":"...","code":"..."}` envelope).
    pub async fn generate(&self, description: &str) -> Result<GeneratedCode, AgentError> {
        let observer = Observer::new((*self.forge).clone());
        let observation = observer
            .gather(description)
            .await
            .map_err(|e| AgentError::ObservationFailed(format!("generate context failed: {e}")))?;

        let symbol_list: Vec<String> = observation
            .symbols
            .iter()
            .map(|s| format!("{} (id:{})", s.name, s.id.0))
            .collect();

        let prompt = format!(
            "Task: {}\nExisting symbols in codebase: [{}]\n\nGenerate Rust code for the task. \
If you want to suggest a file path, respond with JSON: {{\"path\":\"src/...\",\"code\":\"...\"}}. \
Otherwise, respond with plain Rust code only.",
            description,
            symbol_list.join(", ")
        );

        let system = "You are a Rust code generator. \
Write idiomatic, minimal Rust code. No explanations outside the code. \
Only public items where needed. Follow existing project patterns.";

        let raw = self
            .llm
            .complete(&prompt, Some(system))
            .await
            .map_err(|e| AgentError::PlanningFailed(format!("LLM generate failed: {e}")))?;

        Ok(parse_generated(&raw))
    }
}

/// Output of a code generation request.
#[derive(Debug, Clone)]
pub struct GeneratedCode {
    /// The generated Rust code.
    pub content: String,
    /// Suggested file path returned by the LLM, if any.
    pub suggested_path: Option<PathBuf>,
}

/// Attempt to parse JSON envelope `{"path":"...","code":"..."}`.
/// Falls back to treating the whole response as plain code.
fn parse_generated(raw: &str) -> GeneratedCode {
    let trimmed = raw.trim();
    if trimmed.starts_with('{') {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
            let code = v["code"].as_str().unwrap_or("").to_string();
            let path = v["path"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(PathBuf::from);
            if !code.is_empty() {
                return GeneratedCode {
                    content: code,
                    suggested_path: path,
                };
            }
        }
    }
    GeneratedCode {
        content: trimmed.to_string(),
        suggested_path: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::MockProvider;
    use std::sync::Arc;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_generate_returns_llm_content() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let llm = Arc::new(MockProvider::new("fn add(a: i32, b: i32) -> i32 { a + b }"));
        let gen = Generator::new(Arc::new(forge), llm);

        let result = gen.generate("add two integers").await.unwrap();

        assert!(result.content.contains("fn add"), "got: {}", result.content);
        assert!(result.suggested_path.is_none());
    }

    #[tokio::test]
    async fn test_generate_with_empty_codebase() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let llm = Arc::new(MockProvider::new("fn new_fn() {}"));
        let gen = Generator::new(Arc::new(forge), llm);

        let result = gen.generate("add a helper function").await.unwrap();
        assert!(!result.content.is_empty());
    }

    #[tokio::test]
    async fn test_generate_parses_json_envelope() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let llm = Arc::new(MockProvider::new(
            r#"{"path":"src/helpers.rs","code":"fn helper() {}"}"#,
        ));
        let gen = Generator::new(Arc::new(forge), llm);

        let result = gen.generate("add helper").await.unwrap();
        assert_eq!(result.content, "fn helper() {}");
        assert_eq!(result.suggested_path, Some(PathBuf::from("src/helpers.rs")));
    }

    #[test]
    fn test_parse_generated_plain_code() {
        let result = parse_generated("fn foo() {}");
        assert_eq!(result.content, "fn foo() {}");
        assert!(result.suggested_path.is_none());
    }

    #[test]
    fn test_parse_generated_json_malformed_falls_back() {
        let result = parse_generated("{not valid json}");
        assert_eq!(result.content, "{not valid json}");
        assert!(result.suggested_path.is_none());
    }

    #[test]
    fn test_parse_generated_json_no_code_field_falls_back() {
        let result = parse_generated(r#"{"other":"value"}"#);
        assert_eq!(result.content, r#"{"other":"value"}"#);
        assert!(result.suggested_path.is_none());
    }
}

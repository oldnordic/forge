//! LLM provider integration (stub — Phase 4 placeholder).

use serde::{Deserialize, Serialize};

/// LLM provider trait — async with optional system prompt.
#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    /// Return a response for the given prompt.
    async fn complete(&self, prompt: &str, system: Option<&str>) -> Result<String, String>;
}

/// Stub LLM configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LlmConfig;

/// Stub mock provider for tests.
pub struct MockProvider {
    response: String,
}

impl MockProvider {
    /// Create a new mock provider with a canned response.
    pub fn new(response: impl Into<String>) -> Self {
        Self {
            response: response.into(),
        }
    }
}

#[async_trait::async_trait]
impl LlmProvider for MockProvider {
    async fn complete(&self, _prompt: &str, _system: Option<&str>) -> Result<String, String> {
        Ok(self.response.clone())
    }
}

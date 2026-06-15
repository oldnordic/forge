//! LLM provider integration.
//!
//! Implements the `LlmProvider` trait for multiple backends:
//! - `OllamaProvider`    — local Ollama server (`llm-ollama` feature)
//! - `OpenAiProvider`    — OpenAI chat completions API (`llm-openai` feature)
//! - `AnthropicProvider` — Anthropic Messages API (`llm-anthropic` feature)

#[cfg(any(
    feature = "llm-ollama",
    feature = "llm-openai",
    feature = "llm-anthropic"
))]
use serde::{Deserialize, Serialize};

/// Async text-completion contract shared by all LLM backends.
///
/// ## Stability
///
/// This trait is part of the stable SDK contract. Breaking changes to the
/// signature will be accompanied by a major version bump.
#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    /// Return a completion for the given user prompt and optional system prompt.
    async fn complete(&self, prompt: &str, system: Option<&str>) -> Result<String, String>;
}

/// LLM configuration for provider requests.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LlmConfig {
    pub model: String,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub stop: Vec<String>,
    #[serde(default)]
    pub json_mode: bool,
    #[serde(default = "default_max_tool_output_bytes")]
    pub max_tool_output_bytes: usize,
}

fn default_max_tool_output_bytes() -> usize {
    8192
}

impl LlmConfig {
    pub fn new(model: impl Into<String>) -> Self {
        LlmConfig {
            model: model.into(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            stop: Vec::new(),
            json_mode: false,
            max_tool_output_bytes: default_max_tool_output_bytes(),
        }
    }

    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }

    pub fn with_max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    pub fn with_json_mode(mut self) -> Self {
        self.json_mode = true;
        self
    }
}

/// Canned-response provider for tests.
#[cfg(test)]
pub struct MockProvider {
    response: String,
}

#[cfg(test)]
impl MockProvider {
    pub fn new(response: impl Into<String>) -> Self {
        Self {
            response: response.into(),
        }
    }
}

#[cfg(test)]
#[async_trait::async_trait]
impl LlmProvider for MockProvider {
    async fn complete(&self, _prompt: &str, _system: Option<&str>) -> Result<String, String> {
        Ok(self.response.clone())
    }
}

/// Capturing provider for tests — records the last prompt sent.
#[cfg(test)]
pub struct CapturingMockProvider {
    response: String,
    pub last_prompt: std::sync::Mutex<Option<String>>,
}

#[cfg(test)]
impl CapturingMockProvider {
    pub fn new(response: impl Into<String>) -> Self {
        Self {
            response: response.into(),
            last_prompt: std::sync::Mutex::new(None),
        }
    }
}

#[cfg(test)]
#[async_trait::async_trait]
impl LlmProvider for CapturingMockProvider {
    async fn complete(&self, prompt: &str, _system: Option<&str>) -> Result<String, String> {
        *self.last_prompt.lock().unwrap() = Some(prompt.to_string());
        Ok(self.response.clone())
    }
}

// ── Ollama ────────────────────────────────────────────────────────────────────

#[cfg(feature = "llm-ollama")]
pub struct OllamaProvider {
    client: reqwest::Client,
    endpoint: String,
    model: String,
}

#[cfg(feature = "llm-ollama")]
impl OllamaProvider {
    /// Connect to a specific Ollama endpoint.
    pub fn new(endpoint: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            endpoint: endpoint.into(),
            model: model.into(),
        }
    }

    /// Connect to `http://localhost:11434` (default Ollama install).
    pub fn local(model: impl Into<String>) -> Self {
        Self::new("http://localhost:11434", model)
    }
}

#[cfg(feature = "llm-ollama")]
#[async_trait::async_trait]
impl LlmProvider for OllamaProvider {
    async fn complete(&self, prompt: &str, system: Option<&str>) -> Result<String, String> {
        #[derive(Serialize)]
        struct Req<'a> {
            model: &'a str,
            prompt: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            system: Option<&'a str>,
            stream: bool,
        }
        #[derive(Deserialize)]
        struct Resp {
            response: String,
        }

        let resp = self
            .client
            .post(format!("{}/api/generate", self.endpoint))
            .json(&Req {
                model: &self.model,
                prompt,
                system,
                stream: false,
            })
            .send()
            .await
            .map_err(|e| format!("Ollama request failed: {e}"))?;

        if !resp.status().is_success() {
            let s = resp.status();
            return Err(format!(
                "Ollama {s}: {}",
                resp.text().await.unwrap_or_default()
            ));
        }
        Ok(resp
            .json::<Resp>()
            .await
            .map_err(|e| format!("Ollama parse: {e}"))?
            .response)
    }
}

// ── OpenAI ────────────────────────────────────────────────────────────────────

/// OpenAI-compatible chat completions provider.
///
/// Works with OpenAI (`https://api.openai.com/v1`) and any OpenAI-compatible
/// endpoint (Together, Fireworks, vLLM, LM Studio, etc.).
#[cfg(feature = "llm-openai")]
pub struct OpenAiProvider {
    client: reqwest::Client,
    endpoint: String,
    model: String,
    api_key: String,
}

#[cfg(feature = "llm-openai")]
impl OpenAiProvider {
    /// Connect to any OpenAI-compatible endpoint.
    pub fn new(
        endpoint: impl Into<String>,
        model: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            endpoint: endpoint.into(),
            model: model.into(),
            api_key: api_key.into(),
        }
    }

    /// Use the official OpenAI API (`https://api.openai.com/v1`).
    pub fn openai(model: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self::new("https://api.openai.com/v1", model, api_key)
    }

    /// Use a local OpenAI-compatible server (e.g. LM Studio on port 1234).
    pub fn local_compatible(port: u16, model: impl Into<String>) -> Self {
        Self::new(format!("http://localhost:{port}/v1"), model, "local")
    }
}

#[cfg(feature = "llm-openai")]
#[async_trait::async_trait]
impl LlmProvider for OpenAiProvider {
    async fn complete(&self, prompt: &str, system: Option<&str>) -> Result<String, String> {
        #[derive(Serialize)]
        struct Message<'a> {
            role: &'a str,
            content: &'a str,
        }
        #[derive(Serialize)]
        struct Req<'a> {
            model: &'a str,
            messages: Vec<Message<'a>>,
        }
        #[derive(Deserialize)]
        struct Choice {
            message: MessageOut,
        }
        #[derive(Deserialize)]
        struct MessageOut {
            content: String,
        }
        #[derive(Deserialize)]
        struct Resp {
            choices: Vec<Choice>,
        }

        let mut messages = Vec::new();
        if let Some(sys) = system {
            messages.push(Message {
                role: "system",
                content: sys,
            });
        }
        messages.push(Message {
            role: "user",
            content: prompt,
        });

        let resp = self
            .client
            .post(format!("{}/chat/completions", self.endpoint))
            .bearer_auth(&self.api_key)
            .json(&Req {
                model: &self.model,
                messages,
            })
            .send()
            .await
            .map_err(|e| format!("OpenAI request failed: {e}"))?;

        if !resp.status().is_success() {
            let s = resp.status();
            return Err(format!(
                "OpenAI {s}: {}",
                resp.text().await.unwrap_or_default()
            ));
        }
        let mut r = resp
            .json::<Resp>()
            .await
            .map_err(|e| format!("OpenAI parse: {e}"))?;
        r.choices
            .pop()
            .map(|c| c.message.content)
            .ok_or_else(|| "OpenAI returned empty choices".to_string())
    }
}

// ── Anthropic ─────────────────────────────────────────────────────────────────

/// Anthropic Messages API provider (claude-* models).
#[cfg(feature = "llm-anthropic")]
pub struct AnthropicProvider {
    client: reqwest::Client,
    model: String,
    api_key: String,
    max_tokens: u32,
}

#[cfg(feature = "llm-anthropic")]
impl AnthropicProvider {
    pub fn new(model: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            model: model.into(),
            api_key: api_key.into(),
            max_tokens: 4096,
        }
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }
}

#[cfg(feature = "llm-anthropic")]
#[async_trait::async_trait]
impl LlmProvider for AnthropicProvider {
    async fn complete(&self, prompt: &str, system: Option<&str>) -> Result<String, String> {
        #[derive(Serialize)]
        struct Message<'a> {
            role: &'a str,
            content: &'a str,
        }
        #[derive(Serialize)]
        struct Req<'a> {
            model: &'a str,
            max_tokens: u32,
            #[serde(skip_serializing_if = "Option::is_none")]
            system: Option<&'a str>,
            messages: Vec<Message<'a>>,
        }
        #[derive(Deserialize)]
        struct ContentBlock {
            #[serde(rename = "type")]
            kind: String,
            text: Option<String>,
        }
        #[derive(Deserialize)]
        struct Resp {
            content: Vec<ContentBlock>,
        }

        let resp = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&Req {
                model: &self.model,
                max_tokens: self.max_tokens,
                system,
                messages: vec![Message {
                    role: "user",
                    content: prompt,
                }],
            })
            .send()
            .await
            .map_err(|e| format!("Anthropic request failed: {e}"))?;

        if !resp.status().is_success() {
            let s = resp.status();
            return Err(format!(
                "Anthropic {s}: {}",
                resp.text().await.unwrap_or_default()
            ));
        }
        resp.json::<Resp>()
            .await
            .map_err(|e| format!("Anthropic parse: {e}"))?
            .content
            .into_iter()
            .find(|b| b.kind == "text")
            .and_then(|b| b.text)
            .ok_or_else(|| "Anthropic returned no text block".to_string())
    }
}

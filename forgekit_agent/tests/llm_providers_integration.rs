//! Integration tests for all LLM provider backends.
//!
//! Ollama tests run against localhost:11434 (assumed available).
//! OpenAI tests require OPENAI_API_KEY env var.
//! Anthropic tests require ANTHROPIC_API_KEY env var.
//!
//! Run all:
//!   cargo test --features llm-ollama,llm-openai,llm-anthropic \
//!              --test llm_providers_integration -- --nocapture

// ── Ollama ────────────────────────────────────────────────────────────────────

#[cfg(feature = "llm-ollama")]
mod ollama {
    use forgekit_agent::{LlmProvider, OllamaProvider};

    async fn available() -> bool {
        reqwest::Client::new()
            .get("http://localhost:11434/api/tags")
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    #[tokio::test]
    async fn basic_completion() {
        if !available().await {
            eprintln!("SKIP: Ollama not reachable");
            return;
        }
        let p = OllamaProvider::local("qwen3.5:latest");
        let r = p.complete("Reply with exactly: pong", None).await.unwrap();
        println!("ollama: {r}");
        assert!(!r.trim().is_empty());
    }

    #[tokio::test]
    async fn with_system_prompt() {
        if !available().await {
            eprintln!("SKIP: Ollama not reachable");
            return;
        }
        let p = OllamaProvider::local("qwen3.5:latest");
        let r = p
            .complete(
                "What is 2 + 2?",
                Some("Reply with only the numeric answer, nothing else."),
            )
            .await
            .unwrap();
        println!("ollama system: {r}");
        assert!(r.contains('4'), "expected '4' in: {r}");
    }
}

// ── OpenAI ────────────────────────────────────────────────────────────────────

#[cfg(feature = "llm-openai")]
mod openai {
    use forgekit_agent::{LlmProvider, OpenAiProvider};

    fn api_key() -> Option<String> {
        std::env::var("OPENAI_API_KEY").ok()
    }

    #[tokio::test]
    async fn basic_completion() {
        let Some(key) = api_key() else {
            eprintln!("SKIP: OPENAI_API_KEY not set");
            return;
        };
        let p = OpenAiProvider::openai("gpt-4o-mini", key);
        let r = p.complete("Reply with exactly: pong", None).await.unwrap();
        println!("openai: {r}");
        assert!(!r.trim().is_empty());
    }

    #[tokio::test]
    async fn with_system_prompt() {
        let Some(key) = api_key() else {
            eprintln!("SKIP: OPENAI_API_KEY not set");
            return;
        };
        let p = OpenAiProvider::openai("gpt-4o-mini", key);
        let r = p
            .complete(
                "What is 2 + 2?",
                Some("Reply with only the numeric answer, nothing else."),
            )
            .await
            .unwrap();
        println!("openai system: {r}");
        assert!(r.contains('4'), "expected '4' in: {r}");
    }

    /// Verify the OpenAI-compatible path using a local Ollama endpoint.
    /// This validates the code path without needing an OpenAI API key.
    #[tokio::test]
    async fn openai_compatible_via_ollama() {
        let available = reqwest::Client::new()
            .get("http://localhost:11434/api/tags")
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);
        if !available {
            eprintln!("SKIP: Ollama not reachable");
            return;
        }
        // Ollama exposes an OpenAI-compatible endpoint at /v1
        let p = OpenAiProvider::new(
            "http://localhost:11434/v1",
            "qwen3.5:latest",
            "ollama", // any non-empty string
        );
        let r = p.complete("Reply with exactly: pong", None).await.unwrap();
        println!("openai-compat via ollama: {r}");
        assert!(!r.trim().is_empty());
    }
}

// ── Anthropic ─────────────────────────────────────────────────────────────────

#[cfg(feature = "llm-anthropic")]
mod anthropic {
    use forgekit_agent::{AnthropicProvider, LlmProvider};

    fn api_key() -> Option<String> {
        std::env::var("ANTHROPIC_API_KEY").ok()
    }

    #[tokio::test]
    async fn basic_completion() {
        let Some(key) = api_key() else {
            eprintln!("SKIP: ANTHROPIC_API_KEY not set");
            return;
        };
        let p = AnthropicProvider::new("claude-haiku-4-5-20251001", key);
        let r = p.complete("Reply with exactly: pong", None).await.unwrap();
        println!("anthropic: {r}");
        assert!(!r.trim().is_empty());
    }

    #[tokio::test]
    async fn with_system_prompt() {
        let Some(key) = api_key() else {
            eprintln!("SKIP: ANTHROPIC_API_KEY not set");
            return;
        };
        let p = AnthropicProvider::new("claude-haiku-4-5-20251001", key);
        let r = p
            .complete(
                "What is 2 + 2?",
                Some("Reply with only the numeric answer, nothing else."),
            )
            .await
            .unwrap();
        println!("anthropic system: {r}");
        assert!(r.contains('4'), "expected '4' in: {r}");
    }

    #[tokio::test]
    async fn planner_prompt_format() {
        let Some(key) = api_key() else {
            eprintln!("SKIP: ANTHROPIC_API_KEY not set");
            return;
        };
        let p = AnthropicProvider::new("claude-haiku-4-5-20251001", key);
        let system = "You are a code operation planner. Output ONLY a JSON array. No explanation.";
        let prompt = "Query: find add\nSymbols: [add (id:1), subtract (id:2)]";
        let r = p.complete(prompt, Some(system)).await.unwrap();
        println!("anthropic planner: {r}");
        // Should contain JSON array markers
        assert!(
            r.contains('[') && r.contains(']'),
            "expected JSON array in: {r}"
        );
    }
}

// Fallback: at least one test must exist when no features are enabled
#[cfg(not(any(
    feature = "llm-ollama",
    feature = "llm-openai",
    feature = "llm-anthropic"
)))]
#[test]
fn no_llm_features_enabled() {
    eprintln!("No llm-* features enabled; skipping provider tests");
}

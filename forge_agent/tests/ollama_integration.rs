//! Live integration tests against a local Ollama endpoint.
//!
//! Run with: cargo test --features llm-ollama --test ollama_integration -- --nocapture

#[cfg(feature = "llm-ollama")]
mod ollama_tests {
    use forge_agent::{LlmProvider, OllamaProvider};

    const MODEL: &str = "qwen3.5:latest";

    async fn ollama_available() -> bool {
        reqwest::Client::new()
            .get("http://localhost:11434/api/tags")
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    #[tokio::test]
    async fn test_ollama_basic_completion() {
        if !ollama_available().await {
            eprintln!("SKIP: Ollama not reachable at localhost:11434");
            return;
        }

        let provider = OllamaProvider::local(MODEL);
        let response = provider
            .complete("Say exactly: hello forge", None)
            .await
            .expect("Ollama completion failed");

        println!("Response: {response}");
        assert!(!response.trim().is_empty(), "Response should not be empty");
    }

    #[tokio::test]
    async fn test_ollama_with_system_prompt() {
        if !ollama_available().await {
            eprintln!("SKIP: Ollama not reachable at localhost:11434");
            return;
        }

        let provider = OllamaProvider::local(MODEL);
        let response = provider
            .complete(
                "List the symbols in this code: pub fn add(a: i32, b: i32) -> i32 { a + b }",
                Some("You are a code analysis assistant. Reply with a JSON array of symbol names only. No explanation."),
            )
            .await
            .expect("Ollama completion with system prompt failed");

        println!("Response: {response}");
        assert!(!response.trim().is_empty());
        // Should contain "add" somewhere in the response
        assert!(
            response.contains("add"),
            "Expected 'add' in response, got: {response}"
        );
    }

    #[tokio::test]
    async fn test_ollama_planner_prompt() {
        if !ollama_available().await {
            eprintln!("SKIP: Ollama not reachable at localhost:11434");
            return;
        }

        let provider = OllamaProvider::local(MODEL);
        let system = "You are a code operation planner. Given a code query, generate execution steps as a JSON array.\n\n\
            Available operations:\n\
            - {\"operation\":\"inspect\",\"symbol_name\":\"...\",\"symbol_id\":0}\n\n\
            Output ONLY a JSON array. No explanation.";

        let prompt = "Query: find add function\nSummary: Rust lib with add and subtract\nSymbols: [add (id:1), subtract (id:2)]";

        let response = provider
            .complete(prompt, Some(system))
            .await
            .expect("Ollama planner prompt failed");

        println!("Planner response: {response}");
        assert!(!response.trim().is_empty());
    }
}

// Without the feature, the test module is empty — cargo test still passes.
#[cfg(not(feature = "llm-ollama"))]
#[test]
fn ollama_feature_not_enabled() {
    eprintln!("llm-ollama feature not enabled; skipping Ollama tests");
}

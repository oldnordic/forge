//! Multi-agent orchestration example.
//!
//! Demonstrates sequential and parallel agent composition.
//! Run with: `cargo run --example orchestration --features llm-ollama`

#[cfg(not(feature = "llm-ollama"))]
fn main() {
    eprintln!("This example requires the llm-ollama feature.");
    eprintln!("Run with: cargo run --example orchestration --features llm-ollama");
}

#[cfg(feature = "llm-ollama")]
#[tokio::main]
async fn main() {
    use forge_agent::LlmConfig;
    use forge_agent::{chat::OllamaChatProvider, Agent, Orchestrator};
    use std::sync::Arc;

    let provider = Arc::new(OllamaChatProvider::local());
    let config = LlmConfig::new("qwen3.5-agent:latest").with_temperature(0.0);

    let agent1 = Agent::new(".")
        .await
        .expect("agent 1 failed")
        .with_chat_provider(provider.clone(), config.clone())
        .with_max_iterations(3);

    let agent2 = Agent::new(".")
        .await
        .expect("agent 2 failed")
        .with_chat_provider(provider, config)
        .with_max_iterations(3);

    let orchestrator = Orchestrator::new()
        .add_agent_with_id("analyzer", agent1)
        .add_agent_with_id("summarizer", agent2);

    println!("=== Sequential Orchestration ===");
    match orchestrator
        .run_sequential("Analyze the project structure")
        .await
    {
        Ok(results) => {
            for r in &results {
                println!(
                    "[{}] {}",
                    r.agent_id(),
                    if r.is_success() { "OK" } else { "FAIL" }
                );
            }
        }
        Err(e) => println!("Sequential failed: {e}"),
    }
}

//! Minimal agent example using the high-level SDK API.
//!
//! Run with: `cargo run --example minimal_agent --features llm-ollama`

#[cfg(not(feature = "llm-ollama"))]
fn main() {
    eprintln!("This example requires the llm-ollama feature.");
    eprintln!("Run with: cargo run --example minimal_agent --features llm-ollama");
}

#[cfg(feature = "llm-ollama")]
#[tokio::main]
async fn main() {
    use forgekit_agent::LlmConfig;
    use forgekit_agent::{chat::EventBus, chat::OllamaChatProvider, chat::TokenTracker, Agent};
    use std::sync::Arc;

    let provider = Arc::new(OllamaChatProvider::local());
    let config = LlmConfig::new("qwen3.5-agent:latest").with_temperature(0.0);

    let bus = EventBus::new();
    let tracker = TokenTracker::new();
    tracker.attach(&bus).await;

    let agent = Agent::new(".")
        .await
        .expect("failed to create agent")
        .with_chat_provider(provider, config)
        .with_event_bus(bus)
        .with_max_iterations(5);

    let result = agent
        .run_react("List the files in the current directory and describe the project structure.")
        .await
        .expect("agent failed");

    println!("Agent response:\n{result}");

    let usage = tracker.usage().await;
    println!(
        "\nToken usage: {} prompt, {} completion, {} total ({} LLM calls)",
        usage.prompt_tokens, usage.completion_tokens, usage.total_tokens, usage.llm_calls,
    );
}

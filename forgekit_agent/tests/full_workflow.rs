//! Full-agent workflow integration test.
//!
//! Tests real Agent with file_read, shell_exec, graph_query tools
//! against local Ollama models. Runs against all available models
//! from: qwen3.5, qwen3.5-agent, gemma4:e2b.
//!
//! Run with:
//!   cargo test --features llm-ollama --test full_workflow -- --nocapture

#[cfg(feature = "llm-ollama")]
mod workflow {
    use std::sync::{Arc, Mutex};

    use forgekit_agent::chat::{
        self, AgentEvent, EventBus, OllamaChatProvider, ReactStreamEvent, TokenTracker,
    };
    use forgekit_agent::Agent;
    use forgekit_agent::LlmConfig;

    const CANDIDATE_MODELS: &[&str] = &["qwen3.5:latest", "qwen3.5-agent:latest", "gemma4:e2b"];

    async fn ollama_available() -> bool {
        reqwest::Client::new()
            .get("http://localhost:11434/api/tags")
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    async fn available_models() -> Vec<String> {
        let Ok(resp) = reqwest::Client::new()
            .get("http://localhost:11434/api/tags")
            .send()
            .await
        else {
            return Vec::new();
        };
        let body = resp.text().await.unwrap_or_default();
        CANDIDATE_MODELS
            .iter()
            .filter(|m| {
                let base = m.split(':').next().expect("invariant: model name");
                body.contains(base)
            })
            .map(|m| m.to_string())
            .collect()
    }

    async fn make_agent(model: &str) -> Agent {
        Agent::new("/home/feanor/Projects/forge")
            .await
            .expect("agent creation failed")
            .with_chat_provider(
                Arc::new(OllamaChatProvider::local()),
                LlmConfig::new(model).with_temperature(0.1),
            )
            .with_max_iterations(10)
    }

    async fn log_tool_events(bus: &EventBus) -> Arc<Mutex<Vec<String>>> {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let log_clone = log.clone();
        bus.subscribe(move |event| {
            if let AgentEvent::ToolCallCompleted {
                tool_name, success, ..
            } = event
            {
                let status = if *success { "ok" } else { "FAIL" };
                log_clone
                    .lock()
                    .expect("log")
                    .push(format!("{tool_name}:{status}"));
            }
            if let AgentEvent::MaxIterationsReached { .. } = event {
                log_clone
                    .lock()
                    .expect("log")
                    .push("MaxIterationsReached".to_string());
            }
        })
        .await;
        log
    }

    #[tokio::test]
    async fn multi_model_read_file() {
        if !ollama_available().await {
            eprintln!("SKIP: Ollama not reachable");
            return;
        }
        let models = available_models().await;
        if models.is_empty() {
            eprintln!("SKIP: No test models found");
            return;
        }

        for model in &models {
            eprintln!("\n=== {} ===", model);
            let agent = make_agent(model).await;
            let result = agent
                .run_react(
                    "Read the file Cargo.toml and list all workspace members. \
                     Reply with just the member names separated by commas.",
                )
                .await;

            match result {
                Ok(text) => {
                    assert!(!text.is_empty(), "[{model}] Answer should not be empty");
                    eprintln!("  OK: {text}");
                }
                Err(e) => panic!("[{model}] FAILED: {e}"),
            }
        }
    }

    #[tokio::test]
    async fn multi_model_multi_step() {
        if !ollama_available().await {
            eprintln!("SKIP: Ollama not reachable");
            return;
        }
        let models = available_models().await;
        if models.is_empty() {
            eprintln!("SKIP: No test models found");
            return;
        }

        for model in &models {
            eprintln!("\n=== {} ===", model);
            let bus = EventBus::new();
            let log = log_tool_events(&bus).await;
            let agent = make_agent(model).await.with_event_bus(bus);

            let result = agent
                .run_react(
                    "Do the following in order:\n\
                     1. Use shell_exec to run 'wc -l forgekit_agent/src/chat/react.rs'.\n\
                     2. Read forgekit_agent/src/lib.rs using file_read.\n\
                     3. Tell me the line count of react.rs and the first line of lib.rs. \
                        Reply in 2 sentences.",
                )
                .await;

            let tools = log.lock().expect("log").clone();
            match result {
                Ok(text) => {
                    eprintln!("  ANSWER: {text}");
                    eprintln!("  TOOLS: {tools:?}");
                    let tool_ok = tools.iter().filter(|t| t.ends_with(":ok")).count();
                    let tool_fail = tools.iter().filter(|t| t.ends_with(":FAIL")).count();
                    assert_eq!(tool_fail, 0, "[{model}] Tool calls failed: {tools:?}");
                    if tool_ok > 0 {
                        assert!(
                            tool_ok >= 2,
                            "[{model}] If tools were called, should have >= 2, got {tool_ok}"
                        );
                    }
                    assert!(!text.is_empty(), "[{model}] Answer should not be empty");
                    assert!(
                        !tools.iter().any(|t| t == "MaxIterationsReached"),
                        "[{model}] Hit max iterations"
                    );
                }
                Err(e) => {
                    eprintln!("  TOOLS: {tools:?}");
                    panic!("[{model}] FAILED: {e}");
                }
            }
        }
    }

    #[tokio::test]
    async fn multi_model_streaming() {
        if !ollama_available().await {
            eprintln!("SKIP: Ollama not reachable");
            return;
        }
        let models = available_models().await;
        if models.is_empty() {
            eprintln!("SKIP: No test models found");
            return;
        }

        for model in &models {
            eprintln!("\n=== {} ===", model);
            let agent = make_agent(model).await;
            let stream = agent
                .run_react_stream("Read Cargo.toml and list the workspace members. Reply briefly.")
                .await
                .expect("stream creation failed");

            use futures::StreamExt;

            let mut events: Vec<ReactStreamEvent> = Vec::new();
            let mut stream = Box::pin(stream);
            while let Some(event) = stream.next().await {
                match &event {
                    ReactStreamEvent::LlmEvent(chat::StreamEvent::Token(t)) => eprint!("{t}"),
                    ReactStreamEvent::ToolExecuted { name, success, .. } => {
                        eprintln!("\n  [TOOL: {name} ok={success}]");
                    }
                    ReactStreamEvent::Answer(a) => eprintln!("\n  [ANSWER: {a}]"),
                    _ => {}
                }
                events.push(event);
            }

            let has_answer = events
                .iter()
                .any(|e| matches!(e, ReactStreamEvent::Answer(_)));
            let tool_count = events
                .iter()
                .filter(|e| matches!(e, ReactStreamEvent::ToolExecuted { .. }))
                .count();
            eprintln!(
                "  RESULT: {} events, {tool_count} tools, answer={has_answer}",
                events.len()
            );
            assert!(has_answer, "[{model}] Stream should end with Answer");
        }
    }

    #[tokio::test]
    async fn multi_model_observability() {
        if !ollama_available().await {
            eprintln!("SKIP: Ollama not reachable");
            return;
        }
        let models = available_models().await;
        if models.is_empty() {
            eprintln!("SKIP: No test models found");
            return;
        }

        for model in &models {
            eprintln!("\n=== {} ===", model);
            let bus = EventBus::new();
            let tracker = TokenTracker::new();
            tracker.attach(&bus).await;

            let event_log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
            let log_clone = event_log.clone();
            bus.subscribe(move |event| {
                let name = match event {
                    AgentEvent::SessionStarted { .. } => "SessionStarted",
                    AgentEvent::ToolCallStarted { .. } => "ToolCallStarted",
                    AgentEvent::ToolCallCompleted { success, .. } => {
                        if *success {
                            "ToolCallCompleted:ok"
                        } else {
                            "ToolCallCompleted:FAIL"
                        }
                    }
                    AgentEvent::AnswerProduced { .. } => "AnswerProduced",
                    AgentEvent::MaxIterationsReached { .. } => "MaxIterationsReached",
                    _ => return,
                };
                log_clone.lock().expect("log").push(name.to_string());
            })
            .await;

            let agent = make_agent(model).await.with_event_bus(bus);
            let result = agent
                .run_react("Read Cargo.toml and tell me the workspace members. One short sentence.")
                .await;

            match result {
                Ok(text) => {
                    let usage = tracker.usage().await;
                    let events = event_log.lock().expect("log");
                    eprintln!("  OK: {text}");
                    eprintln!("  EVENTS: {:?}", *events);
                    eprintln!(
                        "  TOKENS: llm_calls={}, prompt={:?}, completion={:?}",
                        usage.llm_calls, usage.prompt_tokens, usage.completion_tokens
                    );
                    assert!(
                        events.contains(&"SessionStarted".to_string()),
                        "[{model}] Should have SessionStarted"
                    );
                    assert!(
                        events.contains(&"AnswerProduced".to_string()),
                        "[{model}] Should have AnswerProduced"
                    );
                    assert!(usage.llm_calls > 0, "[{model}] Should have LLM calls");
                }
                Err(e) => panic!("[{model}] FAILED: {e}"),
            }
        }
    }

    #[tokio::test]
    async fn multi_model_verifier() {
        if !ollama_available().await {
            eprintln!("SKIP: Ollama not reachable");
            return;
        }
        let models = available_models().await;
        if models.is_empty() {
            eprintln!("SKIP: No test models found");
            return;
        }

        for model in &models {
            eprintln!("\n=== {} ===", model);
            let verifier: chat::VerifierFn =
                Arc::new(|answer| answer.contains("forge") || answer.contains("core"));

            let bus = EventBus::new();
            let event_log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
            let log_clone = event_log.clone();
            bus.subscribe(move |event| match event {
                AgentEvent::VerificationFailed { .. } => {
                    log_clone
                        .lock()
                        .expect("log")
                        .push("VerificationFailed".to_string());
                }
                AgentEvent::AnswerProduced { .. } => {
                    log_clone
                        .lock()
                        .expect("log")
                        .push("AnswerProduced".to_string());
                }
                _ => {}
            })
            .await;

            let agent = make_agent(model)
                .await
                .with_verifier(verifier)
                .with_event_bus(bus);

            let result = agent
                .run_react(
                    "Read Cargo.toml and tell me about the workspace. \
                     Make sure your answer mentions forge.",
                )
                .await;

            match result {
                Ok(text) => {
                    let events = event_log.lock().expect("log");
                    eprintln!("  OK: {text}");
                    eprintln!("  EVENTS: {:?}", *events);
                    assert!(
                        events.contains(&"AnswerProduced".to_string()),
                        "[{model}] Should produce answer"
                    );
                }
                Err(e) => panic!("[{model}] FAILED: {e}"),
            }
        }
    }
}

#[cfg(not(feature = "llm-ollama"))]
#[test]
fn full_workflow_feature_not_enabled() {
    eprintln!("llm-ollama feature not enabled; skipping full workflow tests");
}

use std::sync::Arc;

use crate::chat::providers::mock::MockChatProvider;
use crate::llm::LlmConfig;
use crate::orchestrate::{OrchestrateResult, Orchestrator};
use crate::Agent;

async fn make_agent(temp: &tempfile::TempDir, answer: &str) -> Agent {
    let provider = Arc::new(MockChatProvider::from_text(answer));
    let config = LlmConfig::new("test-model");
    Agent::new(temp.path())
        .await
        .unwrap()
        .with_chat_provider(provider, config)
}

#[tokio::test]
async fn sequential_chain_passes_results() {
    let temp = tempfile::tempdir().unwrap();
    let agent1 = make_agent(&temp, "step 1 done").await;
    let agent2 = make_agent(&temp, "step 2 done").await;

    let orchestrator = Orchestrator::new().add_agent(agent1).add_agent(agent2);

    let results = orchestrator.run_sequential("start").await;
    assert!(results.is_ok());
    let outputs = results.unwrap();
    assert_eq!(outputs.len(), 2);
    assert_eq!(outputs[0].result(), "step 1 done");
    assert_eq!(outputs[1].result(), "step 2 done");
}

#[tokio::test]
async fn parallel_fan_out_collects_all() {
    let temp = tempfile::tempdir().unwrap();
    let agent1 = make_agent(&temp, "answer A").await;
    let agent2 = make_agent(&temp, "answer B").await;
    let agent3 = make_agent(&temp, "answer C").await;

    let orchestrator = Orchestrator::new()
        .add_agent(agent1)
        .add_agent(agent2)
        .add_agent(agent3);

    let results = orchestrator.run_parallel("query").await;
    assert!(results.is_ok());
    let outputs = results.unwrap();
    assert_eq!(outputs.len(), 3);
    let answers: Vec<&str> = outputs.iter().map(|r| r.result()).collect();
    assert!(answers.contains(&"answer A"));
    assert!(answers.contains(&"answer B"));
    assert!(answers.contains(&"answer C"));
}

#[tokio::test]
async fn sequential_stops_on_error() {
    let temp = tempfile::tempdir().unwrap();
    let agent_no_provider = Agent::new(temp.path()).await.unwrap();
    let agent2 = make_agent(&temp, "should not run").await;

    let orchestrator = Orchestrator::new()
        .add_agent(agent_no_provider)
        .add_agent(agent2);

    let results = orchestrator.run_sequential("test").await;
    assert!(results.is_err());
}

#[tokio::test]
async fn parallel_collects_partial_results_on_error() {
    let temp = tempfile::tempdir().unwrap();
    let agent_ok = make_agent(&temp, "good result").await;
    let agent_no_provider = Agent::new(temp.path()).await.unwrap();

    let orchestrator = Orchestrator::new()
        .add_agent(agent_ok)
        .add_agent(agent_no_provider);

    let results = orchestrator.run_parallel_allow_partial("test").await;
    assert_eq!(results.len(), 2);
    let successes: Vec<_> = results.iter().filter(|r| r.is_success()).collect();
    let failures: Vec<_> = results.iter().filter(|r| !r.is_success()).collect();
    assert_eq!(successes.len(), 1);
    assert_eq!(failures.len(), 1);
    assert_eq!(successes[0].result(), "good result");
}

#[tokio::test]
async fn empty_orchestrator_returns_empty() {
    let orchestrator: Orchestrator = Orchestrator::new();
    let results = orchestrator.run_sequential("test").await;
    assert!(results.unwrap().is_empty());

    let orchestrator: Orchestrator = Orchestrator::new();
    let results = orchestrator.run_parallel("test").await;
    assert!(results.unwrap().is_empty());
}

#[tokio::test]
async fn single_agent_orchestration() {
    let temp = tempfile::tempdir().unwrap();
    let agent = make_agent(&temp, "solo answer").await;

    let orchestrator = Orchestrator::new().add_agent(agent);
    let results = orchestrator.run_sequential("test").await;
    assert_eq!(results.unwrap().len(), 1);
}

#[test]
fn orchestrate_result_accessors() {
    let ok = OrchestrateResult::success("agent-1", "hello".to_string());
    assert!(ok.is_success());
    assert_eq!(ok.agent_id(), "agent-1");
    assert_eq!(ok.result(), "hello");

    let err = OrchestrateResult::failure("agent-2", "failed".to_string());
    assert!(!err.is_success());
    assert_eq!(err.agent_id(), "agent-2");
    assert_eq!(err.error().unwrap(), "failed");
}

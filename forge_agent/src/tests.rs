use super::*;
use crate::chat::AsyncTool;
use std::sync::Arc;

#[tokio::test]
async fn test_agent_creation() {
    let temp = tempfile::tempdir().unwrap();
    let agent = Agent::new(temp.path()).await.unwrap();

    assert_eq!(agent.codebase_path, temp.path());
}

#[tokio::test]
async fn test_agent_with_runtime() {
    let temp = tempfile::tempdir().unwrap();
    let (_agent, runtime) = Agent::with_runtime(temp.path()).await.unwrap();

    assert_eq!(runtime.codebase_path(), temp.path());
}

#[tokio::test]
async fn test_agent_runtime_stats() {
    let temp = tempfile::tempdir().unwrap();
    let (_agent, runtime) = Agent::with_runtime(temp.path()).await.unwrap();

    let stats = runtime.stats();
    assert!(!stats.watch_active);
}

#[tokio::test]
async fn test_agent_backward_compatibility() {
    let temp = tempfile::tempdir().unwrap();
    let agent = Agent::new(temp.path()).await.unwrap();

    assert_eq!(agent.codebase_path, temp.path());
}

#[tokio::test]
async fn test_agent_with_llm_provider() {
    let temp = tempfile::tempdir().unwrap();
    let mock = std::sync::Arc::new(llm::MockProvider::new("mocked LLM response"));
    let agent = Agent::new(temp.path()).await.unwrap().with_llm(mock);

    assert!(agent.llm.is_some());
}

#[tokio::test]
async fn test_agent_without_llm_provider() {
    let temp = tempfile::tempdir().unwrap();
    let agent = Agent::new(temp.path()).await.unwrap();

    assert!(agent.llm.is_none());
}

#[cfg(feature = "envoy")]
#[tokio::test]
async fn test_agent_with_envoy() {
    let temp = tempfile::tempdir().unwrap();
    let config = envoy::EnvoyConfig {
        url: "http://localhost:9999".to_string(),
        agent_name: "test-forge".to_string(),
    };
    let client = envoy::EnvoyClient::new(config);
    let agent = Agent::new(temp.path()).await.unwrap().with_envoy(client);

    assert!(agent.envoy.is_some());
}

#[cfg(feature = "envoy")]
#[tokio::test]
async fn test_agent_without_envoy() {
    let temp = tempfile::tempdir().unwrap();
    let agent = Agent::new(temp.path()).await.unwrap();

    assert!(agent.envoy.is_none());
}

#[tokio::test]
async fn test_agent_run_workflow_passes_forge() {
    use crate::workflow::dag::Workflow;
    use crate::workflow::task::{TaskContext, TaskError, TaskId, TaskResult, WorkflowTask};
    use async_trait::async_trait;

    struct ForgeCheckTask;
    #[async_trait]
    impl WorkflowTask for ForgeCheckTask {
        async fn execute(&self, ctx: &TaskContext) -> std::result::Result<TaskResult, TaskError> {
            if ctx.forge.is_some() {
                Ok(TaskResult::Success)
            } else {
                Err(TaskError::ExecutionFailed("no forge".to_string()))
            }
        }
        fn id(&self) -> TaskId {
            TaskId::new("forge-check")
        }
        fn name(&self) -> &str {
            "ForgeCheckTask"
        }
    }

    let temp = tempfile::tempdir().unwrap();
    let agent = Agent::new(temp.path()).await.unwrap();

    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(ForgeCheckTask));

    let result = agent.run_workflow(workflow).await;
    assert!(result.is_ok(), "run_workflow failed: {:?}", result.err());
    assert!(result.unwrap().success);
}

#[tokio::test]
async fn agent_config_from_forge_toml() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join(".forge.toml"),
        "[agent]\nmax_iterations = 25\nstep_retries = 5\nretrieval_top_k = 15\n",
    )
    .unwrap();
    let agent = Agent::new(temp.path()).await.unwrap();
    assert_eq!(agent.max_iterations, 25);
    assert_eq!(agent.step_retries, 5);
    assert_eq!(agent.retrieval_top_k, 15);
}

#[tokio::test]
async fn agent_config_system_prompt_override() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join(".forge.toml"),
        "[agent]\nsystem_prompt = \"Custom prompt.\"\n",
    )
    .unwrap();
    let agent = Agent::new(temp.path()).await.unwrap();
    let prompt = agent.build_system_prompt();
    assert!(prompt.contains("Custom prompt."));
    assert!(prompt.contains("autonomous coding agent"));
}

#[tokio::test]
async fn agent_config_defaults_without_file() {
    let temp = tempfile::tempdir().unwrap();
    let agent = Agent::new(temp.path()).await.unwrap();
    assert_eq!(agent.max_iterations, 10);
    assert_eq!(agent.step_retries, 2);
    assert_eq!(agent.retrieval_top_k, 5);
    assert!(agent.custom_system_prompt.is_none());
}

#[tokio::test]
async fn agent_config_tool_allowlist() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join(".forge.toml"),
        "[agent]\ntools = [\"file_read\"]\n",
    )
    .unwrap();
    let agent = Agent::new(temp.path()).await.unwrap();
    let registry = agent.build_tool_registry();
    use chat::ToolRegistry;
    assert!(registry.has_tool("file_read"));
    assert!(!registry.has_tool("file_write"));
    assert!(!registry.has_tool("shell_exec"));
}

#[tokio::test]
async fn test_agent_run_workflow_passes_forge_continued() {
    use crate::workflow::dag::Workflow;
    use crate::workflow::task::{TaskContext, TaskError, TaskId, TaskResult, WorkflowTask};
    use async_trait::async_trait;

    struct ForgeCheckTask2;
    #[async_trait]
    impl WorkflowTask for ForgeCheckTask2 {
        async fn execute(&self, ctx: &TaskContext) -> std::result::Result<TaskResult, TaskError> {
            if ctx.forge.is_some() {
                Ok(TaskResult::Success)
            } else {
                Err(TaskError::ExecutionFailed("no forge".to_string()))
            }
        }
        fn id(&self) -> TaskId {
            TaskId::new("forge-check-2")
        }
        fn name(&self) -> &str {
            "ForgeCheckTask2"
        }
    }

    let temp = tempfile::tempdir().unwrap();
    let agent = Agent::new(temp.path()).await.unwrap();

    let mut workflow = Workflow::new();
    workflow.add_task(Box::new(ForgeCheckTask2));

    let result = agent.run_workflow(workflow).await;
    assert!(result.is_ok(), "run_workflow failed: {:?}", result.err());
    assert!(result.unwrap().success);
}

#[cfg(feature = "llm-ollama")]
#[tokio::test]
async fn test_llm_config_loaded_from_forge_toml() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join(".forge.toml"),
        "[llm]\nprovider = \"ollama\"\nmodel = \"llama3\"\nurl = \"http://localhost:11434\"\n",
    )
    .unwrap();
    let agent = Agent::new(temp.path()).await.unwrap();
    assert!(
        agent.llm.is_some(),
        "LLM provider should be loaded from .forge.toml"
    );
}

#[cfg(feature = "envoy")]
#[tokio::test]
async fn test_envoy_client_implements_discovery_store() {
    let config = envoy::EnvoyConfig {
        url: "http://localhost:9999".to_string(),
        agent_name: "test-forge".to_string(),
    };
    let client = std::sync::Arc::new(envoy::EnvoyClient::new(config));
    let store: std::sync::Arc<dyn crate::agent_loop::DiscoveryStore> = client.clone();
    store
        .store(
            "Symbol",
            "test_symbol",
            serde_json::json!({"file": "test.rs"}),
        )
        .await;
}

#[tokio::test]
async fn test_run_react_without_provider_errors() {
    let temp = tempfile::tempdir().unwrap();
    let agent = Agent::new(temp.path()).await.unwrap();

    let result = agent.run_react("do something").await;
    assert!(result.is_err());
    match result.unwrap_err() {
        AgentError::ReActFailed(msg) => {
            assert!(msg.contains("no ChatProvider"));
        }
        other => panic!("expected ReActFailed, got {other}"),
    }
}

#[tokio::test]
async fn test_with_chat_provider_configures_agent() {
    let temp = tempfile::tempdir().unwrap();
    let provider = std::sync::Arc::new(chat::MockChatProvider::from_text("hello"));
    let config = llm::LlmConfig::new("test-model");

    let agent = Agent::new(temp.path())
        .await
        .unwrap()
        .with_chat_provider(provider, config);

    assert!(agent.chat_provider.is_some());
    assert!(agent.chat_config.is_some());
}

#[tokio::test]
async fn test_run_react_returns_answer() {
    let temp = tempfile::tempdir().unwrap();

    let provider = std::sync::Arc::new(chat::MockChatProvider::from_text("The answer is 42"));
    let config = llm::LlmConfig::new("test-model");

    let agent = Agent::new(temp.path())
        .await
        .unwrap()
        .with_chat_provider(provider, config);

    let answer = agent.run_react("What is the answer?").await;
    assert!(answer.is_ok(), "run_react failed: {:?}", answer.err());
    assert_eq!(answer.unwrap(), "The answer is 42");
}

#[tokio::test]
async fn test_run_react_tool_call_then_answer() {
    let temp = tempfile::tempdir().unwrap();

    let provider = std::sync::Arc::new(
        chat::MockChatProvider::from_text("The file contains rust code")
            .with_tool_call("file_read", serde_json::json!({"path": "test.txt"})),
    );
    let config = llm::LlmConfig::new("test-model");

    std::fs::write(temp.path().join("test.txt"), "hello from test").unwrap();

    let agent = Agent::new(temp.path())
        .await
        .unwrap()
        .with_chat_provider(provider, config);

    let answer = agent.run_react("Read test.txt").await;
    assert!(answer.is_ok(), "run_react failed: {:?}", answer.err());
    assert_eq!(answer.unwrap(), "The file contains rust code");
}

#[tokio::test]
async fn test_agent_with_hooks() {
    let temp = tempfile::tempdir().unwrap();

    let mut hook_config = chat::HookConfig::empty();
    hook_config.add_group(
        chat::HookEvent::PreToolUse,
        chat::HookGroup {
            matcher: Some("shell_exec".to_string()),
            hooks: vec![chat::HookSpec {
                hook_type: "command".to_string(),
                command: "exit 0".to_string(),
                timeout: Some(5),
                status_message: None,
            }],
        },
    );

    let provider = std::sync::Arc::new(chat::MockChatProvider::from_text("done"));
    let config = llm::LlmConfig::new("test-model");

    let agent = Agent::new(temp.path())
        .await
        .unwrap()
        .with_chat_provider(provider, config)
        .with_hooks(hook_config);

    assert!(agent.hook_config.is_some());
}

#[tokio::test]
async fn test_agent_with_skill_registry() {
    let temp = tempfile::tempdir().unwrap();

    let skill_dir = temp.path().join(".forge").join("skills").join("test-skill");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "# Test Skill\n\nA test skill.\nTriggers: testing, agent",
    )
    .unwrap();

    let loader =
        chat::SkillLoader::with_search_paths(vec![temp.path().join(".forge").join("skills")]);
    let registry = std::sync::Arc::new(chat::SkillRegistry::new(loader));

    let provider = std::sync::Arc::new(chat::MockChatProvider::from_text("answer"));
    let config = llm::LlmConfig::new("test-model");

    let agent = Agent::new(temp.path())
        .await
        .unwrap()
        .with_chat_provider(provider, config)
        .with_skill_registry(registry);

    assert!(agent.skill_registry.is_some());
    let base_prompt = agent.build_system_prompt();
    assert!(
        !base_prompt.contains("skill tool"),
        "base prompt should not mention skill tool"
    );
}

#[tokio::test]
async fn test_agent_build_tool_registry_includes_skills() {
    use crate::chat::tools::registry::ToolRegistry;

    let temp = tempfile::tempdir().unwrap();

    let skill_dir = temp.path().join(".forge").join("skills").join("my-skill");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "# My Skill\n\nA skill.\nTriggers: coding",
    )
    .unwrap();

    let loader = chat::SkillLoader::new(Some(temp.path()));
    let registry = std::sync::Arc::new(chat::SkillRegistry::new(loader));

    let agent = Agent::new(temp.path())
        .await
        .unwrap()
        .with_skill_registry(registry);

    let reg = agent.build_tool_registry();
    assert!(reg.has_tool("skill"));
}

#[tokio::test]
async fn test_agent_build_tool_registry_without_skills() {
    use crate::chat::tools::registry::ToolRegistry;

    let temp = tempfile::tempdir().unwrap();
    let agent = Agent::new(temp.path()).await.unwrap();
    let reg = agent.build_tool_registry();
    assert!(!reg.has_tool("skill"));
}

#[tokio::test]
async fn test_build_system_prompt_for_query_injects_matched_skill() {
    let temp = tempfile::tempdir().unwrap();

    let debug_dir = temp.path().join(".forge").join("skills").join("debugging");
    std::fs::create_dir_all(&debug_dir).unwrap();
    std::fs::write(
        debug_dir.join("SKILL.md"),
        "---\nname: debugging\ndescription: \"Find root cause before proposing fixes\"\n---\n# Debugging\n\nTriggers: bug, test failure, unexpected behavior\n\nFind the root cause.",
    )
    .unwrap();

    let planning_dir = temp.path().join(".forge").join("skills").join("planning");
    std::fs::create_dir_all(&planning_dir).unwrap();
    std::fs::write(
        planning_dir.join("SKILL.md"),
        "---\nname: planning\ndescription: \"Plan a feature or refactor\"\n---\n# Planning\n\nTriggers: plan, feature, refactor\n\nPlan the change.",
    )
    .unwrap();

    let loader =
        chat::SkillLoader::with_search_paths(vec![temp.path().join(".forge").join("skills")]);
    let registry = std::sync::Arc::new(chat::SkillRegistry::new(loader));

    let agent = Agent::new(temp.path())
        .await
        .unwrap()
        .with_skill_registry(registry);

    let prompt = agent
        .build_system_prompt_for_query("fix the bug in react.rs")
        .await;
    assert!(
        prompt.contains("debugging"),
        "prompt should contain debugging skill, got: {}",
        &prompt[prompt.len().saturating_sub(200)..]
    );
    assert!(
        prompt.contains("Auto-loaded Skills"),
        "prompt should contain auto-loaded header"
    );
    assert!(
        !prompt.contains("planning"),
        "prompt should NOT contain planning skill for a bug query"
    );
}

#[tokio::test]
async fn test_build_system_prompt_for_query_no_match_returns_base() {
    let temp = tempfile::tempdir().unwrap();

    let skill_dir = temp.path().join(".forge").join("skills").join("obscure");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: obscure\ndescription: \"Does zyxwvu things\"\n---\n# Obscure\n\nTriggers: zyxwvu",
    )
    .unwrap();

    let loader =
        chat::SkillLoader::with_search_paths(vec![temp.path().join(".forge").join("skills")]);
    let registry = std::sync::Arc::new(chat::SkillRegistry::new(loader));

    let agent = Agent::new(temp.path())
        .await
        .unwrap()
        .with_skill_registry(registry);

    let prompt = agent
        .build_system_prompt_for_query("write a poem about cats")
        .await;
    assert!(
        !prompt.contains("Auto-loaded Skills"),
        "unrelated query should not trigger skill injection"
    );
    assert!(
        prompt.contains("autonomous coding agent"),
        "base prompt should still be present"
    );
}

#[tokio::test]
async fn test_build_system_prompt_for_query_custom_prompt_appends_skills() {
    let temp = tempfile::tempdir().unwrap();

    let skill_dir = temp.path().join(".forge").join("skills").join("test-skill");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: test-skill\ndescription: \"A test skill for testing\"\n---\n# Test\n\nTriggers: test, agent, skill",
    )
    .unwrap();

    let loader =
        chat::SkillLoader::with_search_paths(vec![temp.path().join(".forge").join("skills")]);
    let registry = std::sync::Arc::new(chat::SkillRegistry::new(loader));

    std::fs::write(
        temp.path().join(".forge.toml"),
        "[agent]\nsystem_prompt = \"You are a test assistant.\"\n",
    )
    .unwrap();

    let agent = Agent::new(temp.path())
        .await
        .unwrap()
        .with_skill_registry(registry);

    assert!(agent.custom_system_prompt.is_some());
    let prompt = agent
        .build_system_prompt_for_query("test this skill agent")
        .await;
    assert!(
        prompt.contains("test assistant"),
        "custom prompt should be in base"
    );
    assert!(
        prompt.contains("Auto-loaded Skills"),
        "skills should still be appended to custom prompt"
    );
}

#[tokio::test]
async fn test_build_system_prompt_for_query_without_registry() {
    let temp = tempfile::tempdir().unwrap();
    let agent = Agent::new(temp.path()).await.unwrap();

    let prompt = agent.build_system_prompt_for_query("fix the bug").await;
    assert!(
        prompt.contains("autonomous coding agent"),
        "base prompt should be present"
    );
    assert!(
        !prompt.contains("Auto-loaded Skills"),
        "no registry means no skill injection"
    );
}

#[tokio::test]
async fn test_agent_with_verifier_rejects_answer() {
    let temp = tempfile::tempdir().unwrap();
    let provider = std::sync::Arc::new(
        chat::MockChatProvider::from_text("bad answer").with_text("good magic answer"),
    );
    let config = llm::LlmConfig::new("test-model");

    let agent = Agent::new(temp.path())
        .await
        .unwrap()
        .with_chat_provider(provider, config)
        .with_verifier(std::sync::Arc::new(|answer: &str| answer.contains("magic")));

    let result = agent.run_react("test").await;
    assert_eq!(result.unwrap(), "good magic answer");
}

#[tokio::test]
async fn test_agent_with_max_iterations() {
    let temp = tempfile::tempdir().unwrap();
    let provider = std::sync::Arc::new(
        chat::MockChatProvider::from_text("never finish")
            .with_tool_call("echo", serde_json::json!({"msg": "loop"}))
            .with_tool_call("echo", serde_json::json!({"msg": "loop"})),
    );
    let config = llm::LlmConfig::new("test-model");

    let agent = Agent::new(temp.path())
        .await
        .unwrap()
        .with_chat_provider(provider, config)
        .with_max_iterations(2);

    let result = agent.run_react("loop").await;
    match result.unwrap_err() {
        AgentError::ReActFailed(msg) => assert!(msg.contains("maximum iterations")),
        other => panic!("expected ReActFailed with max iterations, got {other}"),
    }
}

#[tokio::test]
async fn test_agent_with_step_retries_zero_propagates_error() {
    let temp = tempfile::tempdir().unwrap();
    let provider = std::sync::Arc::new(
        chat::MockChatProvider::from_text("ok")
            .with_error(crate::chat::types::LlmError::Http("fail".to_string())),
    );
    let config = llm::LlmConfig::new("test-model");

    let agent = Agent::new(temp.path())
        .await
        .unwrap()
        .with_chat_provider(provider, config)
        .with_step_retries(0);

    let result = agent.run_react("test").await;
    match result.unwrap_err() {
        AgentError::ReActFailed(msg) => assert!(msg.contains("fail")),
        other => panic!("expected ReActFailed with provider error, got {other}"),
    }
}

#[tokio::test]
async fn test_agent_with_retriever() {
    use crate::chat::retrieval::{CodeRetriever, CodeSnippet, RetrievalSource};

    struct TestRetriever;
    #[async_trait::async_trait]
    impl CodeRetriever for TestRetriever {
        async fn retrieve(&self, _query: &str, _top_k: usize) -> Vec<CodeSnippet> {
            vec![CodeSnippet {
                file: std::path::PathBuf::from("test.rs"),
                line: 1,
                content: "fn test() {}".to_string(),
                score: 0.9,
                source: RetrievalSource::File,
            }]
        }
    }

    let temp = tempfile::tempdir().unwrap();
    let provider = std::sync::Arc::new(chat::MockChatProvider::from_text("retrieved answer"));
    let config = llm::LlmConfig::new("test-model");

    let agent = Agent::new(temp.path())
        .await
        .unwrap()
        .with_chat_provider(provider, config)
        .with_retriever(std::sync::Arc::new(TestRetriever));

    let result = agent.run_react("find test").await;
    assert_eq!(result.unwrap(), "retrieved answer");
}

#[tokio::test]
async fn test_agent_spawn_runs_concurrently() {
    let temp = tempfile::tempdir().unwrap();
    let provider = std::sync::Arc::new(chat::MockChatProvider::from_text("spawned result"));
    let config = llm::LlmConfig::new("test-model");

    let agent = Agent::new(temp.path())
        .await
        .unwrap()
        .with_chat_provider(provider, config);

    let task = agent.spawn("test query").await.unwrap();
    let result = task.await;
    assert_eq!(result.unwrap(), "spawned result");
}

#[tokio::test]
async fn test_agent_spawn_without_provider_errors() {
    let temp = tempfile::tempdir().unwrap();
    let agent = Agent::new(temp.path()).await.unwrap();

    let result = agent.spawn("test").await;
    match result.unwrap_err() {
        AgentError::ReActFailed(msg) => assert!(msg.contains("no ChatProvider")),
        other => panic!("expected ReActFailed, got {other}"),
    }
}

#[tokio::test]
async fn event_bus_captures_lifecycle() {
    use chat::{AgentEvent, EventBus};
    use std::sync::Mutex;

    let bus = EventBus::new();
    let events: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let events_clone = events.clone();
    bus.subscribe(move |event| {
        let label = match event {
            AgentEvent::SessionStarted { .. } => "session_started",
            AgentEvent::IterationStarted { .. } => "iteration_started",
            AgentEvent::LlmResponseReceived { .. } => "llm_response",
            AgentEvent::ToolCallStarted { .. } => "tool_started",
            AgentEvent::ToolCallCompleted { .. } => "tool_completed",
            AgentEvent::AnswerProduced { .. } => "answer",
            _ => "other",
        };
        events_clone.lock().unwrap().push(label.to_string());
    })
    .await;

    let temp = tempfile::tempdir().unwrap();
    let provider = std::sync::Arc::new(chat::MockChatProvider::from_text("done"));
    let config = crate::llm::LlmConfig::new("test");

    let agent = Agent::new(temp.path())
        .await
        .unwrap()
        .with_chat_provider(provider, config)
        .with_event_bus(bus);

    let result = agent.run_react("hello").await.unwrap();
    assert_eq!(result, "done");

    let captured = events.lock().unwrap();
    assert!(captured.contains(&"session_started".to_string()));
    assert!(captured.contains(&"iteration_started".to_string()));
    assert!(captured.contains(&"llm_response".to_string()));
    assert!(captured.contains(&"answer".to_string()));
}

#[tokio::test]
async fn agent_config_tool_denylist() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join(".forge.toml"),
        "[agent]\ntools = [\"file_read\", \"shell_exec\"]\ndenied_tools = [\"shell_exec\"]\n",
    )
    .unwrap();
    let agent = Agent::new(temp.path()).await.unwrap();
    let registry = agent.build_tool_registry();
    use chat::ToolRegistry;
    assert!(registry.has_tool("file_read"));
    assert!(!registry.has_tool("shell_exec"));
    assert!(!registry.has_tool("file_write"));
}

#[tokio::test]
async fn sandbox_blocks_shell_command() {
    use chat::sandbox::Sandbox;
    let sandbox = Sandbox::new().with_blocked_commands(&["rm\\s+-rf".to_string()]);
    let shared = chat::sandbox::shared_sandbox(Some(sandbox));
    let temp = tempfile::tempdir().unwrap();
    let tool = chat::ShellExecTool::new(temp.path()).with_sandbox(shared);
    let result = tool.call(serde_json::json!({"command": "rm -rf /"})).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("blocked by sandbox policy"));
}

#[tokio::test]
async fn sandbox_allows_safe_command() {
    use chat::sandbox::Sandbox;
    let sandbox = Sandbox::new().with_blocked_commands(&["sudo".to_string()]);
    let shared = chat::sandbox::shared_sandbox(Some(sandbox));
    let temp = tempfile::tempdir().unwrap();
    let tool = chat::ShellExecTool::new(temp.path()).with_sandbox(shared);
    let result = tool
        .call(serde_json::json!({"command": "echo hello"}))
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn sandbox_blocks_file_path() {
    use chat::sandbox::Sandbox;
    let sandbox = Sandbox::new().with_blocked_paths(&["\\.env".to_string()]);
    let shared = chat::sandbox::shared_sandbox(Some(sandbox));
    let temp = tempfile::tempdir().unwrap();
    let tool = chat::FileReadTool::new(temp.path()).with_sandbox(shared);
    let result = tool.call(serde_json::json!({"path": ".env"})).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("blocked by sandbox policy"));
}

#[tokio::test]
async fn sandbox_allows_safe_path() {
    use chat::sandbox::Sandbox;
    let sandbox = Sandbox::new().with_blocked_paths(&["\\.env".to_string()]);
    let shared = chat::sandbox::shared_sandbox(Some(sandbox));
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("main.rs"), "fn main() {}").unwrap();
    let tool = chat::FileReadTool::new(temp.path()).with_sandbox(shared);
    let result = tool.call(serde_json::json!({"path": "main.rs"})).await;
    assert!(result.is_ok());
    assert!(result.unwrap().contains("fn main()"));
}

mod builder_tests {
    use super::*;
    use crate::chat::providers::MockChatProvider;
    use crate::llm::LlmConfig;

    fn test_llm_config() -> LlmConfig {
        LlmConfig {
            model: "test-model".to_string(),
            temperature: Some(0.7),
            max_tokens: Some(4096),
            top_p: None,
            stop: Vec::new(),
            json_mode: false,
            max_tool_output_bytes: 8192,
        }
    }

    #[tokio::test]
    async fn builder_produces_agent_with_provider() {
        let temp = tempfile::tempdir().unwrap();
        let provider = Arc::new(MockChatProvider::from_text("hello"));
        let agent = agent_builder(temp.path())
            .chat_provider(provider, test_llm_config())
            .build()
            .await
            .unwrap();

        assert!(agent.chat_provider.is_some());
        assert!(agent.chat_config.is_some());
        assert_eq!(agent.codebase_path, temp.path());
    }

    #[tokio::test]
    async fn builder_applies_optional_config() {
        let temp = tempfile::tempdir().unwrap();
        let provider = Arc::new(MockChatProvider::from_text("hello"));
        let agent = agent_builder(temp.path())
            .chat_provider(provider, test_llm_config())
            .max_iterations(42)
            .step_retries(5)
            .retrieval_top_k(20)
            .system_prompt("You are a test.")
            .build()
            .await
            .unwrap();

        assert_eq!(agent.max_iterations, 42);
        assert_eq!(agent.step_retries, 5);
        assert_eq!(agent.retrieval_top_k, 20);
        assert_eq!(
            agent.custom_system_prompt.as_deref(),
            Some("You are a test.")
        );
    }

    #[tokio::test]
    async fn builder_defaults_match_agent_new() {
        let temp = tempfile::tempdir().unwrap();
        let bare = Agent::new(temp.path()).await.unwrap();
        let provider = Arc::new(MockChatProvider::from_text("hello"));
        let built = agent_builder(temp.path())
            .chat_provider(provider, test_llm_config())
            .build()
            .await
            .unwrap();

        assert_eq!(built.max_iterations, bare.max_iterations);
        assert_eq!(built.step_retries, bare.step_retries);
        assert_eq!(built.retrieval_top_k, bare.retrieval_top_k);
    }

    #[tokio::test]
    async fn builder_via_agent_method() {
        let temp = tempfile::tempdir().unwrap();
        let provider = Arc::new(MockChatProvider::from_text("hello"));
        let agent = Agent::builder(temp.path())
            .chat_provider(provider, test_llm_config())
            .build()
            .await
            .unwrap();

        assert!(agent.chat_provider.is_some());
    }

    #[tokio::test]
    async fn builder_with_hooks_and_verifier() {
        let temp = tempfile::tempdir().unwrap();
        let provider = Arc::new(MockChatProvider::from_text("hello"));
        let hooks = chat::HookConfig::default();
        let verifier: chat::VerifierFn = Arc::new(|_answer: &str| true);

        let agent = agent_builder(temp.path())
            .chat_provider(provider, test_llm_config())
            .hooks(hooks)
            .verifier(verifier)
            .build()
            .await
            .unwrap();

        assert!(agent.hook_config.is_some());
        assert!(agent.verifier.is_some());
    }
}

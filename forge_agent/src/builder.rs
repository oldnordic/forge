use crate::chat::{self, ChatProvider};
use crate::llm;
use crate::policy::Policy;
use crate::Result;
use std::path::PathBuf;
use std::sync::Arc;

pub struct NeedsProvider;

pub struct Ready {
    chat_provider: Arc<dyn ChatProvider>,
    chat_config: llm::LlmConfig,
}

pub struct AgentBuilder<State> {
    codebase_path: PathBuf,
    max_iterations: usize,
    step_retries: usize,
    retrieval_top_k: usize,
    hook_config: Option<chat::HookConfig>,
    skill_registry: Option<Arc<chat::SkillRegistry>>,
    verifier: Option<chat::VerifierFn>,
    retriever: Option<Arc<dyn chat::CodeRetriever>>,
    event_bus: Option<chat::EventBus>,
    policies: Vec<Policy>,
    custom_system_prompt: Option<String>,
    state: State,
}

impl AgentBuilder<NeedsProvider> {
    pub fn chat_provider(
        self,
        provider: Arc<dyn ChatProvider>,
        config: llm::LlmConfig,
    ) -> AgentBuilder<Ready> {
        AgentBuilder {
            codebase_path: self.codebase_path,
            max_iterations: self.max_iterations,
            step_retries: self.step_retries,
            retrieval_top_k: self.retrieval_top_k,
            hook_config: self.hook_config,
            skill_registry: self.skill_registry,
            verifier: self.verifier,
            retriever: self.retriever,
            event_bus: self.event_bus,
            policies: self.policies,
            custom_system_prompt: self.custom_system_prompt,
            state: Ready {
                chat_provider: provider,
                chat_config: config,
            },
        }
    }
}

impl<State> AgentBuilder<State> {
    pub fn max_iterations(mut self, n: usize) -> Self {
        self.max_iterations = n;
        self
    }

    pub fn step_retries(mut self, n: usize) -> Self {
        self.step_retries = n;
        self
    }

    pub fn retrieval_top_k(mut self, k: usize) -> Self {
        self.retrieval_top_k = k;
        self
    }

    pub fn hooks(mut self, config: chat::HookConfig) -> Self {
        self.hook_config = Some(config);
        self
    }

    pub fn skills(mut self, registry: Arc<chat::SkillRegistry>) -> Self {
        self.skill_registry = Some(registry);
        self
    }

    pub fn verifier(mut self, v: chat::VerifierFn) -> Self {
        self.verifier = Some(v);
        self
    }

    pub fn retriever(mut self, r: Arc<dyn chat::CodeRetriever>) -> Self {
        self.retriever = Some(r);
        self
    }

    pub fn event_bus(mut self, bus: chat::EventBus) -> Self {
        self.event_bus = Some(bus);
        self
    }

    pub fn policies(mut self, policies: Vec<Policy>) -> Self {
        self.policies = policies;
        self
    }

    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.custom_system_prompt = Some(prompt.into());
        self
    }
}

impl AgentBuilder<Ready> {
    pub async fn build(self) -> Result<super::Agent> {
        let mut agent = super::Agent::new(&self.codebase_path).await?;

        agent.chat_provider = Some(self.state.chat_provider);
        agent.chat_config = Some(self.state.chat_config);
        agent.max_iterations = self.max_iterations;
        agent.step_retries = self.step_retries;
        agent.retrieval_top_k = self.retrieval_top_k;
        agent.hook_config = self.hook_config;
        agent.skill_registry = self.skill_registry;
        agent.verifier = self.verifier;
        agent.retriever = self.retriever;
        agent.event_bus = self.event_bus;
        agent.policies = self.policies;
        agent.custom_system_prompt = self.custom_system_prompt;

        Ok(agent)
    }
}

pub fn agent_builder(codebase_path: impl AsRef<std::path::Path>) -> AgentBuilder<NeedsProvider> {
    AgentBuilder {
        codebase_path: codebase_path.as_ref().to_path_buf(),
        max_iterations: 10,
        step_retries: 2,
        retrieval_top_k: 5,
        hook_config: None,
        skill_registry: None,
        verifier: None,
        retriever: None,
        event_bus: None,
        policies: Vec::new(),
        custom_system_prompt: None,
        state: NeedsProvider,
    }
}

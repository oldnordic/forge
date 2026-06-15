//! Core agent implementation.
//!
//! The `Agent` struct is the main entry point for AI-driven code operations.
//! It supports two execution modes:
//!
//! - **6-phase pipeline** (`run()`): Observe → Constrain → Plan → Mutate → Verify → Commit
//! - **ReAct loop** (`run_react()`): LLM-driven autonomous reasoning and tool-calling

use std::path::PathBuf;

use crate::agent_config::AgentConfig;
use crate::chat;
#[cfg(feature = "envoy")]
use crate::envoy;
#[cfg(feature = "envoy")]
use crate::evidence;
use crate::llm;
use crate::planner;
use crate::policy;
use crate::{
    agent_loop, commit, mutate, observe, verify, workflow, AgentError, AgentTask, CommitResult,
    ConstrainedPlan, ExecutionPlan, LoopResult, MutationResult, Observation, Result,
    VerificationResult,
};

#[cfg(any(
    feature = "llm-ollama",
    feature = "llm-openai",
    feature = "llm-anthropic"
))]
fn load_llm_from_forge_toml(
    project_path: &std::path::Path,
) -> Option<std::sync::Arc<dyn llm::LlmProvider>> {
    #[derive(serde::Deserialize)]
    struct LlmSection {
        provider: String,
        model: String,
        #[cfg(any(feature = "llm-ollama", feature = "llm-openai"))]
        url: Option<String>,
        #[cfg(any(feature = "llm-openai", feature = "llm-anthropic"))]
        api_key: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct ForgeToml {
        llm: Option<LlmSection>,
    }
    let text = std::fs::read_to_string(project_path.join(".forge.toml")).ok()?;
    let parsed: ForgeToml = toml::from_str(&text).ok()?;
    let cfg = parsed.llm?;
    match cfg.provider.as_str() {
        #[cfg(feature = "llm-ollama")]
        "ollama" => {
            let endpoint = cfg
                .url
                .unwrap_or_else(|| "http://localhost:11434".to_string());
            Some(std::sync::Arc::new(llm::OllamaProvider::new(
                endpoint, cfg.model,
            )))
        }
        #[cfg(feature = "llm-openai")]
        "openai" => {
            let api_key = cfg.api_key?;
            let url = cfg
                .url
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
            Some(std::sync::Arc::new(llm::OpenAiProvider::new(
                url, cfg.model, api_key,
            )))
        }
        #[cfg(feature = "llm-anthropic")]
        "anthropic" => {
            let api_key = cfg.api_key?;
            Some(std::sync::Arc::new(llm::AnthropicProvider::new(
                cfg.model, api_key,
            )))
        }
        _ => None,
    }
}

#[cfg(not(any(
    feature = "llm-ollama",
    feature = "llm-openai",
    feature = "llm-anthropic"
)))]
fn load_llm_from_forge_toml(
    _project_path: &std::path::Path,
) -> Option<std::sync::Arc<dyn llm::LlmProvider>> {
    None
}

/// Agent for deterministic AI-driven code operations.
///
/// The agent follows a strict loop:
/// 1. Observe: Gather context from the graph
/// 2. Constrain: Apply policy rules
/// 3. Plan: Generate execution steps
/// 4. Mutate: Apply changes
/// 5. Verify: Validate results
/// 6. Commit: Finalize transaction
///
/// # Runtime Integration
///
/// The agent can integrate with `ForgeRuntime` for coordinated file watching
/// and caching:
///
/// ```ignore
/// let (agent, mut runtime) = Agent::with_runtime("./project").await?;
/// let result = agent.run_with_runtime(&mut runtime, "refactor function").await?;
/// ```
///
pub struct Agent {
    /// Path to the codebase
    pub(crate) codebase_path: PathBuf,
    /// Forge SDK instance for graph queries
    pub(crate) forge: Option<forgekit_core::Forge>,
    /// Optional LLM provider for semantic operations
    pub(crate) llm: Option<std::sync::Arc<dyn llm::LlmProvider>>,
    /// Optional chat provider for ReAct agent loop
    pub(crate) chat_provider: Option<std::sync::Arc<dyn chat::ChatProvider>>,
    /// Chat config (model, temperature, etc.) for ReAct loop
    pub(crate) chat_config: Option<llm::LlmConfig>,
    /// Optional envoy client for multi-agent coordination
    #[cfg(feature = "envoy")]
    pub(crate) envoy: Option<std::sync::Arc<envoy::EnvoyClient>>,
    /// Optional evidence recording session
    #[cfg(feature = "envoy")]
    pub(crate) session: Option<std::sync::Arc<evidence::ForgeSession>>,
    /// Policies enforced during the constraint phase
    pub(crate) policies: Vec<policy::Policy>,
    /// Hook configuration for lifecycle events
    pub(crate) hook_config: Option<chat::HookConfig>,
    /// Skill registry for loading skills
    pub(crate) skill_registry: Option<std::sync::Arc<chat::SkillRegistry>>,
    /// Optional verifier for validating final answers
    pub(crate) verifier: Option<chat::VerifierFn>,
    /// Optional code retriever for RAG-augmented context
    pub(crate) retriever: Option<std::sync::Arc<dyn chat::CodeRetriever>>,
    /// Number of retrieval results to inject (default 5)
    pub(crate) retrieval_top_k: usize,
    /// Max ReAct loop iterations (default 10)
    pub(crate) max_iterations: usize,
    /// Max consecutive LLM errors before failing (default 2)
    pub(crate) step_retries: usize,
    /// Agent config loaded from .forge.toml [agent] section
    pub(crate) agent_config: Option<AgentConfig>,
    /// Custom system prompt from config (overrides build_system_prompt)
    pub(crate) custom_system_prompt: Option<String>,
    /// Event bus for lifecycle observability
    pub(crate) event_bus: Option<chat::EventBus>,
}

impl Agent {
    /// Returns a type-state builder that requires a chat provider before building.
    pub fn builder(
        codebase_path: impl AsRef<std::path::Path>,
    ) -> crate::builder::AgentBuilder<crate::builder::NeedsProvider> {
        crate::builder::agent_builder(codebase_path)
    }

    /// Creates a new agent for the given codebase.
    pub async fn new(codebase_path: impl AsRef<std::path::Path>) -> Result<Self> {
        let path = codebase_path.as_ref().to_path_buf();

        let forge = forgekit_core::Forge::open(&path).await.ok();

        let llm = load_llm_from_forge_toml(&path);

        #[cfg(feature = "envoy")]
        let envoy = {
            let config_path = path.join(".forge.toml");
            envoy::EnvoyConfig::from_file(&config_path)
                .ok()
                .flatten()
                .map(|c| std::sync::Arc::new(envoy::EnvoyClient::new(c)))
        };

        #[cfg(feature = "envoy")]
        let project_name = path
            .file_name()
            .unwrap_or_default()
            .to_str()
            .unwrap_or("unknown")
            .to_string();

        #[cfg(feature = "envoy")]
        let session: Option<std::sync::Arc<evidence::ForgeSession>> = envoy.as_ref().map(|c| {
            std::sync::Arc::new(evidence::ForgeSession::new(
                c.clone() as std::sync::Arc<dyn evidence::EvidenceRecorder>,
                &project_name,
                "forge",
                None,
            ))
        });

        let agent_config = AgentConfig::from_file(&path.join(".forge.toml"))
            .ok()
            .flatten();
        let (max_iterations, step_retries, retrieval_top_k, custom_system_prompt) =
            match &agent_config {
                Some(cfg) => (
                    cfg.max_iterations(),
                    cfg.step_retries(),
                    cfg.retrieval_top_k(),
                    cfg.system_prompt.clone(),
                ),
                None => (10, 2, 5, None),
            };

        Ok(Self {
            codebase_path: path,
            forge,
            llm,
            chat_provider: None,
            chat_config: None,
            #[cfg(feature = "envoy")]
            envoy,
            #[cfg(feature = "envoy")]
            session,
            policies: Vec::new(),
            hook_config: None,
            skill_registry: None,
            verifier: None,
            retriever: None,
            retrieval_top_k,
            max_iterations,
            step_retries,
            agent_config,
            custom_system_prompt,
            event_bus: None,
        })
    }

    pub fn with_llm(mut self, provider: std::sync::Arc<dyn llm::LlmProvider>) -> Self {
        self.llm = Some(provider);
        self
    }

    pub fn with_policies(mut self, policies: Vec<policy::Policy>) -> Self {
        self.policies = policies;
        self
    }

    pub fn with_chat_provider(
        mut self,
        provider: std::sync::Arc<dyn chat::ChatProvider>,
        config: llm::LlmConfig,
    ) -> Self {
        self.chat_provider = Some(provider);
        self.chat_config = Some(config);
        self
    }

    #[cfg(feature = "envoy")]
    pub fn with_envoy(mut self, client: envoy::EnvoyClient) -> Self {
        self.envoy = Some(std::sync::Arc::new(client));
        self
    }

    pub fn with_hooks(mut self, config: chat::HookConfig) -> Self {
        self.hook_config = Some(config);
        self
    }

    pub fn with_skill_registry(mut self, registry: std::sync::Arc<chat::SkillRegistry>) -> Self {
        self.skill_registry = Some(registry);
        self
    }

    pub fn with_verifier(mut self, verifier: chat::VerifierFn) -> Self {
        self.verifier = Some(verifier);
        self
    }

    pub fn with_retriever(mut self, retriever: std::sync::Arc<dyn chat::CodeRetriever>) -> Self {
        self.retriever = Some(retriever);
        self
    }

    pub fn with_retrieval_top_k(mut self, k: usize) -> Self {
        self.retrieval_top_k = k;
        self
    }

    pub fn with_max_iterations(mut self, n: usize) -> Self {
        self.max_iterations = n;
        self
    }

    pub fn with_step_retries(mut self, n: usize) -> Self {
        self.step_retries = n;
        self
    }

    pub fn with_event_bus(mut self, bus: chat::EventBus) -> Self {
        self.event_bus = Some(bus);
        self
    }

    #[cfg(feature = "envoy")]
    pub async fn connect_envoy(&self) -> std::result::Result<String, String> {
        let client = self
            .envoy
            .as_ref()
            .ok_or_else(|| "envoy not configured".to_string())?;
        client.register().await
    }

    pub async fn observe(&self, query: &str) -> Result<Observation> {
        let forge = self
            .forge
            .as_ref()
            .ok_or_else(|| AgentError::ObservationFailed("Forge SDK not available".to_string()))?;

        let mut observer = observe::Observer::new(forge.clone());
        if let Some(ref llm) = self.llm {
            observer = observer.with_llm(llm.clone());
        }
        #[cfg(feature = "envoy")]
        if let Some(ref envoy) = self.envoy {
            observer = observer.with_knowledge_source(envoy.clone());
        }
        let obs = observer.gather(query).await?;

        Ok(obs)
    }

    pub async fn constrain(
        &self,
        observation: Observation,
        policies: Vec<policy::Policy>,
    ) -> Result<ConstrainedPlan> {
        let forge = self.forge.as_ref().ok_or_else(|| {
            AgentError::ObservationFailed(
                "Forge SDK not available for policy validation".to_string(),
            )
        })?;

        let validator = policy::PolicyValidator::new(forge.clone());

        let diff = policy::Diff {
            file_path: std::path::PathBuf::from(&observation.query),
            original: String::new(),
            modified: format!("query: {}", observation.query),
            changes: Vec::new(),
        };

        let report = validator.validate(&diff, &policies).await?;

        Ok(ConstrainedPlan {
            observation,
            policy_violations: report.violations,
        })
    }

    pub async fn plan(&self, constrained: ConstrainedPlan) -> Result<ExecutionPlan> {
        let mut planner_instance = planner::Planner::new();
        if let Some(ref llm) = self.llm {
            planner_instance = planner_instance.with_llm(llm.clone());
        }

        let obs = observe::Observation {
            query: constrained.observation.query.clone(),
            symbols: constrained.observation.symbols.clone(),
            summary: constrained.observation.summary.clone(),
        };

        let steps = planner_instance.generate_steps(&obs).await?;
        let impact = planner_instance.estimate_impact(&steps).await?;
        let conflicts = planner_instance.detect_conflicts(&steps)?;

        if !conflicts.is_empty() {
            let details: Vec<String> = conflicts
                .iter()
                .map(|c| match &c.reason {
                    planner::ConflictReason::OverlappingRegion { start, end } => {
                        format!("{} at lines {}-{}", c.file, start, end)
                    }
                })
                .collect();
            return Err(AgentError::PlanningFailed(format!(
                "Found {} conflicts in plan: {}",
                conflicts.len(),
                details.join("; ")
            )));
        }

        let mut ordered_steps = steps;
        planner_instance.order_steps(&mut ordered_steps)?;

        let rollback = planner_instance.generate_rollback(&ordered_steps);

        Ok(ExecutionPlan {
            steps: ordered_steps,
            estimated_impact: planner::ImpactEstimate {
                affected_files: impact.affected_files,
                complexity: impact.complexity,
            },
            rollback_plan: rollback,
        })
    }

    pub async fn mutate(&self, plan: ExecutionPlan) -> Result<MutationResult> {
        self.forge
            .as_ref()
            .ok_or_else(|| AgentError::MutationFailed("Forge SDK not available".to_string()))?;

        let mut mutator = mutate::Mutator::new();
        mutator.begin_transaction().await?;

        let mut modified_files = Vec::new();
        for step in &plan.steps {
            mutator.apply_step(step).await?;
            if let Some(path) = step_affected_file(step) {
                modified_files.push(std::path::PathBuf::from(path));
            }
        }

        let diffs: Vec<String> = modified_files
            .iter()
            .map(|f| f.to_string_lossy().to_string())
            .collect();

        Ok(MutationResult {
            modified_files,
            diffs,
        })
    }

    pub async fn verify(&self, result: MutationResult) -> Result<VerificationResult> {
        let verifier = verify::Verifier::new();
        let report = if result.modified_files.is_empty() {
            verifier.verify(&self.codebase_path).await?
        } else {
            verifier
                .verify_changes(&self.codebase_path, &result.modified_files, &result.diffs)
                .await?
        };

        Ok(VerificationResult {
            passed: report.passed,
            diagnostics: report
                .diagnostics
                .iter()
                .map(|d| d.message.clone())
                .collect(),
            suggestions: report.suggestions,
        })
    }

    pub async fn commit(&self, result: VerificationResult) -> Result<CommitResult> {
        let committer = commit::Committer::new();
        let files: Vec<std::path::PathBuf> = result
            .diagnostics
            .iter()
            .filter_map(|d| {
                d.split(':')
                    .next()
                    .map(|s| std::path::PathBuf::from(s.trim()))
            })
            .collect();

        let message = format!("forge: apply changes ({} files)", files.len());
        let commit_report = committer
            .finalize(&self.codebase_path, &files, &message)
            .await?;

        Ok(CommitResult {
            transaction_id: commit_report.transaction_id,
            files_committed: commit_report.files_committed,
            git_committed: commit_report.git_committed,
        })
    }

    pub async fn run(&self, query: &str) -> Result<LoopResult> {
        let forge = self
            .forge
            .as_ref()
            .ok_or_else(|| AgentError::ObservationFailed("Forge SDK not available".to_string()))?;

        let mut agent_loop = agent_loop::AgentLoop::new(std::sync::Arc::new(forge.clone()));

        if let Some(ref llm) = self.llm {
            agent_loop = agent_loop.with_llm(llm.clone());
        }

        #[cfg(feature = "envoy")]
        if let Some(ref envoy) = self.envoy {
            agent_loop = agent_loop.with_discovery_store(envoy.clone());
        }

        #[cfg(feature = "envoy")]
        if let Some(ref session) = self.session {
            agent_loop = agent_loop.with_session(session.clone());
        }

        if !self.policies.is_empty() {
            agent_loop = agent_loop.with_policies(self.policies.clone());
        }

        agent_loop.run(query).await
    }

    pub async fn run_react(&self, query: &str) -> Result<String> {
        let provider = self.chat_provider.as_ref().ok_or_else(|| {
            AgentError::ReActFailed(
                "no ChatProvider configured; use with_chat_provider()".to_string(),
            )
        })?;
        let config = self.resolve_chat_config()?;

        let registry = self.build_tool_registry();
        let system_prompt = self.build_system_prompt_for_query(query).await;

        let mut react = chat::ReActLoop::new(std::sync::Arc::clone(provider), registry, config)
            .with_system_prompt(system_prompt)
            .with_max_iterations(self.max_iterations)
            .with_step_retries(self.step_retries);

        if let Some(ref hook_config) = self.hook_config {
            react = react.with_hooks(chat::HookRunner::new(hook_config.clone()));
        }

        if let Some(ref verifier) = self.verifier {
            react = react.with_verifier(std::sync::Arc::clone(verifier));
        }

        if let Some(ref retriever) = self.retriever {
            react = react
                .with_retriever(std::sync::Arc::clone(retriever))
                .with_retrieval_top_k(self.retrieval_top_k);
        }

        if let Some(ref bus) = self.event_bus {
            react = react.with_event_bus(bus.clone());
        }

        react
            .run(query)
            .await
            .map_err(|e| AgentError::ReActFailed(format!("{e}")))
    }

    pub async fn run_react_stream(
        &self,
        query: impl Into<String>,
    ) -> Result<std::pin::Pin<Box<dyn futures::Stream<Item = chat::ReactStreamEvent> + Send>>> {
        let query_str = query.into();
        let provider = self.chat_provider.as_ref().ok_or_else(|| {
            AgentError::ReActFailed(
                "no ChatProvider configured; use with_chat_provider()".to_string(),
            )
        })?;
        let config = self.resolve_chat_config()?;

        let registry = self.build_tool_registry();
        let system_prompt = self.build_system_prompt_for_query(&query_str).await;

        let mut react = chat::ReActLoop::new(std::sync::Arc::clone(provider), registry, config)
            .with_system_prompt(system_prompt)
            .with_max_iterations(self.max_iterations)
            .with_step_retries(self.step_retries);

        if let Some(ref hook_config) = self.hook_config {
            react = react.with_hooks(chat::HookRunner::new(hook_config.clone()));
        }

        if let Some(ref verifier) = self.verifier {
            react = react.with_verifier(std::sync::Arc::clone(verifier));
        }

        if let Some(ref retriever) = self.retriever {
            react = react
                .with_retriever(std::sync::Arc::clone(retriever))
                .with_retrieval_top_k(self.retrieval_top_k);
        }

        if let Some(ref bus) = self.event_bus {
            react = react.with_event_bus(bus.clone());
        }

        Ok(react.run_stream(&query_str))
    }

    pub async fn spawn(
        self,
        query: impl Into<String>,
    ) -> std::result::Result<AgentTask, AgentError> {
        let provider = self.chat_provider.as_ref().ok_or_else(|| {
            AgentError::ReActFailed(
                "no ChatProvider configured; use with_chat_provider()".to_string(),
            )
        })?;
        let config = self.resolve_chat_config()?;

        let registry = self.build_tool_registry();
        let query = query.into();
        let system_prompt = self.build_system_prompt_for_query(&query).await;

        let mut react = chat::ReActLoop::new(std::sync::Arc::clone(provider), registry, config)
            .with_system_prompt(system_prompt)
            .with_max_iterations(self.max_iterations)
            .with_step_retries(self.step_retries);

        if let Some(ref hook_config) = self.hook_config {
            react = react.with_hooks(chat::HookRunner::new(hook_config.clone()));
        }

        if let Some(verifier) = self.verifier {
            react = react.with_verifier(verifier);
        }

        if let Some(ref retriever) = self.retriever {
            react = react
                .with_retriever(std::sync::Arc::clone(retriever))
                .with_retrieval_top_k(self.retrieval_top_k);
        }

        if let Some(ref bus) = self.event_bus {
            react = react.with_event_bus(bus.clone());
        }

        let handle =
            tokio::spawn(async move { react.run(&query).await.map_err(|e| format!("{e}")) });

        Ok(AgentTask { handle })
    }

    /// Returns the effective `LlmConfig` for ReAct execution.
    ///
    /// Values from `.forge.toml` `[agent]` section (`temperature`,
    /// `max_tokens`) are applied as defaults — they only fill in fields
    /// the caller left as `None` in the explicit `LlmConfig` passed to
    /// `with_chat_provider`. Programmatic configuration always wins.
    fn resolve_chat_config(&self) -> Result<llm::LlmConfig> {
        let mut config = self
            .chat_config
            .as_ref()
            .ok_or_else(|| {
                AgentError::ReActFailed(
                    "no LlmConfig configured; use with_chat_provider()".to_string(),
                )
            })?
            .clone();

        if let Some(ref agent_config) = self.agent_config {
            if config.temperature.is_none() && agent_config.temperature.is_some() {
                config.temperature = agent_config.temperature;
            }
            if config.max_tokens.is_none() && agent_config.max_tokens.is_some() {
                config.max_tokens = agent_config.max_tokens;
            }
        }

        Ok(config)
    }

    pub(crate) fn build_tool_registry(&self) -> chat::BuiltinToolRegistry {
        let mut registry = chat::BuiltinToolRegistry::new();

        let needs_sandbox = self
            .agent_config
            .as_ref()
            .is_some_and(|c| c.blocked_commands.is_some() || c.blocked_paths.is_some());

        if needs_sandbox {
            let sandbox = self
                .agent_config
                .as_ref()
                .map(chat::Sandbox::from_config)
                .unwrap_or_default();
            let shared = chat::sandbox::shared_sandbox(Some(sandbox));

            match self.forge.as_ref() {
                Some(forge) => {
                    registry.register_many(chat::default_builtin_tools_with_graph_sandboxed(
                        &self.codebase_path,
                        forge.clone(),
                        shared,
                    ));
                }
                None => {
                    registry.register_many(chat::default_builtin_tools_sandboxed(
                        &self.codebase_path,
                        shared,
                    ));
                }
            }
        } else {
            match self.forge.as_ref() {
                Some(forge) => {
                    registry.register_many(chat::default_builtin_tools_with_graph(
                        &self.codebase_path,
                        forge.clone(),
                    ));
                }
                None => {
                    registry.register_many(chat::default_builtin_tools(&self.codebase_path));
                }
            }
        }

        if let Some(ref skill_reg) = self.skill_registry {
            registry.register(Box::new(chat::SkillTool::new(skill_reg.clone())));
        }

        #[cfg(feature = "atheneum")]
        {
            let db_path = self.codebase_path.join(".atheneum").join("atheneum.db");
            if db_path.exists() {
                let project_name = self
                    .codebase_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "forgekit-agent".to_string());
                registry.register(Box::new(chat::AtheneumTool::new(db_path, project_name)));
            }
        }

        #[cfg(feature = "envoy")]
        {
            if let Some(ref client) = self.envoy {
                registry.register(Box::new(chat::EnvoyTool::new(client.clone())));
            }
        }

        if let Some(ref config) = self.agent_config {
            registry.retain(|name| config.is_tool_allowed(name));
        }

        registry
    }

    pub(crate) fn build_system_prompt(&self) -> String {
        let mut parts = vec![
            "You are an autonomous coding agent.".to_string(),
            "You have tools to read files, write files, execute shell commands, \
             and query the code graph (find symbols, callers, references, cycles, impact analysis)."
                .to_string(),
        ];

        #[cfg(feature = "atheneum")]
        {
            let db_path = self.codebase_path.join(".atheneum").join("atheneum.db");
            if db_path.exists() {
                parts.push(
                    "You can query and store knowledge in the atheneum graph using the 'atheneum' tool."
                        .to_string(),
                );
            }
        }

        #[cfg(feature = "envoy")]
        {
            if self.envoy.is_some() {
                parts.push(
                    "You can coordinate with other agents using the 'envoy' tool \
                     (send messages, poll for messages, manage handoffs)."
                        .to_string(),
                );
            }
        }

        parts.push(format!(
            "Your workspace is: {}",
            self.codebase_path.display()
        ));

        if let Some(ref prompt) = self.custom_system_prompt {
            parts.push(prompt.clone());
        }

        parts.join(" ")
    }

    pub(crate) async fn build_system_prompt_for_query(&self, query: &str) -> String {
        let mut base = self.build_system_prompt();

        if let Some(ref registry) = self.skill_registry {
            let ranked = registry.rank_matching(query);
            if !ranked.is_empty() {
                let mut skill_parts = vec![format!(
                    "\n\n---\n# Auto-loaded Skills\n\nThe following skills were matched to your task. Follow their workflows.\n"
                )];

                let loaded = registry
                    .rank_and_load(query, 2, chat::MAX_INJECTED_BYTES)
                    .await;

                for content in &loaded {
                    skill_parts.push(content.system_prompt_fragment_bounded(
                        chat::MAX_INJECTED_BYTES / loaded.len().max(1),
                    ));
                    skill_parts.push("\n---\n".to_string());
                }

                if skill_parts.len() > 1 {
                    base.push_str(&skill_parts.join("\n"));
                }
            }
        }

        base
    }

    pub async fn run_workflow(
        &self,
        workflow: workflow::Workflow,
    ) -> Result<workflow::WorkflowResult> {
        let forge = self
            .forge
            .as_ref()
            .ok_or_else(|| AgentError::ObservationFailed("Forge SDK not available".to_string()))?;
        workflow::WorkflowExecutor::new(workflow)
            .with_forge(std::sync::Arc::new(forge.clone()))
            .execute()
            .await
            .map_err(|e| AgentError::WorkflowFailed(e.to_string()))
    }
}

/// Returns the file path affected by a plan step, if any.
///
/// Used by [`Agent::mutate`] to track which files were modified during the
/// mutation phase so that verification and commit receive accurate information.
fn step_affected_file(step: &planner::PlanStep) -> Option<&str> {
    use planner::PlanOperation;
    match &step.operation {
        PlanOperation::Rename { file, old, .. } => file.as_deref().or(Some(old.as_str())),
        PlanOperation::Delete { file, name, .. } => file.as_deref().or(Some(name.as_str())),
        PlanOperation::Create { path, .. } => Some(path.as_str()),
        PlanOperation::Modify { file, .. } => Some(file.as_str()),
        PlanOperation::Inspect { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_agent(
        chat_config: Option<llm::LlmConfig>,
        agent_config: Option<AgentConfig>,
    ) -> Agent {
        Agent {
            codebase_path: PathBuf::new(),
            forge: None,
            llm: None,
            chat_provider: None,
            chat_config,
            #[cfg(feature = "envoy")]
            envoy: None,
            #[cfg(feature = "envoy")]
            session: None,
            policies: Vec::new(),
            hook_config: None,
            skill_registry: None,
            verifier: None,
            retriever: None,
            retrieval_top_k: 5,
            max_iterations: 10,
            step_retries: 2,
            agent_config,
            custom_system_prompt: None,
            event_bus: None,
        }
    }

    #[test]
    fn test_resolve_chat_config_applies_agent_defaults() {
        // AgentConfig sets temperature/max_tokens, LlmConfig leaves them None.
        // The agent_config values should be applied as defaults.
        let agent = minimal_agent(
            Some(llm::LlmConfig::new("test-model")),
            Some(AgentConfig {
                temperature: Some(0.5),
                max_tokens: Some(2048),
                ..Default::default()
            }),
        );
        let config = agent.resolve_chat_config().unwrap();
        assert_eq!(config.temperature, Some(0.5));
        assert_eq!(config.max_tokens, Some(2048));
    }

    #[test]
    fn test_resolve_chat_config_explicit_config_wins() {
        // Both AgentConfig and LlmConfig set temperature/max_tokens.
        // The explicit LlmConfig values must win.
        let agent = minimal_agent(
            Some(llm::LlmConfig {
                temperature: Some(0.9),
                max_tokens: Some(8192),
                ..llm::LlmConfig::new("test-model")
            }),
            Some(AgentConfig {
                temperature: Some(0.5),
                max_tokens: Some(2048),
                ..Default::default()
            }),
        );
        let config = agent.resolve_chat_config().unwrap();
        assert_eq!(config.temperature, Some(0.9));
        assert_eq!(config.max_tokens, Some(8192));
    }

    #[test]
    fn test_resolve_chat_config_no_agent_config_unchanged() {
        // No AgentConfig — LlmConfig should pass through unmodified.
        let agent = minimal_agent(Some(llm::LlmConfig::new("test-model")), None);
        let config = agent.resolve_chat_config().unwrap();
        assert_eq!(config.temperature, None);
        assert_eq!(config.max_tokens, None);
    }

    #[test]
    fn test_resolve_chat_config_no_chat_config_errors() {
        // No chat_config at all — should return an error.
        let agent = minimal_agent(None, None);
        assert!(agent.resolve_chat_config().is_err());
    }
}

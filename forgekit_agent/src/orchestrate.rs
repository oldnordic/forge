use std::future::Future;
use std::pin::Pin;

use crate::Agent;
use crate::AgentError;

static AGENT_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn next_agent_id() -> String {
    let n = AGENT_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("agent-{n}")
}

#[derive(Debug)]
pub struct OrchestrateResult {
    agent_id: String,
    result: Option<String>,
    error: Option<String>,
}

impl OrchestrateResult {
    pub fn success(agent_id: impl Into<String>, result: String) -> Self {
        OrchestrateResult {
            agent_id: agent_id.into(),
            result: Some(result),
            error: None,
        }
    }

    pub fn failure(agent_id: impl Into<String>, error: String) -> Self {
        OrchestrateResult {
            agent_id: agent_id.into(),
            result: None,
            error: Some(error),
        }
    }

    pub fn is_success(&self) -> bool {
        self.error.is_none()
    }

    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    pub fn result(&self) -> &str {
        self.result.as_deref().unwrap_or("")
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

type AgentFuture =
    Pin<Box<dyn Future<Output = (String, std::result::Result<String, AgentError>)> + Send>>;

pub struct Orchestrator {
    agents: Vec<(String, Agent)>,
}

impl Orchestrator {
    pub fn new() -> Self {
        Orchestrator { agents: Vec::new() }
    }

    pub fn add_agent(mut self, agent: Agent) -> Self {
        let id = next_agent_id();
        self.agents.push((id, agent));
        self
    }

    pub fn add_agent_with_id(mut self, id: impl Into<String>, agent: Agent) -> Self {
        self.agents.push((id.into(), agent));
        self
    }

    pub async fn run_sequential(
        self,
        query: &str,
    ) -> std::result::Result<Vec<OrchestrateResult>, AgentError> {
        let mut results = Vec::new();
        let mut current_query = query.to_string();

        for (id, agent) in self.agents {
            match agent.run_react(&current_query).await {
                Ok(answer) => {
                    current_query = answer.clone();
                    results.push(OrchestrateResult::success(&id, answer));
                }
                Err(e) => {
                    results.push(OrchestrateResult::failure(&id, format!("{e}")));
                    return Err(e);
                }
            }
        }

        Ok(results)
    }

    pub async fn run_parallel(
        self,
        query: &str,
    ) -> std::result::Result<Vec<OrchestrateResult>, AgentError> {
        if self.agents.is_empty() {
            return Ok(Vec::new());
        }

        let mut handles: Vec<AgentFuture> = Vec::new();

        for (id, agent) in self.agents {
            let query = query.to_string();
            handles.push(Box::pin(async move {
                let result = agent.run_react(&query).await;
                (id, result)
            }));
        }

        let results = futures::future::join_all(handles).await;

        let mut outputs = Vec::new();
        for (id, result) in results {
            match result {
                Ok(answer) => outputs.push(OrchestrateResult::success(&id, answer)),
                Err(e) => {
                    let err_msg = format!("{e}");
                    outputs.push(OrchestrateResult::failure(&id, err_msg.clone()));
                    return Err(e);
                }
            }
        }

        Ok(outputs)
    }

    pub async fn run_parallel_allow_partial(self, query: &str) -> Vec<OrchestrateResult> {
        if self.agents.is_empty() {
            return Vec::new();
        }

        let mut handles: Vec<AgentFuture> = Vec::new();

        for (id, agent) in self.agents {
            let query = query.to_string();
            handles.push(Box::pin(async move {
                let result = agent.run_react(&query).await;
                (id, result)
            }));
        }

        let results = futures::future::join_all(handles).await;

        results
            .into_iter()
            .map(|(id, result)| match result {
                Ok(answer) => OrchestrateResult::success(&id, answer),
                Err(e) => OrchestrateResult::failure(&id, format!("{e}")),
            })
            .collect()
    }
}

impl Default for Orchestrator {
    fn default() -> Self {
        Self::new()
    }
}

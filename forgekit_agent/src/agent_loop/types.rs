use crate::audit::AuditEvent;
use async_trait::async_trait;

#[async_trait]
pub trait DiscoveryStore: Send + Sync {
    async fn store(&self, discovery_type: &str, target: &str, metadata: serde_json::Value);
}

#[derive(Clone, Debug, PartialEq)]
pub enum AgentPhase {
    Observe,
    Constrain,
    Plan,
    Mutate,
    Verify,
    Commit,
}

#[derive(Clone, Debug)]
pub struct AgentLoopCheckpoint {
    pub phase: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone, Debug)]
pub struct LoopResult {
    pub transaction_id: String,
    pub modified_files: Vec<std::path::PathBuf>,
    pub audit_events: Vec<AuditEvent>,
}

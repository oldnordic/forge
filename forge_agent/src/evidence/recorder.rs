use crate::evidence::types::*;
use async_trait::async_trait;

#[async_trait]
pub trait EvidenceRecorder: Send + Sync {
    async fn record_prompt(&self, session_id: &str, record: &PromptRecord);
    async fn record_tool_call(&self, session_id: &str, record: &ToolCallEvidence);
    async fn record_file_write(&self, session_id: &str, record: &FileWriteRecord);
    async fn record_commit(&self, session_id: &str, record: &CommitRecord);
    async fn record_test_run(&self, session_id: &str, record: &TestRunRecord);
    async fn record_fix_chain(&self, session_id: &str, record: &FixChainRecord);
}

#[cfg(feature = "envoy")]
#[async_trait]
impl EvidenceRecorder for crate::envoy::EnvoyClient {
    async fn record_prompt(&self, session_id: &str, record: &PromptRecord) {
        let _ = self.forge_prompt(session_id, record).await;
    }

    async fn record_tool_call(&self, session_id: &str, record: &ToolCallEvidence) {
        let _ = self.forge_tool_call(session_id, record).await;
    }

    async fn record_file_write(&self, session_id: &str, record: &FileWriteRecord) {
        let _ = self.forge_file_write(session_id, record).await;
    }

    async fn record_commit(&self, session_id: &str, record: &CommitRecord) {
        let _ = self.forge_commit(session_id, record).await;
    }

    async fn record_test_run(&self, session_id: &str, record: &TestRunRecord) {
        let _ = self.forge_test_run(session_id, record).await;
    }

    async fn record_fix_chain(&self, session_id: &str, record: &FixChainRecord) {
        let _ = self.forge_fix_chain(session_id, record).await;
    }
}

pub struct MockEvidenceRecorder {
    pub prompts: std::sync::Mutex<Vec<(String, PromptRecord)>>,
    pub tool_calls: std::sync::Mutex<Vec<(String, ToolCallEvidence)>>,
    pub file_writes: std::sync::Mutex<Vec<(String, FileWriteRecord)>>,
    pub commits: std::sync::Mutex<Vec<(String, CommitRecord)>>,
    pub test_runs: std::sync::Mutex<Vec<(String, TestRunRecord)>>,
    pub fix_chains: std::sync::Mutex<Vec<(String, FixChainRecord)>>,
}

impl Default for MockEvidenceRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl MockEvidenceRecorder {
    pub fn new() -> Self {
        Self {
            prompts: std::sync::Mutex::new(Vec::new()),
            tool_calls: std::sync::Mutex::new(Vec::new()),
            file_writes: std::sync::Mutex::new(Vec::new()),
            commits: std::sync::Mutex::new(Vec::new()),
            test_runs: std::sync::Mutex::new(Vec::new()),
            fix_chains: std::sync::Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl EvidenceRecorder for MockEvidenceRecorder {
    async fn record_prompt(&self, session_id: &str, record: &PromptRecord) {
        self.prompts
            .lock()
            .unwrap()
            .push((session_id.to_string(), record.clone()));
    }

    async fn record_tool_call(&self, session_id: &str, record: &ToolCallEvidence) {
        self.tool_calls
            .lock()
            .unwrap()
            .push((session_id.to_string(), record.clone()));
    }

    async fn record_file_write(&self, session_id: &str, record: &FileWriteRecord) {
        self.file_writes
            .lock()
            .unwrap()
            .push((session_id.to_string(), record.clone()));
    }

    async fn record_commit(&self, session_id: &str, record: &CommitRecord) {
        self.commits
            .lock()
            .unwrap()
            .push((session_id.to_string(), record.clone()));
    }

    async fn record_test_run(&self, session_id: &str, record: &TestRunRecord) {
        self.test_runs
            .lock()
            .unwrap()
            .push((session_id.to_string(), record.clone()));
    }

    async fn record_fix_chain(&self, session_id: &str, record: &FixChainRecord) {
        self.fix_chains
            .lock()
            .unwrap()
            .push((session_id.to_string(), record.clone()));
    }
}

use crate::evidence::recorder::EvidenceRecorder;
use crate::evidence::types::*;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionMetrics {
    pub session_id: String,
    pub project: String,
    pub tool_name: String,
    pub trigger: String,
    pub model: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub exit_status: Option<String>,
    pub git_branch: Option<String>,
    pub git_head: Option<String>,
    pub prompt_count: u32,
    pub tool_call_count: u32,
    pub file_write_count: u32,
    pub commit_count: u32,
    pub test_run_count: u32,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cost_usd: f64,
}

pub struct ForgeSession {
    recorder: Arc<dyn EvidenceRecorder>,
    metrics: Arc<std::sync::RwLock<SessionMetrics>>,
    prompt_sequence: AtomicU32,
    tool_call_count: AtomicU32,
    file_write_count: AtomicU32,
    commit_count: AtomicU32,
    test_run_count: AtomicU32,
    total_input_tokens: AtomicU64,
    total_output_tokens: AtomicU64,
    total_cost_usd: Arc<std::sync::Mutex<f64>>,
}

impl ForgeSession {
    pub fn new(
        recorder: Arc<dyn EvidenceRecorder>,
        project: &str,
        tool_name: &str,
        model: Option<&str>,
    ) -> Self {
        let session_id = uuid::Uuid::now_v7().to_string();
        let started_at = chrono::Utc::now().to_rfc3339();
        let git_branch = Self::current_git_branch();
        let git_head = Self::current_git_head();
        let model_value = model.map(String::from);

        let metrics = SessionMetrics {
            session_id: session_id.clone(),
            project: project.to_string(),
            tool_name: tool_name.to_string(),
            trigger: "cli".to_string(),
            model: model_value.clone(),
            started_at,
            ended_at: None,
            exit_status: None,
            git_branch,
            git_head,
            prompt_count: 0,
            tool_call_count: 0,
            file_write_count: 0,
            commit_count: 0,
            test_run_count: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cost_usd: 0.0,
        };

        let session = Self {
            recorder,
            metrics: Arc::new(std::sync::RwLock::new(metrics)),
            prompt_sequence: AtomicU32::new(0),
            tool_call_count: AtomicU32::new(0),
            file_write_count: AtomicU32::new(0),
            commit_count: AtomicU32::new(0),
            test_run_count: AtomicU32::new(0),
            total_input_tokens: AtomicU64::new(0),
            total_output_tokens: AtomicU64::new(0),
            total_cost_usd: Arc::new(std::sync::Mutex::new(0.0)),
        };

        let recorder = session.recorder.clone();
        let session_id_clone = session_id.clone();
        tokio::spawn(async move {
            let _ = recorder
                .record_prompt(
                    &session_id_clone,
                    &PromptRecord {
                        role: "system".into(),
                        sequence: 0,
                        input_hash: sha256_hex(&session_id_clone),
                        input_tokens: None,
                        output_hash: None,
                        output_tokens: None,
                        latency_ms: None,
                        model: model_value,
                        cost_usd: None,
                    },
                )
                .await;
        });

        session
    }

    pub fn session_id(&self) -> String {
        self.metrics.read().unwrap().session_id.clone()
    }

    pub async fn end(&self, exit_status: &str) {
        let mut m = self.metrics.write().unwrap();
        m.ended_at = Some(chrono::Utc::now().to_rfc3339());
        m.exit_status = Some(exit_status.to_string());
        m.prompt_count = self.prompt_sequence.load(Ordering::Relaxed);
        m.tool_call_count = self.tool_call_count.load(Ordering::Relaxed);
        m.file_write_count = self.file_write_count.load(Ordering::Relaxed);
        m.commit_count = self.commit_count.load(Ordering::Relaxed);
        m.test_run_count = self.test_run_count.load(Ordering::Relaxed);
        m.total_input_tokens = self.total_input_tokens.load(Ordering::Relaxed);
        m.total_output_tokens = self.total_output_tokens.load(Ordering::Relaxed);
        m.total_cost_usd = *self.total_cost_usd.lock().unwrap();
    }

    pub fn record_prompt(&self, record: PromptRecord) {
        let seq = self.prompt_sequence.fetch_add(1, Ordering::Relaxed);
        let mut r = record;
        r.sequence = seq;

        if let Some(tokens) = r.input_tokens {
            self.total_input_tokens.fetch_add(tokens, Ordering::Relaxed);
        }
        if let Some(tokens) = r.output_tokens {
            self.total_output_tokens
                .fetch_add(tokens, Ordering::Relaxed);
        }
        if let Some(cost) = r.cost_usd {
            *self.total_cost_usd.lock().unwrap() += cost;
        }

        let session_id = self.session_id();
        let recorder = self.recorder.clone();
        tokio::spawn(async move {
            recorder.record_prompt(&session_id, &r).await;
        });
    }

    pub fn record_tool_call(&self, record: ToolCallEvidence) {
        self.tool_call_count.fetch_add(1, Ordering::Relaxed);
        if let Some(tokens) = record.input_tokens_est {
            self.total_input_tokens.fetch_add(tokens, Ordering::Relaxed);
        }

        let session_id = self.session_id();
        let recorder = self.recorder.clone();
        tokio::spawn(async move {
            recorder.record_tool_call(&session_id, &record).await;
        });
    }

    pub fn record_file_write(&self, record: FileWriteRecord) {
        self.file_write_count.fetch_add(1, Ordering::Relaxed);

        let session_id = self.session_id();
        let recorder = self.recorder.clone();
        tokio::spawn(async move {
            recorder.record_file_write(&session_id, &record).await;
        });
    }

    pub fn record_commit(&self, record: CommitRecord) {
        self.commit_count.fetch_add(1, Ordering::Relaxed);

        let session_id = self.session_id();
        let recorder = self.recorder.clone();
        tokio::spawn(async move {
            recorder.record_commit(&session_id, &record).await;
        });
    }

    pub fn record_test_run(&self, record: TestRunRecord) {
        self.test_run_count.fetch_add(1, Ordering::Relaxed);

        let session_id = self.session_id();
        let recorder = self.recorder.clone();
        tokio::spawn(async move {
            recorder.record_test_run(&session_id, &record).await;
        });
    }

    pub fn record_fix_chain(&self, record: FixChainRecord) {
        let session_id = self.session_id();
        let recorder = self.recorder.clone();
        tokio::spawn(async move {
            recorder.record_fix_chain(&session_id, &record).await;
        });
    }

    fn current_git_branch() -> Option<String> {
        std::process::Command::new("git")
            .args(["branch", "--show-current"])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
    }

    fn current_git_head() -> Option<String> {
        std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
    }
}

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCategory {
    #[default]
    Other,
    GroundedQuery,
    FileRead,
    FileWrite,
    Test,
    Bench,
    Git,
    Shell,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitType {
    #[default]
    Feature,
    Fix,
    Refactor,
    Test,
    Docs,
    Release,
    Chore,
    Ci,
    Style,
    Merge,
}

impl CommitType {
    pub fn classify(msg: &str) -> Self {
        let lower = msg.to_ascii_lowercase();
        if lower.starts_with("feat") || lower.contains("feature") {
            Self::Feature
        } else if lower.starts_with("fix") || lower.contains("bug") {
            Self::Fix
        } else if lower.starts_with("refactor") {
            Self::Refactor
        } else if lower.starts_with("test") || lower.contains("bench") {
            Self::Test
        } else if lower.starts_with("docs") {
            Self::Docs
        } else if lower.starts_with("release")
            || (lower.contains("bump") && !lower.starts_with("chore"))
        {
            Self::Release
        } else if lower.starts_with("chore") {
            Self::Chore
        } else if lower.starts_with("ci") {
            Self::Ci
        } else if lower.starts_with("style") {
            Self::Style
        } else if lower.starts_with("merge") {
            Self::Merge
        } else {
            Self::Feature
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixType {
    #[default]
    CompileError,
    LogicBug,
    TestFailure,
    Crash,
    Deadlock,
    PerfRegression,
    Style,
    Doc,
    Ci,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Critical,
    High,
    #[default]
    Medium,
    Low,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PromptRecord {
    pub role: String,
    pub sequence: u32,
    pub input_hash: String,
    pub input_tokens: Option<u64>,
    pub output_hash: Option<String>,
    pub output_tokens: Option<u64>,
    pub latency_ms: Option<u64>,
    pub model: Option<String>,
    pub cost_usd: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ToolCallEvidence {
    pub tool_name: String,
    pub tool_version: Option<String>,
    pub input_hash: String,
    pub input_summary: String,
    pub output_hash: Option<String>,
    pub output_summary: Option<String>,
    pub exit_status: String,
    pub latency_ms: u64,
    pub input_tokens_est: Option<u64>,
    pub tool_category: ToolCategory,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FileWriteRecord {
    pub file_path: String,
    pub file_id: String,
    pub before_hash: Option<String>,
    pub after_hash: String,
    pub lines_added: u64,
    pub lines_deleted: u64,
    pub lines_changed: u64,
    pub write_type: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CommitRecord {
    pub commit_sha: String,
    pub parent_sha: Option<String>,
    pub message: String,
    pub author: String,
    pub files_changed: u64,
    pub lines_inserted: u64,
    pub lines_deleted: u64,
    pub commit_type: CommitType,
    pub feature_tag: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TestRunRecord {
    pub test_name: String,
    pub test_suite: Option<String>,
    pub test_command: String,
    pub result: String,
    pub duration_ms: u64,
    pub logs_summary: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BenchRunRecord {
    pub bench_name: String,
    pub bench_suite: Option<String>,
    pub mean_ns: Option<u64>,
    pub median_ns: Option<u64>,
    pub p95_ns: Option<u64>,
    pub iterations: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FixChainRecord {
    pub bug_commit_sha: String,
    pub fix_commit_sha: String,
    pub fix_type: FixType,
    pub severity: Severity,
    pub cycles_to_fix: u32,
    pub time_to_fix_ms: u64,
}

#[inline]
pub fn sha256_hex(data: impl AsRef<[u8]>) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data.as_ref());
    format!("{:x}", hasher.finalize())
}

pub fn extract_feature_tag(msg: &str) -> Option<String> {
    let re = regex::Regex::new(r"(?:feat|fix|refactor)\(([^)]+)\)").ok()?;
    re.captures(msg).map(|c| c[1].to_string())
}

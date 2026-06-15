//! Commit engine - Transaction finalization.

use crate::Result;
use tokio::process::Command;

#[derive(Clone, Default)]
pub struct Committer {}

impl Committer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Stages `modified_files` and runs `git commit -m message` in `working_dir`.
    /// If git is unavailable or `working_dir` is empty, `git_committed` is false
    /// and the function still returns Ok (non-fatal).
    pub async fn finalize(
        &self,
        working_dir: &std::path::Path,
        modified_files: &[std::path::PathBuf],
        message: &str,
    ) -> Result<CommitReport> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let transaction_id = format!("txn-{}", now);

        let git_committed = if !modified_files.is_empty() && !working_dir.as_os_str().is_empty() {
            self.git_add_and_commit(working_dir, modified_files, message)
                .await
                .unwrap_or_else(|e| {
                    tracing::warn!("git commit skipped: {e}");
                    false
                })
        } else {
            false
        };

        Ok(CommitReport {
            transaction_id,
            files_committed: modified_files.to_vec(),
            git_committed,
        })
    }

    async fn git_add_and_commit(
        &self,
        working_dir: &std::path::Path,
        files: &[std::path::PathBuf],
        message: &str,
    ) -> Result<bool> {
        for file in files {
            let status = Command::new("git")
                .args(["add", "--"])
                .arg(file)
                .current_dir(working_dir)
                .status()
                .await
                .map_err(|e| crate::AgentError::CommitFailed(format!("git add: {e}")))?;
            if !status.success() {
                return Err(crate::AgentError::CommitFailed(format!(
                    "git add failed for {}",
                    file.display()
                )));
            }
        }

        let status = Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(working_dir)
            .status()
            .await
            .map_err(|e| crate::AgentError::CommitFailed(format!("git commit: {e}")))?;

        Ok(status.success())
    }
}

#[derive(Clone, Debug)]
pub struct CommitReport {
    pub transaction_id: String,
    pub files_committed: Vec<std::path::PathBuf>,
    pub git_committed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_committer_creation() {
        let _committer = Committer::new();
    }

    #[tokio::test]
    async fn test_finalize_empty_files_no_git() {
        let _temp_dir = TempDir::new().unwrap();
        let committer = Committer::new();
        let result = committer
            .finalize(std::path::Path::new(""), &[], "empty")
            .await
            .unwrap();
        assert!(!result.transaction_id.is_empty());
        assert!(!result.git_committed);
    }

    #[tokio::test]
    async fn test_finalize_runs_git_commit() {
        use std::process::Command as StdCommand;
        let temp_dir = TempDir::new().unwrap();
        StdCommand::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();

        let file_path = temp_dir.path().join("hello.rs");
        std::fs::write(&file_path, "fn hello() {}").unwrap();

        let committer = Committer::new();
        let result = committer
            .finalize(temp_dir.path(), &[file_path], "test: add hello")
            .await
            .unwrap();

        assert!(!result.transaction_id.is_empty());
        assert_eq!(result.files_committed.len(), 1);
        assert!(result.git_committed, "expected git commit to run");

        let log = StdCommand::new("git")
            .args(["log", "--oneline"])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();
        let log_str = String::from_utf8_lossy(&log.stdout);
        assert!(log_str.contains("test: add hello"), "git log: {log_str}");
    }
}

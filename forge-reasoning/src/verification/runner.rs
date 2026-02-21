//! Async verification runner with parallel execution
//!
//! This module provides VerificationRunner for executing verification checks
//! in parallel with configurable concurrency limits and automatic evidence attachment.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use uuid::Uuid;

use crate::hypothesis::{HypothesisBoard, HypothesisStatus};
use crate::hypothesis::types::HypothesisId;
use crate::hypothesis::evidence::{Evidence, EvidenceId, EvidenceType, EvidenceMetadata};
use crate::errors::{ReasoningError, Result};
use super::check::{
    VerificationCheck, CheckResult, CheckStatus, VerificationCommand,
    PassAction, FailAction, CheckId,
};
use super::retry::{execute_with_retry, RetryConfig};

/// Verification runner for async check execution
pub struct VerificationRunner {
    board: Arc<HypothesisBoard>,
    max_concurrent: usize,
    retry_config: RetryConfig,
    checks: Arc<tokio::sync::Mutex<HashMap<CheckId, VerificationCheck>>>,
}

impl VerificationRunner {
    /// Create a new verification runner
    pub fn new(board: Arc<HypothesisBoard>, max_concurrent: usize) -> Self {
        Self {
            board,
            max_concurrent,
            retry_config: RetryConfig::default(),
            checks: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }

    /// Create a new verification runner with custom retry config
    pub fn with_retry_config(
        board: Arc<HypothesisBoard>,
        max_concurrent: usize,
        retry_config: RetryConfig,
    ) -> Self {
        Self {
            board,
            max_concurrent,
            retry_config,
            checks: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }

    /// Register a verification check
    pub async fn register_check(
        &self,
        name: String,
        hypothesis_id: HypothesisId,
        command: VerificationCommand,
        timeout: Duration,
        on_pass: Option<PassAction>,
        on_fail: Option<FailAction>,
    ) -> Result<CheckId> {
        let check = VerificationCheck::new(
            name,
            hypothesis_id,
            timeout,
            command,
            on_pass,
            on_fail,
        );

        let check_id = check.id;
        let mut checks = self.checks.lock().await;
        checks.insert(check_id, check);
        Ok(check_id)
    }

    /// Get status of a check
    pub fn get_status(&self, check_id: CheckId) -> Option<CheckStatus> {
        // Note: This is a synchronous method that returns cached status
        // For real-time status, we'd need async or channel-based updates
        None // TODO: Implement in Task 3
    }

    /// List all checks
    pub async fn list_checks(&self) -> Vec<(CheckId, CheckStatus)> {
        let checks = self.checks.lock().await;
        checks.iter().map(|(id, check)| (*id, check.status.clone())).collect()
    }

    /// Execute a single check
    async fn execute_check(&self, mut check: VerificationCheck) -> CheckResult {
        check.status = CheckStatus::Running;

        let start = std::time::Instant::now();

        let result = match &check.command {
            VerificationCommand::ShellCommand(cmd) => {
                self.execute_shell_command(cmd, check.timeout).await
            }
            VerificationCommand::CustomAssertion { .. } => {
                // TODO: Implement custom assertions
                CheckResult::Panic {
                    message: "Custom assertions not yet implemented".to_string(),
                }
            }
        };

        let duration = start.elapsed();
        self.update_check_status(check.id, &result).await;

        result
    }

    /// Execute a shell command with timeout
    async fn execute_shell_command(&self, command: &str, timeout: Duration) -> CheckResult {
        use tokio::process::Command;

        let result = tokio::time::timeout(
            timeout,
            Command::new("sh")
                .arg("-c")
                .arg(command)
                .output()
        ).await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                if output.status.success() {
                    CheckResult::Passed {
                        output: stdout,
                        duration: std::time::Duration::from_secs(0), // Placeholder
                    }
                } else {
                    CheckResult::Failed {
                        output: stdout,
                        error: stderr,
                    }
                }
            }
            Ok(Err(e)) => {
                CheckResult::Panic {
                    message: format!("Failed to execute command: {}", e),
                }
            }
            Err(_) => {
                CheckResult::Timeout {
                    output: format!("Command timed out after {:?}", timeout),
                }
            }
        }
    }

    /// Update check status based on result
    async fn update_check_status(&self, check_id: CheckId, result: &CheckResult) {
        let mut checks = self.checks.lock().await;
        if let Some(check) = checks.get_mut(&check_id) {
            check.status = match result {
                CheckResult::Passed { .. } => CheckStatus::Completed,
                _ => CheckStatus::Failed,
            };
        }
    }

    /// Execute a single check with retry logic
    async fn execute_with_retry_wrapper(&self, check: VerificationCheck) -> CheckResult {
        let check_clone = check.clone();
        let board = self.board.clone();

        execute_with_retry(
            || {
                let check = check_clone.clone();
                async move {
                    // Execute the check
                    let result = self.execute_check(check).await;

                    // Check if result is retryable
                    match &result {
                        CheckResult::Timeout { .. } | CheckResult::Panic { .. } => {
                            // These are retryable
                            Err(result)
                        }
                        _ => {
                            // Success or non-retryable error
                            Ok(result)
                        }
                    }
                }
            },
            self.retry_config.clone(),
        ).await.unwrap_or_else(|err| err)
    }

    /// Record check result as evidence
    async fn record_result(
        &self,
        check: &VerificationCheck,
        result: &CheckResult,
    ) -> Result<EvidenceId> {
        let (strength, passed) = match result {
            CheckResult::Passed { .. } => (1.0, true),
            CheckResult::Failed { .. } => (-1.0, false),
            CheckResult::Timeout { .. } => (-1.0, false),
            CheckResult::Panic { .. } => (-1.0, false),
        };

        let metadata = match &check.command {
            VerificationCommand::ShellCommand(cmd) => {
                EvidenceMetadata::Experiment {
                    name: check.name.clone(),
                    test_command: cmd.clone(),
                    output: result.output().unwrap_or("").to_string(),
                    passed,
                }
            }
            VerificationCommand::CustomAssertion { description } => {
                EvidenceMetadata::Experiment {
                    name: check.name.clone(),
                    test_command: format!("assertion: {}", description),
                    output: result.output().unwrap_or("").to_string(),
                    passed,
                }
            }
        };

        let (evidence_id, _posterior) = self.board.attach_evidence(
            check.hypothesis_id,
            EvidenceType::Experiment,
            strength,
            metadata,
        ).await?;

        Ok(evidence_id)
    }

    /// Execute on_pass action
    async fn execute_pass_action(&self, check: &VerificationCheck) -> Result<()> {
        if let Some(action) = &check.on_pass {
            match action {
                PassAction::SetStatus(status) => {
                    self.board.set_status(check.hypothesis_id, status.clone()).await?;
                }
                PassAction::UpdateConfidence(_delta) => {
                    // TODO: Implement confidence update
                }
            }
        }
        Ok(())
    }

    /// Execute on_fail action
    async fn execute_fail_action(&self, check: &VerificationCheck) -> Result<()> {
        if let Some(action) = &check.on_fail {
            match action {
                FailAction::SetStatus(status) => {
                    self.board.set_status(check.hypothesis_id, status.clone()).await?;
                }
                FailAction::UpdateConfidence(_delta) => {
                    // TODO: Implement confidence update
                }
            }
        }
        Ok(())
    }

    /// Execute multiple checks in parallel
    pub async fn execute_checks(&self, check_ids: Vec<CheckId>) -> Vec<(CheckId, CheckResult)> {
        let semaphore = Arc::new(Semaphore::new(self.max_concurrent));
        let mut join_set = JoinSet::new();

        for check_id in check_ids {
            let check = {
                let checks = self.checks.lock().await;
                checks.get(&check_id).cloned()
            };

            if let Some(check) = check {
                let semaphore = semaphore.clone();
                let checks = self.checks.clone();
                let board = self.board.clone();

                join_set.spawn(async move {
                    let _permit = semaphore.acquire().await.unwrap();
                    let result = self.execute_with_retry_wrapper(check.clone()).await;
                    let _ = self.record_result(&check, &result).await;

                    // Execute actions
                    if result.is_success() {
                        let _ = self.execute_pass_action(&check).await;
                    } else {
                        let _ = self.execute_fail_action(&check).await;
                    }

                    (check.id, result)
                });
            }
        }

        let mut results = Vec::new();
        while let Some(result) = join_set.join_next().await {
            if let Ok((check_id, check_result)) = result {
                results.push((check_id, check_result));
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hypothesis::confidence::Confidence;

    #[tokio::test]
    async fn test_runner_creation() {
        let board = Arc::new(HypothesisBoard::in_memory());
        let runner = VerificationRunner::new(board, 10);
        assert_eq!(runner.max_concurrent, 10);
    }

    #[tokio::test]
    async fn test_register_check() {
        let board = Arc::new(HypothesisBoard::in_memory());
        let runner = VerificationRunner::new(board, 10);

        let prior = Confidence::new(0.5).unwrap();
        let h_id = runner.board.propose("Test hypothesis", prior).await.unwrap();

        let check_id = runner.register_check(
            "test check".to_string(),
            h_id,
            VerificationCommand::ShellCommand("echo test".to_string()),
            Duration::from_secs(5),
            None,
            None,
        ).await;

        assert!(check_id.is_ok());
    }

    #[tokio::test]
    async fn test_execute_shell_command_passes() {
        let board = Arc::new(HypothesisBoard::in_memory());
        let runner = VerificationRunner::new(board, 10);

        let result = runner.execute_shell_command("echo hello", Duration::from_secs(1)).await;
        assert!(result.is_passed());
    }

    #[tokio::test]
    async fn test_execute_shell_command_fails() {
        let board = Arc::new(HypothesisBoard::in_memory());
        let runner = VerificationRunner::new(board, 10);

        let result = runner.execute_shell_command("exit 1", Duration::from_secs(1)).await;
        assert!(!result.is_passed());
    }

    #[tokio::test]
    async fn test_execute_shell_command_times_out() {
        let board = Arc::new(HypothesisBoard::in_memory());
        let runner = VerificationRunner::new(board, 10);

        let result = runner.execute_shell_command("sleep 10", Duration::from_millis(100)).await;
        assert!(matches!(result, CheckResult::Timeout { .. }));
    }

    #[tokio::test]
    async fn test_evidence_attachment() {
        let board = Arc::new(HypothesisBoard::in_memory());
        let runner = VerificationRunner::new(board.clone(), 10);

        let prior = Confidence::new(0.5).unwrap();
        let h_id = board.propose("Test", prior).await.unwrap();

        let check = VerificationCheck::new(
            "test".to_string(),
            h_id,
            Duration::from_secs(1),
            VerificationCommand::ShellCommand("echo test".to_string()),
            None,
            None,
        );

        let result = CheckResult::Passed {
            output: "test output".to_string(),
            duration: Duration::from_millis(100),
        };

        let evidence_id = runner.record_result(&check, &result).await;
        assert!(evidence_id.is_ok());

        // Verify evidence was attached
        let evidence_list = board.list_evidence(h_id).await.unwrap();
        assert_eq!(evidence_list.len(), 1);
    }
}

// Helper trait for CheckResult
trait CheckResultHelper {
    fn is_passed(&self) -> bool;
}

impl CheckResultHelper for CheckResult {
    fn is_passed(&self) -> bool {
        matches!(self, CheckResult::Passed { .. })
    }
}

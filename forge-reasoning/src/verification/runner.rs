//! Async verification runner with parallel execution
//!
//! This module provides VerificationRunner for executing verification checks
//! in parallel with configurable concurrency limits and automatic evidence attachment.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

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
    pub async fn get_status(&self, check_id: CheckId) -> Option<CheckStatus> {
        let checks = self.checks.lock().await;
        checks.get(&check_id).map(|c| c.status.clone())
    }

    /// List all checks
    pub async fn list_checks(&self) -> Vec<(CheckId, CheckStatus)> {
        let checks = self.checks.lock().await;
        checks.iter().map(|(id, check)| (*id, check.status.clone())).collect()
    }

    /// Execute a shell command with timeout
    async fn execute_shell_command(&self, command: &str, timeout: Duration) -> CheckResult {
        use tokio::process::Command;

        let start = std::time::Instant::now();

        let result = tokio::time::timeout(
            timeout,
            Command::new("sh")
                .arg("-c")
                .arg(command)
                .output()
        ).await;

        let duration = start.elapsed();

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                if output.status.success() {
                    CheckResult::Passed {
                        output: stdout,
                        duration,
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
            let semaphore = semaphore.clone();
            let checks = self.checks.clone();
            let board = self.board.clone();

            join_set.spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();

                // Get check
                let check = {
                    let checks_lock = checks.lock().await;
                    checks_lock.get(&check_id).cloned()
                };

                let (check, result) = match check {
                    Some(c) => {
                        // Execute shell command
                        let cmd = match &c.command {
                            VerificationCommand::ShellCommand(cmd) => cmd.clone(),
                            VerificationCommand::CustomAssertion { .. } => {
                                return (check_id, CheckResult::Panic {
                                    message: "Custom assertions not yet implemented".to_string()
                                });
                            }
                        };

                        let start = std::time::Instant::now();
                        let cmd_result = tokio::process::Command::new("sh")
                            .arg("-c")
                            .arg(&cmd)
                            .output()
                            .await;

                        let duration = start.elapsed();

                        let result = match cmd_result {
                            Ok(output) => {
                                if output.status.success() {
                                    CheckResult::Passed {
                                        output: String::from_utf8_lossy(&output.stdout).to_string(),
                                        duration,
                                    }
                                } else {
                                    CheckResult::Failed {
                                        output: String::from_utf8_lossy(&output.stdout).to_string(),
                                        error: String::from_utf8_lossy(&output.stderr).to_string(),
                                    }
                                }
                            }
                            Err(e) => CheckResult::Panic {
                                message: format!("Failed to execute: {}", e),
                            }
                        };

                        // Record evidence
                        let (strength, passed) = match &result {
                            CheckResult::Passed { .. } => (1.0, true),
                            _ => (-1.0, false),
                        };

                        let metadata = EvidenceMetadata::Experiment {
                            name: c.name.clone(),
                            test_command: cmd,
                            output: result.output().unwrap_or("").to_string(),
                            passed,
                        };

                        let _ = board.attach_evidence(
                            c.hypothesis_id,
                            EvidenceType::Experiment,
                            strength,
                            metadata,
                        ).await;

                        // Execute actions
                        if result.is_success() {
                            if let Some(PassAction::SetStatus(status)) = &c.on_pass {
                                let _ = board.set_status(c.hypothesis_id, status.clone()).await;
                            }
                        } else {
                            if let Some(FailAction::SetStatus(status)) = &c.on_fail {
                                let _ = board.set_status(c.hypothesis_id, status.clone()).await;
                            }
                        }

                        (check_id, result)
                    }
                    None => {
                        return (check_id, CheckResult::Panic {
                            message: "Check not found".to_string()
                        });
                    }
                };

                (check_id, result)
            });
        }

        let mut results = Vec::new();
        while let Some(result) = join_set.join_next().await {
            if let Ok(r) = result {
                results.push(r);
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
        let runner = VerificationRunner::new(board.clone(), 10);

        let prior = Confidence::new(0.5).unwrap();
        let h_id = board.propose("Test hypothesis", prior).await.unwrap();

        let check_id = runner.register_check(
            "test check".to_string(),
            h_id,
            VerificationCommand::ShellCommand("echo test".to_string()),
            Duration::from_secs(5),
            None,
            None,
        ).await;

        assert!(check_id.is_ok());

        // List checks should include our check
        let checks = runner.list_checks().await;
        assert_eq!(checks.len(), 1);
    }

    #[tokio::test]
    async fn test_execute_shell_command_passes() {
        let board = Arc::new(HypothesisBoard::in_memory());
        let runner = VerificationRunner::new(board, 10);

        let result = runner.execute_shell_command("echo hello", Duration::from_secs(1)).await;
        assert!(result.is_success());
    }

    #[tokio::test]
    async fn test_execute_shell_command_fails() {
        let board = Arc::new(HypothesisBoard::in_memory());
        let runner = VerificationRunner::new(board, 10);

        let result = runner.execute_shell_command("exit 1", Duration::from_secs(1)).await;
        assert!(!result.is_success());
        assert!(matches!(result, CheckResult::Failed { .. }));
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

    #[tokio::test]
    async fn test_on_pass_action_sets_status() {
        let board = Arc::new(HypothesisBoard::in_memory());
        let runner = VerificationRunner::new(board.clone(), 10);

        let prior = Confidence::new(0.5).unwrap();
        let h_id = board.propose("Test", prior).await.unwrap();

        // First set status to UnderTest (valid transition)
        board.set_status(h_id, HypothesisStatus::UnderTest).await.unwrap();

        let check = VerificationCheck::new(
            "test".to_string(),
            h_id,
            Duration::from_secs(1),
            VerificationCommand::ShellCommand("echo test".to_string()),
            Some(PassAction::SetStatus(HypothesisStatus::Confirmed)),
            None,
        );

        let result = CheckResult::Passed {
            output: "test".to_string(),
            duration: Duration::from_millis(100),
        };

        runner.execute_pass_action(&check).await.unwrap();

        let hypothesis = board.get(h_id).await.unwrap().unwrap();
        assert_eq!(hypothesis.status(), HypothesisStatus::Confirmed);
    }

    #[tokio::test]
    async fn test_on_fail_action_sets_status() {
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
            Some(FailAction::SetStatus(HypothesisStatus::Rejected)),
        );

        let result = CheckResult::Failed {
            output: "test".to_string(),
            error: "error".to_string(),
        };

        runner.execute_fail_action(&check).await.unwrap();

        let hypothesis = board.get(h_id).await.unwrap().unwrap();
        assert_eq!(hypothesis.status(), HypothesisStatus::Rejected);
    }

    #[tokio::test]
    async fn test_parallel_execution_respects_semaphore() {
        let board = Arc::new(HypothesisBoard::in_memory());
        let runner = VerificationRunner::new(board.clone(), 2); // max 2 concurrent

        let prior = Confidence::new(0.5).unwrap();
        let h1 = board.propose("H1", prior).await.unwrap();
        let h2 = board.propose("H2", prior).await.unwrap();
        let h3 = board.propose("H3", prior).await.unwrap();

        // Register 3 checks
        let c1 = runner.register_check(
            "check1".to_string(),
            h1,
            VerificationCommand::ShellCommand("sleep 0.1".to_string()),
            Duration::from_secs(1),
            None,
            None,
        ).await.unwrap();

        let c2 = runner.register_check(
            "check2".to_string(),
            h2,
            VerificationCommand::ShellCommand("sleep 0.1".to_string()),
            Duration::from_secs(1),
            None,
            None,
        ).await.unwrap();

        let c3 = runner.register_check(
            "check3".to_string(),
            h3,
            VerificationCommand::ShellCommand("sleep 0.1".to_string()),
            Duration::from_secs(1),
            None,
            None,
        ).await.unwrap();

        // Execute all 3
        let results = runner.execute_checks(vec![c1, c2, c3]).await;
        assert_eq!(results.len(), 3);
    }
}

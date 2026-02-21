//! Verification check types
//!
//! Provides types for defining verification checks with commands, timeouts, and actions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

use crate::hypothesis::types::{HypothesisId, HypothesisStatus};

/// Unique identifier for a verification check
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CheckId(pub Uuid);

impl CheckId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for CheckId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for CheckId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Command to execute for verification
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum VerificationCommand {
    /// Execute a shell command
    ShellCommand(String),
    /// Custom assertion for future extensibility
    CustomAssertion {
        description: String,
        // check_fn will be added in future when function storage is needed
    },
}

/// Action to take when a check passes
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PassAction {
    /// Update hypothesis confidence by given amount (-1.0 to 1.0)
    UpdateConfidence(f64),
    /// Set hypothesis to specific status
    SetStatus(HypothesisStatus),
}

/// Action to take when a check fails
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FailAction {
    /// Update hypothesis confidence by given amount (-1.0 to 1.0)
    UpdateConfidence(f64),
    /// Set hypothesis to specific status
    SetStatus(HypothesisStatus),
}

/// Current status of a verification check
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

/// Result of a verification check execution
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CheckResult {
    Passed {
        output: String,
        duration: Duration,
    },
    Failed {
        output: String,
        error: String,
    },
    Timeout {
        output: String,
    },
    Panic {
        message: String,
    },
}

impl CheckResult {
    /// Check if result indicates success
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Passed { .. })
    }

    /// Get the output string (if available)
    pub fn output(&self) -> Option<&str> {
        match self {
            Self::Passed { output, .. } => Some(output),
            Self::Failed { output, .. } => Some(output),
            Self::Timeout { output } => Some(output),
            Self::Panic { .. } => None,
        }
    }

    /// Get duration (only available for Passed result)
    pub fn duration(&self) -> Option<Duration> {
        match self {
            Self::Passed { duration, .. } => Some(*duration),
            _ => None,
        }
    }
}

/// A verification check to execute
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerificationCheck {
    pub id: CheckId,
    pub name: String,
    pub hypothesis_id: HypothesisId,
    pub timeout: Duration,
    pub command: VerificationCommand,
    pub on_pass: Option<PassAction>,
    pub on_fail: Option<FailAction>,
    pub status: CheckStatus,
    pub created_at: DateTime<Utc>,
}

impl VerificationCheck {
    pub fn new(
        name: String,
        hypothesis_id: HypothesisId,
        timeout: Duration,
        command: VerificationCommand,
        on_pass: Option<PassAction>,
        on_fail: Option<FailAction>,
    ) -> Self {
        Self {
            id: CheckId::new(),
            name,
            hypothesis_id,
            timeout,
            command,
            on_pass,
            on_fail,
            status: CheckStatus::Pending,
            created_at: Utc::now(),
        }
    }

    /// Update the status of this check
    pub fn with_status(mut self, status: CheckStatus) -> Self {
        self.status = status;
        self
    }

    /// Check if this check is retryable based on current status
    pub fn is_retryable(&self) -> bool {
        matches!(self.status, CheckStatus::Failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_id_generation() {
        let id1 = CheckId::new();
        let id2 = CheckId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_check_result_is_success() {
        let passed = CheckResult::Passed {
            output: "test".to_string(),
            duration: Duration::from_millis(100),
        };
        assert!(passed.is_success());

        let failed = CheckResult::Failed {
            output: "test".to_string(),
            error: "error".to_string(),
        };
        assert!(!failed.is_success());
    }

    #[test]
    fn test_check_result_output() {
        let passed = CheckResult::Passed {
            output: "output".to_string(),
            duration: Duration::from_millis(100),
        };
        assert_eq!(passed.output(), Some("output"));

        let timeout = CheckResult::Timeout {
            output: "timeout output".to_string(),
        };
        assert_eq!(timeout.output(), Some("timeout output"));

        let panic = CheckResult::Panic {
            message: "panic".to_string(),
        };
        assert_eq!(panic.output(), None);
    }

    #[test]
    fn test_verification_check_creation() {
        let hypothesis_id = HypothesisId::new();
        let check = VerificationCheck::new(
            "test check".to_string(),
            hypothesis_id,
            Duration::from_secs(5),
            VerificationCommand::ShellCommand("echo test".to_string()),
            Some(PassAction::SetStatus(HypothesisStatus::Confirmed)),
            Some(FailAction::SetStatus(HypothesisStatus::Rejected)),
        );

        assert_eq!(check.name, "test check");
        assert_eq!(check.hypothesis_id, hypothesis_id);
        assert_eq!(check.status, CheckStatus::Pending);
    }

    #[test]
    fn test_verification_check_with_status() {
        let hypothesis_id = HypothesisId::new();
        let check = VerificationCheck::new(
            "test".to_string(),
            hypothesis_id,
            Duration::from_secs(1),
            VerificationCommand::ShellCommand("test".to_string()),
            None,
            None,
        );

        assert_eq!(check.status, CheckStatus::Pending);

        let running_check = check.with_status(CheckStatus::Running);
        assert_eq!(running_check.status, CheckStatus::Running);
    }

    #[test]
    fn test_is_retryable() {
        let hypothesis_id = HypothesisId::new();

        let pending_check = VerificationCheck::new(
            "test".to_string(),
            hypothesis_id,
            Duration::from_secs(1),
            VerificationCommand::ShellCommand("test".to_string()),
            None,
            None,
        );
        assert!(!pending_check.is_retryable());

        let failed_check = pending_check.clone().with_status(CheckStatus::Failed);
        assert!(failed_check.is_retryable());

        let failed_check = pending_check.clone().with_status(CheckStatus::Failed);
        assert!(failed_check.is_retryable());
    }
}

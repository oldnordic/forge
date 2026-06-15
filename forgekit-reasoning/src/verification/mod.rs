//! Verification execution system
//!
//! This module provides async verification execution with parallel execution,
//! configurable timeouts, retry logic with exponential backoff, and automatic
//! evidence attachment to hypotheses.

pub mod check;
pub mod retry;
pub mod runner;

// Public API exports
pub use check::{
    CheckId, CheckResult, CheckStatus, FailAction, PassAction, VerificationCheck,
    VerificationCommand,
};
pub use retry::RetryConfig;
pub use runner::VerificationRunner;

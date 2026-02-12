//! Verification engine - Post-mutation validation.
//!
//! This module implements the verification phase, validating that
//! mutations meet quality and policy requirements.

use crate::{AgentError, Result};
use forge_core::Forge;
use std::sync::Arc;
use std::process::Command;

/// Verifier for post-mutation validation.
///
/// The Verifier runs compile checks, tests, and graph validation.
#[derive(Clone)]
pub struct Verifier {
    /// Forge SDK for graph queries
    forge: Arc<Forge>,
}

impl Verifier {
    /// Creates a new verifier.
    pub fn new(forge: Forge) -> Self {
        Self {
            forge: Arc::new(forge),
        }
    }

    /// Runs compile check via cargo.
    ///
    /// # Arguments
    ///
    /// * `working_dir` - Directory to check
    pub async fn compile_check(&self, working_dir: &std::path::Path) -> Result<Vec<Diagnostic>> {
        let output = Command::new("cargo")
            .args(["check", "--message-format=short"])
            .current_dir(working_dir)
            .output()
            .map_err(|e| AgentError::VerificationFailed(
                format!("Cargo check failed: {}", e)
            ))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut diagnostics = Vec::new();
        for line in stdout.lines().chain(stderr.lines()) {
            if line.contains("error:") {
                diagnostics.push(Diagnostic {
                    level: DiagnosticLevel::Error,
                    message: line.trim().to_string(),
                });
            } else if line.contains("warning:") {
                diagnostics.push(Diagnostic {
                    level: DiagnosticLevel::Warning,
                    message: line.trim().to_string(),
                });
            }
        }

        Ok(diagnostics)
    }

    /// Runs tests via cargo.
    ///
    /// # Arguments
    ///
    /// * `working_dir` - Directory to test
    pub async fn test_check(&self, working_dir: &std::path::Path) -> Result<Vec<Diagnostic>> {
        let output = Command::new("cargo")
            .args(["test", "--message-format=short"])
            .current_dir(working_dir)
            .output()
            .map_err(|e| AgentError::VerificationFailed(
                format!("Cargo test failed: {}", e)
            ))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut diagnostics = Vec::new();

        // Parse test results
        for line in stdout.lines().chain(stderr.lines()) {
            if line.contains("test result:") && line.contains("FAILED") {
                diagnostics.push(Diagnostic {
                    level: DiagnosticLevel::Error,
                    message: line.trim().to_string(),
                });
            } else if line.contains("test result:") && line.contains("ok") {
                // Tests passed
            }
        }

        Ok(diagnostics)
    }

    /// Validates graph consistency.
    ///
    /// Checks for orphan references and broken symbol links.
    pub async fn graph_check(&self, _working_dir: &std::path::Path) -> Result<Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();

        // For v0.4, this is a simplified check
        // In production, would query graph for orphan references

        // Placeholder: graph is assumed consistent
        diagnostics.push(Diagnostic {
            level: DiagnosticLevel::Info,
            message: "Graph consistency check: skipped (not yet implemented)".to_string(),
        });

        Ok(diagnostics)
    }

    /// Runs full verification.
    ///
    /// # Arguments
    ///
    /// * `working_dir` - Directory to verify
    pub async fn verify(&self, working_dir: &std::path::Path) -> Result<VerificationReport> {
        let mut all_diagnostics = Vec::new();

        // Run compile check
        let compile_diags = self.compile_check(working_dir).await?;
        all_diagnostics.extend(compile_diags);

        // Run test check
        let test_diags = self.test_check(working_dir).await?;
        all_diagnostics.extend(test_diags);

        // Run graph check
        let graph_diags = self.graph_check(working_dir).await?;
        all_diagnostics.extend(graph_diags);

        let errors = all_diagnostics
            .iter()
            .filter(|d| d.level == DiagnosticLevel::Error)
            .count();

        let passed = errors == 0;

        Ok(VerificationReport {
            passed,
            diagnostics: all_diagnostics,
        })
    }
}

/// Verification report.
#[derive(Clone, Debug)]
pub struct VerificationReport {
    /// Whether verification passed
    pub passed: bool,
    /// Any diagnostics or errors
    pub diagnostics: Vec<Diagnostic>,
}

/// Diagnostic message.
#[derive(Clone, Debug)]
pub struct Diagnostic {
    /// Severity level
    pub level: DiagnosticLevel,
    /// Diagnostic message
    pub message: String,
}

/// Diagnostic severity level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiagnosticLevel {
    /// Info message
    Info,
    /// Warning message
    Warning,
    /// Error message
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_verifier_creation() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let verifier = Verifier::new(forge);

        // Should create successfully
        assert_eq!(verifier.forge.db_path(), forge.db_path());
    }

    #[test]
    fn test_diagnostic_level_equality() {
        assert_eq!(DiagnosticLevel::Error, DiagnosticLevel::Error);
        assert_ne!(DiagnosticLevel::Error, DiagnosticLevel::Warning);
    }
}

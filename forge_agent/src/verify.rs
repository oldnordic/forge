//! Verification engine - Post-mutation validation.
//!
//! This module implements the verification phase, validating that
//! mutations meet quality and policy requirements.

use crate::{AgentError, Result};
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

/// Verifier for post-mutation validation.
///
/// The Verifier runs compile checks, tests, and graph validation.
/// When an LLM is available, it interprets errors and suggests fixes.
#[derive(Clone)]
pub struct Verifier {
    /// Optional Forge SDK for graph consistency checks
    forge: Option<forge_core::Forge>,
    /// Optional LLM provider for error interpretation
    llm: Option<Arc<dyn crate::llm::LlmProvider>>,
}

impl Default for Verifier {
    fn default() -> Self {
        Self::new()
    }
}

impl Verifier {
    /// Creates a new verifier.
    pub fn new() -> Self {
        Self {
            forge: None,
            llm: None,
        }
    }

    /// Creates a new verifier with Forge SDK for graph checks.
    pub fn with_forge(forge: forge_core::Forge) -> Self {
        Self {
            forge: Some(forge),
            llm: None,
        }
    }

    /// Sets the LLM provider for error interpretation.
    pub fn with_llm(mut self, provider: Arc<dyn crate::llm::LlmProvider>) -> Self {
        self.llm = Some(provider);
        self
    }

    /// Runs compile check.
    ///
    /// Uses `BuildModule::check()` when a Forge instance is available,
    /// falling back to raw `cargo check` otherwise.
    pub async fn compile_check(&self, working_dir: &std::path::Path) -> Result<Vec<Diagnostic>> {
        if let Some(ref forge) = self.forge {
            if let Some(build) = forge.build() {
                let output = build.check(working_dir).await.map_err(|e| {
                    AgentError::VerificationFailed(format!("Build check failed: {}", e))
                })?;
                return Ok(output
                    .diagnostics
                    .iter()
                    .filter_map(|d| {
                        let level = match d.severity {
                            forge_core::diagnostic::DiagnosticSeverity::Error => {
                                Some(DiagnosticLevel::Error)
                            }
                            forge_core::diagnostic::DiagnosticSeverity::Warning => {
                                Some(DiagnosticLevel::Warning)
                            }
                            _ => None,
                        };
                        level.map(|l| Diagnostic {
                            level: l,
                            message: d.message.clone(),
                        })
                    })
                    .collect());
            }
        }

        let output = Command::new("cargo")
            .args(["check", "--message-format=short"])
            .current_dir(working_dir)
            .output()
            .map_err(|e| AgentError::VerificationFailed(format!("Cargo check failed: {}", e)))?;

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

    /// Runs tests.
    ///
    /// Uses `BuildModule::test()` when a Forge instance is available,
    /// falling back to raw `cargo test` otherwise.
    pub async fn test_check(&self, working_dir: &std::path::Path) -> Result<Vec<Diagnostic>> {
        if let Some(ref forge) = self.forge {
            if let Some(build) = forge.build() {
                let output = build.test(working_dir).await.map_err(|e| {
                    AgentError::VerificationFailed(format!("Build test failed: {}", e))
                })?;
                if output.success {
                    return Ok(Vec::new());
                }
                return Ok(output
                    .diagnostics
                    .iter()
                    .filter_map(|d| {
                        let level = match d.severity {
                            forge_core::diagnostic::DiagnosticSeverity::Error => {
                                Some(DiagnosticLevel::Error)
                            }
                            _ => None,
                        };
                        level.map(|l| Diagnostic {
                            level: l,
                            message: d.message.clone(),
                        })
                    })
                    .collect());
            }
        }

        let output = Command::new("cargo")
            .args(["test", "--message-format=short"])
            .current_dir(working_dir)
            .output()
            .map_err(|e| AgentError::VerificationFailed(format!("Cargo test failed: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut diagnostics = Vec::new();

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
    /// Checks for symbols and reports graph health.
    pub async fn graph_check(&self, working_dir: &Path) -> Result<Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();

        if let Some(ref forge) = self.forge {
            // Use Forge SDK to check graph health
            match forge.graph().symbol_count().await {
                Ok(count) => {
                    diagnostics.push(Diagnostic {
                        level: DiagnosticLevel::Info,
                        message: format!(
                            "Graph consistency: {} symbols indexed in {}",
                            count,
                            working_dir.display()
                        ),
                    });
                }
                Err(e) => {
                    diagnostics.push(Diagnostic {
                        level: DiagnosticLevel::Warning,
                        message: format!("Graph query failed: {}", e),
                    });
                }
            }
        } else {
            diagnostics.push(Diagnostic {
                level: DiagnosticLevel::Info,
                message: "Graph check: no Forge SDK available".to_string(),
            });
        }

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

        // Interpret errors using LLM if available and errors exist
        let suggestions = if !passed {
            self.interpret_errors(&all_diagnostics).await
        } else {
            None
        };

        Ok(VerificationReport {
            passed,
            diagnostics: all_diagnostics,
            suggestions,
        })
    }

    /// Runs verification scoped to changed files.
    ///
    /// Like `verify()` but records which files were changed and includes
    /// diff context in LLM error interpretation.
    pub async fn verify_changes(
        &self,
        working_dir: &std::path::Path,
        changed_files: &[std::path::PathBuf],
        diffs: &[String],
    ) -> Result<VerificationReport> {
        tracing::info!(
            "Verifying {} changed files in {}",
            changed_files.len(),
            working_dir.display()
        );
        let mut all_diagnostics = Vec::new();

        let compile_diags = self.compile_check(working_dir).await?;
        all_diagnostics.extend(compile_diags);

        let test_diags = self.test_check(working_dir).await?;
        all_diagnostics.extend(test_diags);

        let graph_diags = self.graph_check(working_dir).await?;
        all_diagnostics.extend(graph_diags);

        let errors = all_diagnostics
            .iter()
            .filter(|d| d.level == DiagnosticLevel::Error)
            .count();

        let passed = errors == 0;

        let suggestions = if !passed {
            if diffs.is_empty() {
                self.interpret_errors(&all_diagnostics).await
            } else {
                self.interpret_errors_with_diffs(&all_diagnostics, diffs)
                    .await
            }
        } else {
            None
        };

        Ok(VerificationReport {
            passed,
            diagnostics: all_diagnostics,
            suggestions,
        })
    }

    /// Interpret verification errors using LLM. Returns suggestions.
    pub async fn interpret_errors(&self, diagnostics: &[Diagnostic]) -> Option<String> {
        let llm = self.llm.as_ref()?;

        let errors: Vec<&Diagnostic> = diagnostics
            .iter()
            .filter(|d| d.level == DiagnosticLevel::Error)
            .collect();

        if errors.is_empty() {
            return None;
        }

        let error_text: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
        let prompt = format!(
            "The following compilation/test errors were detected:\n\n{}",
            error_text.join("\n")
        );

        let system = "You are a Rust compiler error interpreter. Given compilation or test errors, provide concise fix suggestions. Focus on: root cause, specific fix, affected files. Keep response under 200 words.";

        match llm.complete(&prompt, Some(system)).await {
            Ok(suggestions) => Some(suggestions),
            Err(e) => {
                tracing::warn!("LLM error interpretation failed: {e}");
                None
            }
        }
    }

    /// Interpret errors with diff context for richer LLM suggestions.
    pub async fn interpret_errors_with_diffs(
        &self,
        diagnostics: &[Diagnostic],
        diffs: &[String],
    ) -> Option<String> {
        let llm = self.llm.as_ref()?;

        let errors: Vec<&Diagnostic> = diagnostics
            .iter()
            .filter(|d| d.level == DiagnosticLevel::Error)
            .collect();

        if errors.is_empty() {
            return None;
        }

        let error_text: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
        let diff_text = diffs.join("\n");
        let prompt = format!(
            "The following compilation/test errors were detected:\n\n{}\n\nRecent changes (diffs):\n{}",
            error_text.join("\n"),
            diff_text
        );

        let system = "You are a Rust compiler error interpreter. Given compilation/test errors AND the recent code changes that likely caused them, provide concise fix suggestions. Focus on: root cause in the diff, specific fix, affected files. Keep response under 200 words.";

        match llm.complete(&prompt, Some(system)).await {
            Ok(suggestions) => Some(suggestions),
            Err(e) => {
                tracing::warn!("LLM error interpretation with diffs failed: {e}");
                None
            }
        }
    }
}

/// Verification report.
#[derive(Clone, Debug)]
pub struct VerificationReport {
    /// Whether verification passed
    pub passed: bool,
    /// Any diagnostics or errors
    pub diagnostics: Vec<Diagnostic>,
    /// LLM-generated fix suggestions (None if no LLM or no errors)
    pub suggestions: Option<String>,
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

    #[tokio::test]
    async fn test_verifier_creation() {
        let _verifier = Verifier::new();
    }

    #[test]
    fn test_diagnostic_level_equality() {
        assert_eq!(DiagnosticLevel::Error, DiagnosticLevel::Error);
        assert_ne!(DiagnosticLevel::Error, DiagnosticLevel::Warning);
    }

    #[tokio::test]
    async fn test_verifier_interpret_errors_with_llm() {
        let mock = Arc::new(crate::llm::MockProvider::new(
            "Missing semicolon on line 42. Add `;` after the expression.",
        ));

        let verifier = Verifier::new().with_llm(mock);

        let diagnostics = vec![Diagnostic {
            level: DiagnosticLevel::Error,
            message: "error: expected `;`, found `let`".to_string(),
        }];

        let result = verifier.interpret_errors(&diagnostics).await;
        assert!(result.is_some());
        assert!(result.unwrap().contains("semicolon"));
    }

    #[tokio::test]
    async fn test_verifier_interpret_errors_without_llm() {
        let verifier = Verifier::new();

        let diagnostics = vec![Diagnostic {
            level: DiagnosticLevel::Error,
            message: "error: something broke".to_string(),
        }];

        let result = verifier.interpret_errors(&diagnostics).await;
        assert!(
            result.is_none(),
            "should return None when no LLM configured"
        );
    }

    #[tokio::test]
    async fn test_verifier_interpret_errors_no_errors() {
        let mock = Arc::new(crate::llm::MockProvider::new("should not be called"));
        let verifier = Verifier::new().with_llm(mock);

        let diagnostics = vec![Diagnostic {
            level: DiagnosticLevel::Warning,
            message: "warning: unused variable".to_string(),
        }];

        let result = verifier.interpret_errors(&diagnostics).await;
        assert!(
            result.is_none(),
            "should return None when no errors to interpret"
        );
    }

    #[tokio::test]
    async fn test_verifier_verify_changes_scoped_to_files() {
        use std::path::PathBuf;

        let verifier = Verifier::new();
        let changed_files = vec![PathBuf::from("src/lib.rs")];
        let diffs = vec!["--- src/lib.rs\n+++ src/lib.rs\n- old\n+ new".to_string()];

        // verify_changes should accept changed files and diffs
        // On a non-existent project dir it will fail to run cargo,
        // but the method should exist and accept the params
        let temp_dir = tempfile::tempdir().unwrap();
        let result = verifier
            .verify_changes(temp_dir.path(), &changed_files, &diffs)
            .await;

        assert!(result.is_ok(), "verify_changes should succeed");
        // Cargo check on empty dir will have errors, but method ran
    }

    #[tokio::test]
    async fn test_verifier_interpret_errors_with_diff_context() {
        let mock = Arc::new(crate::llm::MockProvider::new(
            "The diff shows you removed the import. Add it back.",
        ));

        let verifier = Verifier::new().with_llm(mock);

        let diagnostics = vec![Diagnostic {
            level: DiagnosticLevel::Error,
            message: "error: cannot find type `Foo` in scope".to_string(),
        }];
        let diffs = vec!["--- src/lib.rs\n- use module::Foo;".to_string()];

        let result = verifier
            .interpret_errors_with_diffs(&diagnostics, &diffs)
            .await;
        assert!(result.is_some());
        assert!(result.unwrap().contains("import"));
    }

    #[tokio::test]
    async fn test_verifier_with_forge_uses_build_module() {
        let temp_dir = tempfile::tempdir().unwrap();
        let forge = forge_core::ForgeBuilder::new()
            .path(temp_dir.path())
            .db_path(temp_dir.path().join("test.db"))
            .build()
            .await
            .unwrap();

        let verifier = Verifier::with_forge(forge);
        let result = verifier.compile_check(temp_dir.path()).await;
        assert!(result.is_ok(), "compile_check with forge should succeed");
    }
}

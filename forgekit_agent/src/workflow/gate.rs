//! Quality gate definitions and runner.
//!
//! Gates run in priority order and short-circuit on Block failures.

use serde::{Deserialize, Serialize};

/// What language a gate targets.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GateLanguage {
    Rust,
    Python,
    TypeScript,
    Go,
}

/// What happens when a gate fails.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum GateAction {
    /// Block further execution on failure
    Block,
    /// Log warning but continue
    Warn,
    /// Attempt automatic fix then re-run
    AutoFix,
}

/// A quality gate definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Gate {
    pub name: String,
    pub tool: String,
    pub language: GateLanguage,
    pub priority: u32,
    pub on_fail: GateAction,
    pub config: Option<String>,
}

/// Result of running a single gate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GateResult {
    pub gate_name: String,
    pub passed: bool,
    pub exit_code: i32,
    pub stdout: String,
    pub structured_output: Option<serde_json::Value>,
    pub errors: u32,
    pub warnings: u32,
    pub duration_ms: u64,
}

/// Runs quality gates in priority order.
/// Short-circuits on the first Block failure.
pub struct GateRunner {
    gates: Vec<Gate>,
}

impl GateRunner {
    pub fn new(gates: Vec<Gate>) -> Self {
        let mut gates = gates;
        gates.sort_by_key(|g| g.priority);
        Self { gates }
    }

    /// Run all gates in the given working directory, returning results in priority order.
    /// Short-circuits on the first `Block` failure.
    pub fn run(&self, working_dir: &std::path::Path) -> Vec<GateResult> {
        let mut results = Vec::new();
        for gate in &self.gates {
            let start = std::time::Instant::now();
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg(&gate.tool)
                .current_dir(working_dir)
                .output();

            let result = match output {
                Ok(out) => {
                    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                    let combined = format!("{stdout}{stderr}");
                    let errors = combined.lines().filter(|l| l.contains("error:")).count() as u32;
                    let warnings =
                        combined.lines().filter(|l| l.contains("warning:")).count() as u32;
                    let passed = out.status.success();
                    GateResult {
                        gate_name: gate.name.clone(),
                        passed,
                        exit_code: out.status.code().unwrap_or(-1),
                        stdout,
                        structured_output: None,
                        errors,
                        warnings,
                        duration_ms: start.elapsed().as_millis() as u64,
                    }
                }
                Err(e) => GateResult {
                    gate_name: gate.name.clone(),
                    passed: false,
                    exit_code: -1,
                    stdout: format!("failed to spawn command: {e}"),
                    structured_output: None,
                    errors: 1,
                    warnings: 0,
                    duration_ms: start.elapsed().as_millis() as u64,
                },
            };

            let should_block = !result.passed && gate.on_fail == GateAction::Block;
            results.push(result);
            if should_block {
                break;
            }
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_gate(name: &str, priority: u32, on_fail: GateAction) -> Gate {
        Gate {
            name: name.to_string(),
            tool: "test-tool".to_string(),
            language: GateLanguage::Rust,
            priority,
            on_fail,
            config: None,
        }
    }

    #[test]
    fn test_gate_priority_ordering() {
        let gates = vec![
            make_gate("low", 30, GateAction::Warn),
            make_gate("high", 10, GateAction::Block),
            make_gate("mid", 20, GateAction::Warn),
        ];
        let runner = GateRunner::new(gates);
        let names: Vec<&str> = runner.gates.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, vec!["high", "mid", "low"]);
    }

    #[test]
    fn test_gate_runner_executes_real_command() {
        let temp_dir = TempDir::new().unwrap();
        let gate = Gate {
            name: "echo".into(),
            tool: "echo hello".into(),
            language: GateLanguage::Rust,
            priority: 0,
            on_fail: GateAction::Warn,
            config: None,
        };
        let runner = GateRunner::new(vec![gate]);
        let results = runner.run(temp_dir.path());
        assert_eq!(results.len(), 1);
        assert!(results[0].passed, "echo should exit 0");
        assert_eq!(results[0].exit_code, 0);
        let _ = results[0].duration_ms; // field is present and set
    }

    #[test]
    fn test_gate_runner_detects_failure() {
        let temp_dir = TempDir::new().unwrap();
        let gate = Gate {
            name: "fail".into(),
            tool: "sh -c 'exit 1'".into(),
            language: GateLanguage::Rust,
            priority: 0,
            on_fail: GateAction::Warn,
            config: None,
        };
        let runner = GateRunner::new(vec![gate]);
        let results = runner.run(temp_dir.path());
        assert_eq!(results.len(), 1);
        assert!(!results[0].passed, "exit 1 should fail");
        assert_ne!(results[0].exit_code, 0);
    }

    #[test]
    fn test_gate_runner_short_circuits_on_block_failure() {
        let temp_dir = TempDir::new().unwrap();
        let gates = vec![
            Gate {
                name: "fail-block".into(),
                tool: "sh -c 'exit 1'".into(),
                language: GateLanguage::Rust,
                priority: 10,
                on_fail: GateAction::Block,
                config: None,
            },
            Gate {
                name: "should-not-run".into(),
                tool: "echo ok".into(),
                language: GateLanguage::Rust,
                priority: 20,
                on_fail: GateAction::Warn,
                config: None,
            },
        ];
        let runner = GateRunner::new(gates);
        let results = runner.run(temp_dir.path());
        assert_eq!(results.len(), 1, "should stop after Block failure");
        assert_eq!(results[0].gate_name, "fail-block");
        assert!(!results[0].passed);
    }

    #[test]
    fn test_gate_runner_warn_does_not_block() {
        let temp_dir = TempDir::new().unwrap();
        let gates = vec![
            Gate {
                name: "fail-warn".into(),
                tool: "sh -c 'exit 1'".into(),
                language: GateLanguage::Rust,
                priority: 10,
                on_fail: GateAction::Warn,
                config: None,
            },
            Gate {
                name: "should-run".into(),
                tool: "echo ok".into(),
                language: GateLanguage::Rust,
                priority: 20,
                on_fail: GateAction::Block,
                config: None,
            },
        ];
        let runner = GateRunner::new(gates);
        let results = runner.run(temp_dir.path());
        assert_eq!(results.len(), 2, "Warn should not block");
        assert!(!results[0].passed);
        assert!(results[1].passed);
    }

    #[test]
    fn test_gate_result_serialization() {
        let result = GateResult {
            gate_name: "clippy".to_string(),
            passed: true,
            exit_code: 0,
            stdout: "no warnings".to_string(),
            structured_output: Some(serde_json::json!({"warnings": 0})),
            errors: 0,
            warnings: 0,
            duration_ms: 42,
        };
        let json = serde_json::to_string(&result).expect("serialization failed");
        let roundtrip: GateResult = serde_json::from_str(&json).expect("deserialization failed");
        assert_eq!(roundtrip.gate_name, "clippy");
        assert!(roundtrip.passed);
        assert_eq!(roundtrip.exit_code, 0);
        assert_eq!(roundtrip.duration_ms, 42);
        assert!(roundtrip.structured_output.is_some());
    }
}

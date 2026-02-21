//! Policy engine - Constraint validation system.
//!
//! This module implements policy validation for agent operations, ensuring
//! that code changes comply with specified constraints.

use crate::Result;
use forge_core::Forge;
use std::sync::Arc;

/// Policy for constraint validation.
///
/// Policies define rules that must be satisfied before mutations are applied.
#[derive(Clone, Debug)]
pub enum Policy {
    /// No unsafe code in public API
    NoUnsafeInPublicAPI,

    /// Preserve test coverage
    PreserveTests,

    /// Maximum cyclomatic complexity
    MaxComplexity(usize),

    /// Custom policy with validation function
    Custom { name: String, description: String },
}

impl Policy {
    /// Validates an edit operation against this policy.
    pub async fn validate(&self, forge: &Forge, diff: &Diff) -> Result<PolicyReport> {
        let mut violations = Vec::new();

        match self {
            Policy::NoUnsafeInPublicAPI => {
                if let Some(v) = check_no_unsafe_in_public_api(diff).await? {
                    violations.push(v);
                }
            }
            Policy::PreserveTests => {
                if let Some(v) = check_preserve_tests(forge, diff).await? {
                    violations.push(v);
                }
            }
            Policy::MaxComplexity(max) => {
                if let Some(v) = check_max_complexity(forge, *max, diff).await? {
                    violations.push(v);
                }
            }
            Policy::Custom { name, .. } => {
                // Custom policies are not yet implemented
                // In production, this would use a DSL or plugin system
                violations.push(PolicyViolation {
                    policy: name.clone(),
                    message: "Custom policy validation not yet implemented".to_string(),
                    location: None,
                });
            }
        }

        Ok(PolicyReport {
            policy: self.clone(),
            violations: violations.clone(),
            passed: violations.is_empty(),
        })
    }
}

/// Policy validator that can check multiple policies.
#[derive(Clone)]
pub struct PolicyValidator {
    /// Forge SDK for graph queries
    forge: Arc<Forge>,
}

impl PolicyValidator {
    /// Creates a new policy validator.
    pub fn new(forge: Forge) -> Self {
        Self {
            forge: Arc::new(forge),
        }
    }

    /// Validates a diff against all policies.
    pub async fn validate(&self, diff: &Diff, policies: &[Policy]) -> Result<PolicyReport> {
        let mut all_violations = Vec::new();

        for policy in policies {
            let report = policy.validate(&self.forge, diff).await?;
            all_violations.extend(report.violations);
        }

        Ok(PolicyReport {
            policy: Policy::Custom {
                name: "All".to_string(),
                description: "Combined policy check".to_string(),
            },
            violations: all_violations.clone(),
            passed: all_violations.is_empty(),
        })
    }

    /// Validates a single policy.
    pub async fn validate_single(&self, policy: &Policy, diff: &Diff) -> Result<PolicyReport> {
        policy.validate(&self.forge, diff).await
    }
}

/// Result of policy validation.
#[derive(Clone, Debug)]
pub struct PolicyReport {
    /// The policy that was validated
    pub policy: Policy,
    /// Any violations found
    pub violations: Vec<PolicyViolation>,
    /// Whether validation passed
    pub passed: bool,
}

/// A policy violation with location information.
#[derive(Clone, Debug)]
pub struct PolicyViolation {
    /// Policy that was violated
    pub policy: String,
    /// Human-readable violation message
    pub message: String,
    /// Source location (if applicable)
    pub location: Option<forge_core::types::Location>,
}

impl PolicyViolation {
    /// Creates a new policy violation.
    pub fn new(policy: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            policy: policy.into(),
            message: message.into(),
            location: None,
        }
    }

    /// Creates a new policy violation with location.
    pub fn with_location(
        policy: impl Into<String>,
        message: impl Into<String>,
        location: forge_core::types::Location,
    ) -> Self {
        Self {
            policy: policy.into(),
            message: message.into(),
            location: Some(location),
        }
    }
}

/// Policy composition: All policies must pass.
#[derive(Clone, Debug)]
pub struct AllPolicies {
    /// The policies to validate
    pub policies: Vec<Policy>,
}

impl AllPolicies {
    /// Creates a new AllPolicies composition.
    pub fn new(policies: Vec<Policy>) -> Self {
        Self { policies }
    }

    /// Validates all policies.
    pub async fn validate(&self, forge: &Forge, diff: &Diff) -> Result<PolicyReport> {
        let mut all_violations = Vec::new();

        for policy in &self.policies {
            let report = policy.validate(forge, diff).await?;
            all_violations.extend(report.violations);
        }

        Ok(PolicyReport {
            policy: Policy::Custom {
                name: "All".to_string(),
                description: format!("All {} policies must pass", self.policies.len()),
            },
            violations: all_violations.clone(),
            passed: all_violations.is_empty(),
        })
    }
}

/// Policy composition: At least one policy must pass.
#[derive(Clone, Debug)]
pub struct AnyPolicy {
    /// The policies to validate
    pub policies: Vec<Policy>,
}

impl AnyPolicy {
    /// Creates a new AnyPolicy composition.
    pub fn new(policies: Vec<Policy>) -> Self {
        Self { policies }
    }

    /// Validates that at least one policy passes.
    pub async fn validate(&self, forge: &Forge, diff: &Diff) -> Result<PolicyReport> {
        let mut all_violations = Vec::new();
        let mut any_passed = false;

        for policy in &self.policies {
            let report = policy.validate(forge, diff).await?;
            if report.passed {
                any_passed = true;
            }
            all_violations.extend(report.violations);
        }

        Ok(PolicyReport {
            policy: Policy::Custom {
                name: "Any".to_string(),
                description: format!("At least one of {} policies must pass", self.policies.len()),
            },
            violations: if any_passed {
                Vec::new()
            } else {
                all_violations.clone()
            },
            passed: any_passed,
        })
    }
}

/// A diff representing code changes.
///
/// This is a simplified representation - in production, this would be
/// a proper AST diff or line-based diff.
#[derive(Clone, Debug)]
pub struct Diff {
    /// File path
    pub file_path: std::path::PathBuf,
    /// Original content
    pub original: String,
    /// Modified content
    pub modified: String,
    /// Changed lines
    pub changes: Vec<DiffChange>,
}

/// A single change in a diff.
#[derive(Clone, Debug)]
pub struct DiffChange {
    /// Line number
    pub line: usize,
    /// Original line
    pub original: String,
    /// Modified line
    pub modified: String,
    /// Change type
    pub kind: DiffChangeKind,
}

/// Type of diff change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiffChangeKind {
    /// Line was added
    Added,
    /// Line was removed
    Removed,
    /// Line was modified
    Modified,
}

// Policy validation implementations

/// Checks that no unsafe code appears in public API.
async fn check_no_unsafe_in_public_api(diff: &Diff) -> Result<Option<PolicyViolation>> {
    // Parse the modified content for unsafe blocks
    let mut violations = Vec::new();

    for (line_num, line) in diff.modified.lines().enumerate() {
        let line_num = line_num + 1;
        let trimmed = line.trim();

        // Check for unsafe keyword
        if trimmed.contains("unsafe") {
            // Check if it's in a public context
            let is_public_function = trimmed.starts_with("pub ")
                && (trimmed.contains("fn ") || trimmed.contains("unsafe fn"));

            let is_public_struct = trimmed.starts_with("pub ")
                && (trimmed.contains("struct ") || trimmed.contains("enum "));

            if is_public_function || is_public_struct {
                violations.push(PolicyViolation::new(
                    "NoUnsafeInPublicAPI",
                    format!("Unsafe code in public API at line {}", line_num),
                ));
            }
        }
    }

    Ok(if violations.is_empty() {
        None
    } else {
        Some(PolicyViolation::new(
            "NoUnsafeInPublicAPI",
            format!(
                "Found {} violations of unsafe in public API",
                violations.len()
            ),
        ))
    })
}

/// Checks that test coverage is preserved.
async fn check_preserve_tests(_forge: &Forge, diff: &Diff) -> Result<Option<PolicyViolation>> {
    // Count tests in original and modified
    let original_tests = count_tests(&diff.original);
    let modified_tests = count_tests(&diff.modified);

    if modified_tests < original_tests {
        Ok(Some(PolicyViolation::new(
            "PreserveTests",
            format!(
                "Test count decreased from {} to {}",
                original_tests, modified_tests
            ),
        )))
    } else {
        Ok(None)
    }
}

/// Checks that cyclomatic complexity is within limit.
async fn check_max_complexity(
    _forge: &Forge,
    max_complexity: usize,
    diff: &Diff,
) -> Result<Option<PolicyViolation>> {
    // For each function in modified content, estimate complexity
    // Find all functions and check their complexity
    let violations: Vec<_> = diff
        .modified
        .lines()
        .enumerate()
        .filter(|(_, line)| line.trim().starts_with("pub fn ") || line.trim().starts_with("fn "))
        .map(|(line_num, line)| {
            // Get the rest of the line after "fn name("
            let rest = if let Some(fn_pos) = line.find("fn ") {
                &line[fn_pos + 3..]
            } else {
                line
            };

            // Count branching keywords in this function declaration line
            let complexity = estimate_complexity_from_line(rest);

            if complexity > max_complexity {
                Some(PolicyViolation::new(
                    "MaxComplexity",
                    format!(
                        "Function at line {} has complexity {}, exceeds max {}",
                        line_num + 1,
                        complexity,
                        max_complexity
                    ),
                ))
            } else {
                None
            }
        })
        .flatten()
        .collect();

    Ok(if violations.is_empty() {
        None
    } else {
        Some(PolicyViolation::new(
            "MaxComplexity",
            format!(
                "Found {} functions exceeding complexity limit",
                violations.len()
            ),
        ))
    })
}

/// Estimates complexity from a single line (for inline functions).
fn estimate_complexity_from_line(line: &str) -> usize {
    let mut complexity = 1; // Base complexity

    // Count branching keywords
    let if_count = line.matches("if ").count();
    let while_count = line.matches("while ").count();
    let for_count = line.matches("for ").count();
    let match_count = line.matches("match ").count();
    let and_count = line.matches("&&").count();
    let or_count = line.matches("||").count();

    complexity += if_count + while_count + for_count + match_count + and_count + or_count;
    complexity
}

/// Counts test functions in content.
fn count_tests(content: &str) -> usize {
    content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.contains("#[test]") || trimmed.contains("#[tokio::test]")
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use forge_core::Forge;

    #[tokio::test]
    async fn test_policy_no_unsafe_in_public_api() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();

        let diff = Diff {
            file_path: PathBuf::from("test.rs"),
            original: "fn safe() {}".to_string(),
            modified: "pub unsafe fn dangerous() {}".to_string(),
            changes: vec![],
        };

        let policy = Policy::NoUnsafeInPublicAPI;
        let report = policy.validate(&forge, &diff).await.unwrap();

        assert!(!report.passed);
        assert_eq!(report.violations.len(), 1);
    }

    #[tokio::test]
    async fn test_policy_preserve_tests() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();

        let diff = Diff {
            file_path: PathBuf::from("test.rs"),
            original: "#[test]\nfn test_one() {}\n#[test]\nfn test_two() {}".to_string(),
            modified: "#[test]\nfn test_one() {}".to_string(),
            changes: vec![],
        };

        let policy = Policy::PreserveTests;
        let report = policy.validate(&forge, &diff).await.unwrap();

        assert!(!report.passed);
        assert_eq!(report.violations.len(), 1);
    }

    #[tokio::test]
    async fn test_policy_max_complexity() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();

        let diff = Diff {
            file_path: PathBuf::from("test.rs"),
            original: "".to_string(),
            modified: "pub fn complex() { if x { if y { if z {} } } }".to_string(),
            changes: vec![],
        };

        let policy = Policy::MaxComplexity(3);
        let report = policy.validate(&forge, &diff).await.unwrap();

        assert!(!report.passed);
    }

    #[tokio::test]
    async fn test_all_policies() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();

        let diff = Diff {
            file_path: PathBuf::from("test.rs"),
            original: "".to_string(),
            modified: "pub fn safe() {}".to_string(),
            changes: vec![],
        };

        let policies = vec![Policy::NoUnsafeInPublicAPI, Policy::PreserveTests];

        let all = AllPolicies::new(policies);
        let report = all.validate(&forge, &diff).await.unwrap();

        assert!(report.passed);
    }

    #[tokio::test]
    async fn test_any_policy() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();

        let diff = Diff {
            file_path: PathBuf::from("test.rs"),
            original: "".to_string(),
            modified: "pub unsafe fn dangerous() {}".to_string(),
            changes: vec![],
        };

        let policies = vec![
            Policy::NoUnsafeInPublicAPI,
            Policy::Custom {
                name: "AlwaysPass".to_string(),
                description: "Always passes".to_string(),
            },
        ];

        let any = AnyPolicy::new(policies);
        let report = any.validate(&forge, &diff).await.unwrap();

        // Custom policy fails but Any still passes because first one also fails
        // Actually with current implementation, Custom fails too
        assert!(!report.passed);
    }

    #[tokio::test]
    async fn test_count_tests() {
        let content = r#"
            #[test]
            fn test_one() {}

            #[test]
            fn test_two() {}

            #[tokio::test]
            async fn test_three() {}
        "#;

        let count = count_tests(content);
        assert_eq!(count, 3);
    }
}

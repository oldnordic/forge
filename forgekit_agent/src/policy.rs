//! Policy engine - Constraint validation system.
//!
//! This module implements policy validation for agent operations, ensuring
//! that code changes comply with specified constraints.

use crate::Result;
use forgekit_core::Forge;
use std::fmt;
use std::sync::Arc;

type CustomValidatorFn = Arc<dyn Fn(&Diff) -> Vec<PolicyViolation> + Send + Sync>;

/// Policy for constraint validation.
///
/// Policies define rules that must be satisfied before mutations are applied.
#[derive(Clone)]
pub enum Policy {
    /// No unsafe code in public API
    NoUnsafeInPublicAPI,

    /// Preserve test coverage
    PreserveTests,

    /// Maximum cyclomatic complexity
    MaxComplexity(usize),

    /// Custom policy with a caller-supplied validation closure.
    ///
    /// Create with [`Policy::custom`].
    Custom {
        name: String,
        description: String,
        validator: CustomValidatorFn,
    },
}

impl fmt::Debug for Policy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoUnsafeInPublicAPI => write!(f, "NoUnsafeInPublicAPI"),
            Self::PreserveTests => write!(f, "PreserveTests"),
            Self::MaxComplexity(n) => write!(f, "MaxComplexity({n})"),
            Self::Custom {
                name, description, ..
            } => f
                .debug_struct("Custom")
                .field("name", name)
                .field("description", description)
                .finish(),
        }
    }
}

impl Policy {
    /// Creates a custom policy backed by a synchronous validation closure.
    ///
    /// # Arguments
    ///
    /// * `name` - Machine-readable policy identifier
    /// * `description` - Human-readable description
    /// * `validator` - Closure that inspects the diff and returns any violations
    pub fn custom(
        name: impl Into<String>,
        description: impl Into<String>,
        validator: impl Fn(&Diff) -> Vec<PolicyViolation> + Send + Sync + 'static,
    ) -> Self {
        Self::Custom {
            name: name.into(),
            description: description.into(),
            validator: Arc::new(validator),
        }
    }
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
            Policy::Custom { validator, .. } => {
                violations.extend(validator(diff));
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
            policy: Policy::custom("All", "Combined policy check", |_| vec![]),
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
    pub location: Option<forgekit_core::types::Location>,
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
        location: forgekit_core::types::Location,
    ) -> Self {
        Self {
            policy: policy.into(),
            message: message.into(),
            location: Some(location),
        }
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
        .filter_map(|(line_num, line)| {
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
    use forgekit_core::Forge;
    use std::path::PathBuf;
    use tempfile::TempDir;

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

    #[tokio::test]
    async fn test_custom_policy_passing_validator() {
        let temp = tempfile::tempdir().unwrap();
        let forge = forgekit_core::Forge::open(temp.path()).await.unwrap();

        // Validator that always passes (no violations)
        let policy = Policy::custom("allow-all", "permits everything", |_diff| vec![]);

        let diff = Diff {
            file_path: std::path::PathBuf::from("src/lib.rs"),
            original: "fn foo() {}".to_string(),
            modified: "fn foo() { /* changed */ }".to_string(),
            changes: vec![],
        };

        let report = policy.validate(&forge, &diff).await.unwrap();
        assert!(
            report.passed,
            "custom passing validator should produce no violations"
        );
    }

    #[tokio::test]
    async fn test_custom_policy_failing_validator() {
        let temp = tempfile::tempdir().unwrap();
        let forge = forgekit_core::Forge::open(temp.path()).await.unwrap();

        // Validator that always fires a violation
        let policy = Policy::custom("no-changes", "forbids any diff", |diff| {
            vec![PolicyViolation::new(
                "no-changes",
                format!("file {} must not be modified", diff.file_path.display()),
            )]
        });

        let diff = Diff {
            file_path: std::path::PathBuf::from("src/lib.rs"),
            original: "fn foo() {}".to_string(),
            modified: "fn foo() { /* changed */ }".to_string(),
            changes: vec![],
        };

        let report = policy.validate(&forge, &diff).await.unwrap();
        assert!(
            !report.passed,
            "custom failing validator should produce violations"
        );
        assert_eq!(report.violations.len(), 1);
        assert!(report.violations[0]
            .message
            .contains("must not be modified"));
    }
}

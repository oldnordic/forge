//! Semgrep integration for forge quality gates.
//!
//! Parses semgrep JSON output into structured findings.

use serde::{Deserialize, Serialize};

/// A single semgrep finding.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SemgrepFinding {
    pub check_id: String,
    pub file: String,
    pub start_line: u32,
    pub end_line: u32,
    pub message: String,
    pub severity: String,
    pub category: Option<String>,
}

/// Runs semgrep with configurable rulesets.
pub struct SemgrepRunner {
    _configs: Vec<String>,
    _json_output: bool,
}

impl SemgrepRunner {
    pub fn new(configs: Vec<String>) -> Self {
        Self {
            _configs: configs,
            _json_output: true,
        }
    }

    /// Parse semgrep JSON output into findings.
    pub fn parse_output(json: &str) -> anyhow::Result<Vec<SemgrepFinding>> {
        let value: serde_json::Value = serde_json::from_str(json)?;
        let results = value
            .get("results")
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        Some(SemgrepFinding {
                            check_id: item.get("check_id")?.as_str()?.to_string(),
                            file: item.get("path")?.as_str()?.to_string(),
                            start_line: item.get("start")?.get("line")?.as_u64()? as u32,
                            end_line: item.get("end")?.get("line")?.as_u64()? as u32,
                            message: item.get("extra")?.get("message")?.as_str()?.to_string(),
                            severity: item.get("extra")?.get("severity")?.as_str()?.to_string(),
                            category: item
                                .get("extra")?
                                .get("metadata")?
                                .get("category")?
                                .as_str()
                                .map(String::from),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semgrep_finding_parse_from_json() {
        let json = r#"{"results":[{"check_id":"llm-sql-injection","path":"src/db.py","start":{"line":10},"end":{"line":12},"extra":{"message":"SQL injection","severity":"ERROR","metadata":{"category":"security"}}}]}"#;
        let findings = SemgrepRunner::parse_output(json).expect("parse should succeed");

        assert_eq!(findings.len(), 1);
        let f = &findings[0];
        assert_eq!(f.check_id, "llm-sql-injection");
        assert_eq!(f.file, "src/db.py");
        assert_eq!(f.start_line, 10);
        assert_eq!(f.end_line, 12);
        assert_eq!(f.message, "SQL injection");
        assert_eq!(f.severity, "ERROR");
        assert_eq!(f.category, Some("security".to_string()));
    }

    #[test]
    fn test_semgrep_empty_results() {
        let json = r#"{"results":[]}"#;
        let findings = SemgrepRunner::parse_output(json).expect("parse should succeed");
        assert!(findings.is_empty());
    }

    #[test]
    fn test_semgrep_invalid_json() {
        let json = "not valid json {{{";
        let result = SemgrepRunner::parse_output(json);
        assert!(result.is_err());
    }
}

use std::path::PathBuf;

use crate::types::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub code: Option<String>,
    pub message: String,
    pub source: DiagnosticSource,
    pub location: Option<DiagnosticLocation>,
    pub related: Vec<RelatedInfo>,
    pub fixes: Vec<FixSuggestion>,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            code: None,
            message: message.into(),
            source: DiagnosticSource::Unknown,
            location: None,
            related: Vec::new(),
            fixes: Vec::new(),
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Warning,
            code: None,
            message: message.into(),
            source: DiagnosticSource::Unknown,
            location: None,
            related: Vec::new(),
            fixes: Vec::new(),
        }
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    pub fn with_source(mut self, source: DiagnosticSource) -> Self {
        self.source = source;
        self
    }

    pub fn with_location(mut self, loc: DiagnosticLocation) -> Self {
        self.location = Some(loc);
        self
    }

    pub fn with_fix(mut self, fix: FixSuggestion) -> Self {
        self.fixes.push(fix);
        self
    }

    pub fn with_related(mut self, info: RelatedInfo) -> Self {
        self.related.push(info);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticSource {
    Compiler(String),
    Linter(String),
    TestRunner,
    TypeChecker,
    GraphAnalysis,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticLocation {
    pub file: PathBuf,
    pub line: usize,
    pub column: Option<usize>,
    pub span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedInfo {
    pub message: String,
    pub file: PathBuf,
    pub line: usize,
    pub column: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixSuggestion {
    pub title: String,
    pub edits: Vec<TextEdit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub file: PathBuf,
    pub range: Span,
    pub new_text: String,
}

pub trait DiagnosticParser: Send + Sync {
    fn parse(&self, stdout: &str, stderr: &str) -> Vec<Diagnostic>;
}

pub struct CargoDiagnosticParser;

impl DiagnosticParser for CargoDiagnosticParser {
    fn parse(&self, _stdout: &str, stderr: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for line in stderr.lines() {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(reason) = value.get("reason").and_then(|r| r.as_str()) {
                    if reason == "compiler-message" {
                        if let Some(msg) = value.get("message") {
                            if let Some(d) = parse_cargo_message(msg) {
                                diagnostics.push(d);
                            }
                        }
                    }
                }
            } else if is_generic_rust_error(line) {
                if let Some(d) = parse_rustc_line(line) {
                    diagnostics.push(d);
                }
            }
        }

        diagnostics
    }
}

fn parse_cargo_message(msg: &serde_json::Value) -> Option<Diagnostic> {
    let message = msg.get("message")?.as_str()?.to_string();
    let level = msg.get("level")?.as_str().unwrap_or("error");
    let code = msg
        .get("code")
        .and_then(|c| c.get("code"))
        .and_then(|c| c.as_str())
        .map(|s| s.to_string());

    let severity = match level {
        "error" => DiagnosticSeverity::Error,
        "warning" => DiagnosticSeverity::Warning,
        "note" => DiagnosticSeverity::Info,
        _ => DiagnosticSeverity::Hint,
    };

    let spans = msg
        .get("spans")
        .and_then(|s| s.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|s| {
                    let file = s.get("file_name")?.as_str()?;
                    let line = s.get("line_start")?.as_u64()? as usize;
                    let column = s
                        .get("column_start")
                        .and_then(|c| c.as_u64())
                        .map(|c| c as usize);
                    Some(DiagnosticLocation {
                        file: PathBuf::from(file),
                        line,
                        column,
                        span: None,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let location = spans.into_iter().next();

    let children = msg
        .get("children")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|child| {
                    let child_msg = child.get("message")?.as_str()?.to_string();
                    Some(RelatedInfo {
                        message: child_msg,
                        file: PathBuf::new(),
                        line: 0,
                        column: None,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Some(Diagnostic {
        severity,
        code,
        message,
        source: DiagnosticSource::Compiler("rustc".to_string()),
        location,
        related: children,
        fixes: Vec::new(),
    })
}

fn is_generic_rust_error(line: &str) -> bool {
    let re = regex::Regex::new(r"^error\[E\d{4}\]:|^error:|^warning:").unwrap();
    re.is_match(line)
}

fn parse_rustc_line(line: &str) -> Option<Diagnostic> {
    let re = regex::Regex::new(r"^(error|warning)\[(E\d+)\]: (.+)").unwrap();
    let caps = re.captures(line)?;

    let level = caps.get(1)?.as_str();
    let code = caps.get(2)?.as_str().to_string();
    let message = caps.get(3)?.as_str().to_string();

    let severity = if level == "error" {
        DiagnosticSeverity::Error
    } else {
        DiagnosticSeverity::Warning
    };

    Some(
        Diagnostic::error(message)
            .with_code(code)
            .with_source(DiagnosticSource::Compiler("rustc".to_string())),
    )
    .map(|mut d| {
        d.severity = severity;
        d
    })
}

pub struct GoDiagnosticParser;

impl DiagnosticParser for GoDiagnosticParser {
    fn parse(&self, _stdout: &str, stderr: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let re =
            regex::Regex::new(r"^(?P<file>[^:\s]+):(?P<line>\d+):(?P<col>\d+):\s*(?P<message>.+)")
                .unwrap();

        for line in stderr.lines() {
            if let Some(caps) = re.captures(line) {
                let file = caps.name("file").map(|m| m.as_str()).unwrap_or("");
                let line_num = caps
                    .name("line")
                    .and_then(|m| m.as_str().parse::<usize>().ok())
                    .unwrap_or(0);
                let col = caps
                    .name("col")
                    .and_then(|m| m.as_str().parse::<usize>().ok());
                let message = caps
                    .name("message")
                    .map(|m| m.as_str().trim().to_string())
                    .unwrap_or_default();

                let severity = if message.starts_with("cannot")
                    || message.starts_with("undefined")
                    || message.starts_with("syntax error")
                {
                    DiagnosticSeverity::Error
                } else {
                    DiagnosticSeverity::Warning
                };

                diagnostics.push(Diagnostic {
                    severity,
                    code: None,
                    message,
                    source: DiagnosticSource::Compiler("go".to_string()),
                    location: Some(DiagnosticLocation {
                        file: PathBuf::from(file),
                        line: line_num,
                        column: col,
                        span: None,
                    }),
                    related: Vec::new(),
                    fixes: Vec::new(),
                });
            }
        }

        diagnostics
    }
}

pub struct GenericDiagnosticParser {
    pub tool_name: String,
}

impl DiagnosticParser for GenericDiagnosticParser {
    fn parse(&self, _stdout: &str, stderr: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let re = regex::Regex::new(
            r"^(?P<file>[^:\s]+):(?P<line>\d+)(?::(?P<col>\d+))?:\s*(?P<message>.+)",
        )
        .unwrap();

        for line in stderr.lines() {
            if let Some(caps) = re.captures(line) {
                let file = caps.name("file").map(|m| m.as_str()).unwrap_or("");
                let line_num = caps
                    .name("line")
                    .and_then(|m| m.as_str().parse::<usize>().ok())
                    .unwrap_or(0);
                let col = caps
                    .name("col")
                    .and_then(|m| m.as_str().parse::<usize>().ok());
                let message = caps
                    .name("message")
                    .map(|m| m.as_str().trim().to_string())
                    .unwrap_or_default();

                let severity = if message.starts_with("error") {
                    DiagnosticSeverity::Error
                } else if message.starts_with("warning") {
                    DiagnosticSeverity::Warning
                } else {
                    DiagnosticSeverity::Info
                };

                diagnostics.push(Diagnostic {
                    severity,
                    code: None,
                    message,
                    source: DiagnosticSource::Compiler(self.tool_name.clone()),
                    location: Some(DiagnosticLocation {
                        file: PathBuf::from(file),
                        line: line_num,
                        column: col,
                        span: None,
                    }),
                    related: Vec::new(),
                    fixes: Vec::new(),
                });
            }
        }

        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_error_builder() {
        let d = Diagnostic::error("something broke")
            .with_code("E0001")
            .with_source(DiagnosticSource::Compiler("rustc".to_string()));
        assert_eq!(d.severity, DiagnosticSeverity::Error);
        assert_eq!(d.code.as_deref(), Some("E0001"));
        assert_eq!(d.message, "something broke");
    }

    #[test]
    fn test_diagnostic_warning_builder() {
        let d = Diagnostic::warning("unused variable");
        assert_eq!(d.severity, DiagnosticSeverity::Warning);
        assert!(d.code.is_none());
    }

    #[test]
    fn test_diagnostic_with_location() {
        let d = Diagnostic::error("bad code").with_location(DiagnosticLocation {
            file: PathBuf::from("src/main.rs"),
            line: 42,
            column: Some(10),
            span: None,
        });
        assert!(d.location.is_some());
        let loc = d.location.unwrap();
        assert_eq!(loc.line, 42);
    }

    #[test]
    fn test_diagnostic_with_fix() {
        let d = Diagnostic::error("missing semicolon").with_fix(FixSuggestion {
            title: "Add semicolon".to_string(),
            edits: vec![TextEdit {
                file: PathBuf::from("foo.rs"),
                range: Span { start: 10, end: 10 },
                new_text: ";".to_string(),
            }],
        });
        assert_eq!(d.fixes.len(), 1);
        assert_eq!(d.fixes[0].title, "Add semicolon");
    }

    #[test]
    fn test_cargo_parser_json_message() {
        let json = r#"{"reason":"compiler-message","message":{"message":"cannot find value `x` in this scope","code":{"code":"E0425","explanation":""},"level":"error","spans":[{"file_name":"src/main.rs","byte_start":100,"byte_end":101,"line_start":5,"line_end":5,"column_start":12,"column_end":13}]}}"#;
        let parser = CargoDiagnosticParser;
        let diags = parser.parse("", json);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, DiagnosticSeverity::Error);
        assert_eq!(diags[0].code.as_deref(), Some("E0425"));
        assert!(diags[0].location.is_some());
        let loc = diags[0].location.as_ref().unwrap();
        assert_eq!(loc.file, PathBuf::from("src/main.rs"));
        assert_eq!(loc.line, 5);
    }

    #[test]
    fn test_cargo_parser_rustc_line() {
        let stderr = "error[E0425]: cannot find value `x` in this scope\n";
        let parser = CargoDiagnosticParser;
        let diags = parser.parse("", stderr);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code.as_deref(), Some("E0425"));
    }

    #[test]
    fn test_cargo_parser_ignores_noise() {
        let stderr = "   Compiling my-crate v0.1.0\n    Finished dev profile\n";
        let parser = CargoDiagnosticParser;
        let diags = parser.parse("", stderr);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_go_parser() {
        let stderr =
            "main.go:10:3: syntax error: unexpected }\nother.go:5:1: cannot find package\n";
        let parser = GoDiagnosticParser;
        let diags = parser.parse("", stderr);
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0].location.as_ref().unwrap().line, 10);
        assert_eq!(diags[1].location.as_ref().unwrap().line, 5);
    }

    #[test]
    fn test_go_parser_ignores_clean() {
        let stderr = "build\n";
        let parser = GoDiagnosticParser;
        let diags = parser.parse("", stderr);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_generic_parser() {
        let stderr = "src/main.rs:42:10: error: undeclared identifier\nsrc/lib.rs:5:1: warning: unused import\n";
        let parser = GenericDiagnosticParser {
            tool_name: "cc".to_string(),
        };
        let diags = parser.parse("", stderr);
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0].severity, DiagnosticSeverity::Error);
        assert_eq!(diags[1].severity, DiagnosticSeverity::Warning);
    }

    #[test]
    fn test_generic_parser_no_column() {
        let stderr = "main.c:15: implicit declaration of function\n";
        let parser = GenericDiagnosticParser {
            tool_name: "gcc".to_string(),
        };
        let diags = parser.parse("", stderr);
        assert_eq!(diags.len(), 1);
        let loc = diags[0].location.as_ref().unwrap();
        assert_eq!(loc.line, 15);
        assert!(loc.column.is_none());
    }
}

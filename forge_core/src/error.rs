//! Error types for ForgeKit.

use std::path::PathBuf;
use crate::types::Span;

/// Main error type for ForgeKit.
///
/// All Forge operations return `Result<T> = std::result::Result<T, ForgeError>`.
#[derive(thiserror::Error, Debug)]
pub enum ForgeError {
    /// Database operation failed.
    #[error("Database error: {0}")]
    DatabaseError(String),

    /// Symbol could not be found.
    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),

    /// Invalid query syntax or parameters.
    #[error("Invalid query: {0}")]
    InvalidQuery(String),

    /// Edit conflict detected.
    #[error("Edit conflict in {file:?} at {span:?}")]
    EditConflict {
        /// File containing the conflict
        file: PathBuf,
        /// Conflicting span
        span: Span,
    },

    /// Pre-commit verification failed.
    #[error("Verification failed: {0}")]
    VerificationFailed(String),

    /// Policy constraint violated.
    #[error("Policy violation: {0}")]
    PolicyViolation(String),

    /// Requested backend is not available.
    #[error("Backend not available: {0}")]
    BackendNotAvailable(String),

    /// CFG not available for the requested function.
    #[error("CFG not available for symbol: {0:?}")]
    CfgNotAvailable(crate::types::SymbolId),

    /// Path enumeration overflow (too many paths).
    #[error("Path overflow for symbol: {0:?}")]
    PathOverflow(crate::types::SymbolId),

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Error from underlying sqlitegraph.
    #[error("Graph error: {0}")]
    Graph(#[from] anyhow::Error),

    /// External tool execution error (Magellan, LLMGrep, etc.).
    #[error("Tool error: {0}")]
    ToolError(String),
}

/// Type alias for Result with ForgeError.
pub type Result<T> = std::result::Result<T, ForgeError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = ForgeError::SymbolNotFound("test".to_string());
        assert_eq!(err.to_string(), "Symbol not found: test");
    }

    #[test]
    fn test_span_is_empty() {
        let span = Span { start: 10, end: 10 };
        assert!(span.is_empty());
    }

    #[test]
    fn test_span_contains() {
        let span = Span { start: 10, end: 20 };
        assert!(span.contains(15));
        assert!(!span.contains(20));
        assert!(!span.contains(5));
    }
}

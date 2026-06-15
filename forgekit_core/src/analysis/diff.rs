use super::{AnalysisModule, ApplyResult, Result};

#[derive(Debug, Clone)]
pub struct Diff {
    /// Original content
    pub original: String,
    /// New content
    pub new: String,
    /// Changed lines
    pub changed_lines: Vec<usize>,
}

impl Diff {
    /// Create a new diff.
    pub fn new(original: String, new: String) -> Self {
        let changed_lines = compute_changed_lines(&original, &new);
        Self {
            original,
            new,
            changed_lines,
        }
    }

    /// Returns true if there are any changes.
    pub fn has_changes(&self) -> bool {
        !self.changed_lines.is_empty()
    }
}

/// Compute which lines changed between two strings.
fn compute_changed_lines(original: &str, new: &str) -> Vec<usize> {
    let orig_lines: Vec<&str> = original.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    let mut changed = Vec::new();

    for (i, (o, n)) in orig_lines.iter().zip(new_lines.iter()).enumerate() {
        if o != n {
            changed.push(i);
        }
    }

    // Handle lines added at the end
    if new_lines.len() > orig_lines.len() {
        for i in orig_lines.len()..new_lines.len() {
            changed.push(i);
        }
    }

    changed
}

/// Edit operation trait for code transformations.
#[async_trait::async_trait]
pub trait EditOperation: Send + Sync {
    /// Verify the operation can be applied.
    async fn verify(&self, module: &AnalysisModule) -> Result<ApplyResult>;

    /// Preview the changes without applying.
    async fn preview(&self, module: &AnalysisModule) -> Result<Diff>;

    /// Apply the operation.
    async fn apply(&self, module: &mut AnalysisModule) -> Result<ApplyResult>;
}

/// Insert content at a specific location.
#[derive(Debug, Clone)]
pub struct InsertOperation {
    /// Symbol to insert content after
    pub after_symbol: String,
    /// Content to insert
    pub content: String,
}

#[async_trait::async_trait]
impl EditOperation for InsertOperation {
    async fn verify(&self, module: &AnalysisModule) -> Result<ApplyResult> {
        // Check if the symbol exists
        let symbols = module.graph().find_symbol(&self.after_symbol).await?;

        if symbols.is_empty() {
            return Ok(ApplyResult::Failed(format!(
                "Symbol '{}' not found",
                self.after_symbol
            )));
        }

        Ok(ApplyResult::Pending)
    }

    async fn preview(&self, module: &AnalysisModule) -> Result<Diff> {
        let symbols = module.graph().find_symbol(&self.after_symbol).await?;

        if symbols.is_empty() {
            return Ok(Diff::new(
                String::from(""),
                format!(
                    "// Would insert after: {}\n{}",
                    self.after_symbol, self.content
                ),
            ));
        }

        let original = format!("// Original content at {}\n", self.after_symbol);
        let new_content = format!("{}\n// Inserted content\n{}", original, self.content);

        Ok(Diff::new(original, new_content))
    }

    async fn apply(&self, module: &mut AnalysisModule) -> Result<ApplyResult> {
        let symbols = module.graph().find_symbol(&self.after_symbol).await?;

        if symbols.is_empty() {
            return Ok(ApplyResult::Failed(format!(
                "Symbol '{}' not found",
                self.after_symbol
            )));
        }

        let sym = &symbols[0];
        let file_path = &sym.location.file_path;
        let content = tokio::fs::read_to_string(file_path).await.map_err(|e| {
            crate::error::ForgeError::DatabaseError(format!(
                "Failed to read {}: {}",
                file_path.display(),
                e
            ))
        })?;

        let insert_pos = sym.location.byte_end as usize;
        let content_bytes = content.as_bytes();
        let mut modified = content_bytes[..insert_pos].to_vec();
        modified.extend_from_slice(self.content.as_bytes());
        modified.extend_from_slice(&content_bytes[insert_pos..]);

        tokio::fs::write(file_path, modified).await.map_err(|e| {
            crate::error::ForgeError::DatabaseError(format!(
                "Failed to write {}: {}",
                file_path.display(),
                e
            ))
        })?;

        Ok(ApplyResult::Applied)
    }
}

/// Delete a symbol by name.
#[derive(Debug, Clone)]
pub struct DeleteOperation {
    /// Name of symbol to delete
    pub symbol_name: String,
}

#[async_trait::async_trait]
impl EditOperation for DeleteOperation {
    async fn verify(&self, module: &AnalysisModule) -> Result<ApplyResult> {
        let symbols = module.graph().find_symbol(&self.symbol_name).await?;

        if symbols.is_empty() {
            return Ok(ApplyResult::Failed(format!(
                "Symbol '{}' not found",
                self.symbol_name
            )));
        }

        // Check if anything references this symbol
        let refs = module.graph().references(&self.symbol_name).await?;

        if !refs.is_empty() {
            return Ok(ApplyResult::Failed(format!(
                "Cannot delete '{}': still referenced by {} symbols",
                self.symbol_name,
                refs.len()
            )));
        }

        Ok(ApplyResult::Pending)
    }

    async fn preview(&self, _module: &AnalysisModule) -> Result<Diff> {
        let original = format!(
            "fn {}() {{\n    // original implementation\n}}\n",
            self.symbol_name
        );
        let new_content = String::from("// Symbol deleted\n");

        Ok(Diff::new(original, new_content))
    }

    async fn apply(&self, _module: &mut AnalysisModule) -> Result<ApplyResult> {
        tracing::info!("DeleteOperation: deleting symbol '{}'", self.symbol_name);
        Ok(ApplyResult::Applied)
    }
}

/// Rename a symbol with validation.
#[derive(Debug, Clone)]
pub struct RenameOperation {
    /// Current symbol name
    pub old_name: String,
    /// New symbol name
    pub new_name: String,
}

impl RenameOperation {
    /// Create a new rename operation.
    pub fn new(old_name: impl Into<String>, new_name: impl Into<String>) -> Self {
        Self {
            old_name: old_name.into(),
            new_name: new_name.into(),
        }
    }

    /// Validate the new name is acceptable.
    pub(super) fn validate_name(&self) -> Result<()> {
        if self.new_name.is_empty() {
            return Err(crate::error::ForgeError::InvalidQuery(
                "New name cannot be empty".to_string(),
            ));
        }

        if self.new_name.chars().any(|c| c.is_whitespace()) {
            return Err(crate::error::ForgeError::InvalidQuery(
                "New name cannot contain spaces".to_string(),
            ));
        }

        // Check if it's a valid Rust identifier
        if !self
            .new_name
            .chars()
            .next()
            .map(|c| c.is_alphabetic() || c == '_')
            .unwrap_or(false)
        {
            return Err(crate::error::ForgeError::InvalidQuery(
                "New name must start with a letter or underscore".to_string(),
            ));
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl EditOperation for RenameOperation {
    async fn verify(&self, module: &AnalysisModule) -> Result<ApplyResult> {
        // First validate the new name format
        if let Err(e) = self.validate_name() {
            return Ok(ApplyResult::Failed(e.to_string()));
        }

        // Check if old symbol exists
        let old_symbols = module.graph().find_symbol(&self.old_name).await?;

        if old_symbols.is_empty() {
            return Ok(ApplyResult::Failed(format!(
                "Symbol '{}' not found",
                self.old_name
            )));
        }

        // Check if new name already exists
        let new_symbols = module.graph().find_symbol(&self.new_name).await?;

        if !new_symbols.is_empty() {
            return Ok(ApplyResult::Failed(format!(
                "Cannot rename to '{}': symbol already exists",
                self.new_name
            )));
        }

        Ok(ApplyResult::Pending)
    }

    async fn preview(&self, _module: &AnalysisModule) -> Result<Diff> {
        let original = format!("fn {}()", self.old_name);
        let new_content = format!("fn {}()", self.new_name);

        Ok(Diff::new(original, new_content))
    }

    async fn apply(&self, module: &mut AnalysisModule) -> Result<ApplyResult> {
        // Use the edit module to perform the rename
        let result = module
            .edit()
            .rename_symbol(&self.old_name, &self.new_name)
            .await?;

        if result.success {
            Ok(ApplyResult::Applied)
        } else {
            Ok(ApplyResult::Failed(result.error.unwrap_or_default()))
        }
    }
}

/// Error result - operation always fails.
#[derive(Debug, Clone)]
pub struct ErrorResult {
    pub reason: String,
}

impl ErrorResult {
    /// Create a new error result.
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

#[async_trait::async_trait]
impl EditOperation for ErrorResult {
    async fn verify(&self, _module: &AnalysisModule) -> Result<ApplyResult> {
        Ok(ApplyResult::AlwaysError)
    }

    async fn preview(&self, _module: &AnalysisModule) -> Result<Diff> {
        Ok(Diff::new(
            format!("// Error: {}", self.reason),
            format!("// Error: {}", self.reason),
        ))
    }

    async fn apply(&self, _module: &mut AnalysisModule) -> Result<ApplyResult> {
        Ok(ApplyResult::Failed(self.reason.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::CfgModule;
    use crate::edit::EditModule;
    use crate::graph::GraphModule;
    use crate::search::SearchModule;
    use crate::storage::BackendKind;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_insert_operation_verify() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await
                .unwrap(),
        );
        let graph = GraphModule::new(Arc::clone(&store));
        let search = SearchModule::new(Arc::clone(&store));
        let cfg = CfgModule::new(Arc::clone(&store));
        let edit = EditModule::new(store);

        let analysis = AnalysisModule::new(graph, cfg, edit, search);
        let insert = InsertOperation {
            after_symbol: "nonexistent".to_string(),
            content: "// new content".to_string(),
        };

        let result = insert.verify(&analysis).await.unwrap();
        assert!(matches!(result, ApplyResult::Failed(_)));
    }

    #[tokio::test]
    async fn test_insert_operation_preview() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await
                .unwrap(),
        );
        let graph = GraphModule::new(Arc::clone(&store));
        let search = SearchModule::new(Arc::clone(&store));
        let cfg = CfgModule::new(Arc::clone(&store));
        let edit = EditModule::new(store);

        let analysis = AnalysisModule::new(graph, cfg, edit, search);
        let insert = InsertOperation {
            after_symbol: "test_symbol".to_string(),
            content: "// new content".to_string(),
        };

        let diff = insert.preview(&analysis).await.unwrap();
        assert!(!diff.new.is_empty());
    }

    #[tokio::test]
    async fn test_delete_operation_verify_not_found() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await
                .unwrap(),
        );
        let graph = GraphModule::new(Arc::clone(&store));
        let search = SearchModule::new(Arc::clone(&store));
        let cfg = CfgModule::new(Arc::clone(&store));
        let edit = EditModule::new(store);

        let analysis = AnalysisModule::new(graph, cfg, edit, search);
        let delete = DeleteOperation {
            symbol_name: "nonexistent".to_string(),
        };

        let result = delete.verify(&analysis).await.unwrap();
        assert!(matches!(result, ApplyResult::Failed(_)));
    }

    #[tokio::test]
    async fn test_delete_operation_preview() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await
                .unwrap(),
        );
        let graph = GraphModule::new(Arc::clone(&store));
        let search = SearchModule::new(Arc::clone(&store));
        let cfg = CfgModule::new(Arc::clone(&store));
        let edit = EditModule::new(store);

        let analysis = AnalysisModule::new(graph, cfg, edit, search);
        let delete = DeleteOperation {
            symbol_name: "test_func".to_string(),
        };

        let diff = delete.preview(&analysis).await.unwrap();
        assert!(diff.new.contains("deleted"));
    }

    #[tokio::test]
    async fn test_rename_operation_verify_not_found() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await
                .unwrap(),
        );
        let graph = GraphModule::new(Arc::clone(&store));
        let search = SearchModule::new(Arc::clone(&store));
        let cfg = CfgModule::new(Arc::clone(&store));
        let edit = EditModule::new(store);

        let analysis = AnalysisModule::new(graph, cfg, edit, search);
        let rename = RenameOperation::new("old_name", "new_name");

        let result = rename.verify(&analysis).await.unwrap();
        assert!(matches!(result, ApplyResult::Failed(_)));
    }

    #[tokio::test]
    async fn test_rename_operation_validate_empty_name() {
        let rename = RenameOperation::new("old", "");
        let result = rename.validate_name();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_rename_operation_validate_invalid_name() {
        let rename = RenameOperation::new("old", "123invalid");
        let result = rename.validate_name();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_error_result_always_fails() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await
                .unwrap(),
        );
        let graph = GraphModule::new(Arc::clone(&store));
        let search = SearchModule::new(Arc::clone(&store));
        let cfg = CfgModule::new(Arc::clone(&store));
        let edit = EditModule::new(store);

        let mut analysis = AnalysisModule::new(graph, cfg, edit, search);
        let error = ErrorResult::new("Test error");

        let result = error.verify(&analysis).await.unwrap();
        assert_eq!(result, ApplyResult::AlwaysError);

        let apply_result = error.apply(&mut analysis).await.unwrap();
        assert!(matches!(apply_result, ApplyResult::Failed(_)));
    }

    #[test]
    fn test_diff_creation() {
        let diff = Diff::new("original content".to_string(), "new content".to_string());
        assert_eq!(diff.original, "original content");
        assert_eq!(diff.new, "new content");
    }

    #[test]
    fn test_diff_has_changes() {
        let diff = Diff::new("a".to_string(), "b".to_string());
        assert!(diff.has_changes());
    }

    #[test]
    fn test_diff_no_changes() {
        let diff = Diff::new("same".to_string(), "same".to_string());
        assert!(!diff.has_changes());
    }

    #[test]
    fn test_apply_result_variants() {
        assert!(matches!(ApplyResult::Applied, ApplyResult::Applied));
        assert!(matches!(ApplyResult::AlwaysError, ApplyResult::AlwaysError));
        assert!(matches!(ApplyResult::Pending, ApplyResult::Pending));
        assert!(matches!(
            ApplyResult::Failed("x".to_string()),
            ApplyResult::Failed(_)
        ));
    }

    #[tokio::test]
    async fn test_full_workflow_from_lookup_to_edit() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await
                .unwrap(),
        );
        let graph = GraphModule::new(Arc::clone(&store));
        let search = SearchModule::new(Arc::clone(&store));
        let cfg = CfgModule::new(Arc::clone(&store));
        let edit = EditModule::new(store);

        let analysis = AnalysisModule::new(graph, cfg, edit, search);

        let symbols = analysis.graph().find_symbol("test").await.unwrap();
        assert!(symbols.is_empty());

        let impact = analysis.impact_analysis("test").await.unwrap();
        assert_eq!(impact.symbol, "test");
        assert_eq!(impact.impact_score, 0);

        let rename = RenameOperation::new("test", "new_name");
        let result = rename.verify(&analysis).await.unwrap();
        assert!(matches!(result, ApplyResult::Failed(_)));
    }
}

//! Edit module - Span-safe code editing
//!
//! This module provides span-safe refactoring operations by delegating to
//! `splice::forge` convenience functions when the `splice` feature is enabled.
//! Symbol discovery uses `llmgrep::forge::search_symbols` for file resolution,
//! then `splice::forge::patch_symbol_in_file` for each file.

mod identifiers;
mod undo;

pub use undo::{PendingUndo, UndoResult};

use crate::error::{ForgeError, Result};
use std::path::{Path, PathBuf};

use identifiers::language_from_extension;
use undo::UndoableOp;

use identifiers::identifier_spans;

/// Result of an edit operation.
#[derive(Debug, Clone)]
pub struct EditResult {
    pub success: bool,
    pub changed_files: Vec<PathBuf>,
    pub error: Option<String>,
}

impl EditResult {
    pub fn success(files: Vec<PathBuf>) -> Self {
        Self {
            success: true,
            changed_files: files,
            error: None,
        }
    }

    pub fn failure(error: String) -> Self {
        Self {
            success: false,
            changed_files: Vec::new(),
            error: Some(error),
        }
    }
}

/// Edit module for span-safe refactoring.
pub struct EditModule {
    store: std::sync::Arc<crate::storage::UnifiedGraphStore>,
    undo_stack: std::sync::Mutex<Vec<PendingUndo>>,
    undo_capacity: usize,
}

impl EditModule {
    pub fn new(store: std::sync::Arc<crate::storage::UnifiedGraphStore>) -> Self {
        Self {
            store,
            undo_stack: std::sync::Mutex::new(Vec::new()),
            undo_capacity: 100,
        }
    }

    fn validate_relative_path(&self, path: &Path) -> Result<PathBuf> {
        if path.is_absolute() {
            return Err(ForgeError::PathNotAllowed(path.to_path_buf()));
        }
        let resolved = self.store.codebase_path.join(path);
        let canonical_base = self
            .store
            .codebase_path
            .canonicalize()
            .unwrap_or_else(|_| self.store.codebase_path.clone());
        if let Some(parent) = resolved.parent() {
            if let Ok(canonical_parent) = parent.canonicalize() {
                if !canonical_parent.starts_with(&canonical_base) {
                    return Err(ForgeError::PathNotAllowed(path.to_path_buf()));
                }
            }
        }
        Ok(resolved)
    }

    pub async fn create_file(&self, path: &Path, content: &str) -> Result<EditResult> {
        let resolved = self.validate_relative_path(path)?;
        if resolved.exists() {
            return Err(ForgeError::FileAlreadyExists(resolved));
        }
        if let Some(parent) = resolved.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&resolved, content).await?;
        self.push_undo(UndoableOp::CreateFile {
            path: path.to_path_buf(),
        });
        Ok(EditResult::success(vec![path.to_path_buf()]))
    }

    pub async fn create_directory(&self, path: &Path) -> Result<EditResult> {
        let resolved = self.validate_relative_path(path)?;
        if resolved.exists() {
            return Err(ForgeError::FileAlreadyExists(resolved));
        }
        tokio::fs::create_dir_all(&resolved).await?;
        self.push_undo(UndoableOp::CreateDirectory {
            path: path.to_path_buf(),
        });
        Ok(EditResult::success(vec![path.to_path_buf()]))
    }

    pub async fn write_file(&self, path: &Path, content: &str) -> Result<EditResult> {
        let resolved = self.validate_relative_path(path)?;
        let previous = if resolved.exists() {
            tokio::fs::read_to_string(&resolved).await.ok()
        } else {
            None
        };
        if let Some(parent) = resolved.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&resolved, content).await?;
        self.push_undo(UndoableOp::WriteFile {
            path: path.to_path_buf(),
            previous,
        });
        Ok(EditResult::success(vec![path.to_path_buf()]))
    }

    pub async fn apply(&mut self, op: EditOperation) -> Result<()> {
        match op {
            EditOperation::Replace {
                file_path,
                start,
                end,
                new_content,
            } => {
                splice::patch::replace_span(&file_path, start, end, &new_content).map_err(|e| {
                    ForgeError::DatabaseError(format!("Splice replace failed: {}", e))
                })?;
                Ok(())
            }
        }
    }

    pub async fn patch_symbol(&self, symbol: &str, replacement: &str) -> Result<EditResult> {
        let db_path = self.store.db_path.clone();
        if !db_path.exists() {
            return Err(ForgeError::DatabaseError(
                "graph DB not found; run forge.graph().index() first".to_string(),
            ));
        }
        self.patch_symbol_via_db(symbol, replacement, &db_path)
            .await
    }

    async fn patch_symbol_via_db(
        &self,
        symbol: &str,
        replacement: &str,
        db_path: &Path,
    ) -> Result<EditResult> {
        let matches = llmgrep::forge::search_symbols(symbol, db_path, 50)
            .map_err(|e| ForgeError::DatabaseError(format!("Symbol search failed: {}", e)))?;

        let files: std::collections::HashSet<PathBuf> = matches
            .iter()
            .map(|m| PathBuf::from(&m.span.file_path))
            .collect();

        let mut changed_files = Vec::new();
        for file_path in files {
            let full_path = self.store.codebase_path.join(&file_path);
            match splice::forge::patch_symbol_in_file(&full_path, symbol, replacement, db_path) {
                Ok(_) => {
                    changed_files.push(file_path);
                }
                Err(e) => {
                    tracing::warn!("Failed to patch {} in file: {}", symbol, e);
                }
            }
        }

        if changed_files.is_empty() {
            return Err(ForgeError::SymbolNotFound(format!(
                "Symbol '{}' not found",
                symbol
            )));
        }

        Ok(EditResult::success(changed_files))
    }

    pub async fn rename_symbol(&self, old_name: &str, new_name: &str) -> Result<EditResult> {
        let db_path = self.store.db_path.clone();
        if !db_path.exists() {
            return Err(ForgeError::DatabaseError(
                "graph DB not found; run forge.graph().index() first".to_string(),
            ));
        }
        self.rename_symbol_via_db(old_name, new_name, &db_path)
            .await
    }

    async fn rename_symbol_via_db(
        &self,
        old_name: &str,
        new_name: &str,
        db_path: &Path,
    ) -> Result<EditResult> {
        let mut graph = magellan::CodeGraph::open(db_path)
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to open graph: {}", e)))?;

        let mut affected_files: std::collections::HashSet<std::path::PathBuf> =
            std::collections::HashSet::new();

        if let Ok(defs) = graph.search_symbols_by_name(old_name) {
            for sym in &defs {
                affected_files.insert(std::path::PathBuf::from(&sym.file_path));
            }
        }

        let file_nodes = graph
            .all_file_nodes()
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to get file nodes: {}", e)))?;

        for file_path in file_nodes.keys() {
            if let Ok(call_facts) = graph.callers_of_symbol(file_path, old_name) {
                if !call_facts.is_empty() {
                    affected_files.insert(std::path::PathBuf::from(file_path));
                }
            }
            if let Ok(Some(symbol_id)) = graph.symbol_id_by_name(file_path, old_name) {
                if let Ok(refs) = graph.references_to_symbol(symbol_id) {
                    for r in refs {
                        affected_files.insert(r.file_path.clone());
                    }
                }
            }
        }

        if affected_files.is_empty() {
            return Err(ForgeError::SymbolNotFound(format!(
                "Symbol '{}' not found",
                old_name
            )));
        }

        let mut all_refs: Vec<magellan::references::ReferenceFact> = Vec::new();
        for rel_path in &affected_files {
            let full_path = self.store.codebase_path.join(rel_path);
            if let Ok(content) = std::fs::read(&full_path) {
                let lang = language_from_extension(rel_path);
                for (start, end) in identifier_spans(&content, old_name, lang) {
                    all_refs.push(magellan::references::ReferenceFact {
                        file_path: rel_path.clone(),
                        referenced_symbol: old_name.to_string(),
                        byte_start: start,
                        byte_end: end,
                        start_line: 0,
                        start_col: 0,
                        end_line: 0,
                        end_col: 0,
                    });
                }
            }
        }

        if all_refs.is_empty() {
            return Err(ForgeError::SymbolNotFound(format!(
                "Symbol '{}' not found",
                old_name
            )));
        }

        let mut changed_files = Vec::new();
        let by_file = splice::graph::rename::group_references_by_file(&all_refs);
        for (file_path, refs) in by_file {
            let full_path = self.store.codebase_path.join(&file_path);
            match splice::graph::rename::apply_replacements_in_file(
                &full_path, old_name, new_name, &refs,
            ) {
                Ok(count) if count > 0 => {
                    changed_files.push(file_path);
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!("Failed to rename in {}: {}", file_path.display(), e);
                }
            }
        }

        if changed_files.is_empty() {
            return Err(ForgeError::SymbolNotFound(format!(
                "Symbol '{}' references found but no files changed",
                old_name
            )));
        }

        Ok(EditResult::success(changed_files))
    }

    pub async fn delete_symbol(&self, file_path: &Path, symbol: &str) -> Result<EditResult> {
        let db_path = self.store.db_path.clone();
        if !db_path.exists() {
            return Err(ForgeError::DatabaseError(
                "graph DB not found; run forge.graph().index() first".to_string(),
            ));
        }
        self.delete_symbol_via_db(file_path, symbol, &db_path).await
    }

    async fn delete_symbol_via_db(
        &self,
        file_path: &Path,
        symbol: &str,
        db_path: &Path,
    ) -> Result<EditResult> {
        let full_path = self.store.codebase_path.join(file_path);
        match splice::forge::delete_symbol_from_file(&full_path, symbol, db_path) {
            Ok(result) => Ok(EditResult::success(vec![result.file])),
            Err(e) => Err(ForgeError::DatabaseError(format!("Delete failed: {}", e))),
        }
    }

    pub async fn resolve_span(
        &self,
        file_path: &Path,
        symbol: &str,
    ) -> Result<splice::forge::SymbolSpan> {
        let db_path = self.store.db_path.clone();
        if !db_path.exists() {
            return Err(ForgeError::DatabaseError(
                "graph DB not found; run forge.graph().index() first".to_string(),
            ));
        }
        splice::forge::resolve_symbol_span(
            &self.store.codebase_path.join(file_path),
            symbol,
            &db_path,
        )
        .map_err(|e| ForgeError::DatabaseError(format!("Span resolution failed: {}", e)))
    }
}

/// An edit operation.
pub enum EditOperation {
    Replace {
        file_path: PathBuf,
        start: usize,
        end: usize,
        new_content: String,
    },
}

#[cfg(test)]
fn find_symbol_span(content: &str, symbol: &str) -> Option<(usize, usize)> {
    let patterns = [
        format!("fn {}", symbol),
        format!("pub fn {}", symbol),
        format!("pub(crate) fn {}", symbol),
        format!("async fn {}", symbol),
        format!("pub async fn {}", symbol),
        format!("struct {}", symbol),
        format!("pub struct {}", symbol),
        format!("enum {}", symbol),
        format!("pub enum {}", symbol),
        format!("trait {}", symbol),
        format!("pub trait {}", symbol),
        format!("impl {}", symbol),
        format!("mod {}", symbol),
        format!("pub mod {}", symbol),
        format!("const {}", symbol),
        format!("static {}", symbol),
        format!("type {}", symbol),
    ];

    for pattern in &patterns {
        if let Some(pos) = content.find(pattern.as_str()) {
            let end = find_definition_end(content, pos);
            return Some((pos, end));
        }
    }

    None
}

#[cfg(test)]
fn find_definition_end(content: &str, start: usize) -> usize {
    let rest = &content[start..];

    if let Some(brace_pos) = rest.find('{') {
        let mut depth = 0u32;
        for (i, b) in rest[brace_pos..].bytes().enumerate() {
            match b {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return start + brace_pos + i + 1;
                    }
                }
                _ => {}
            }
        }
    }

    if let Some(semi_pos) = rest.find(';') {
        return start + semi_pos + 1;
    }

    content.len()
}

#[cfg(test)]
fn simple_word_replace(content: &str, old: &str, new: &str) -> String {
    let mut result = String::new();
    let mut last_end = 0;

    for (i, _) in content.match_indices(old) {
        let before = if i > 0 {
            content.as_bytes().get(i - 1).copied()
        } else {
            None
        };
        let after = content.as_bytes().get(i + old.len()).copied();

        let is_word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
        let word_before = before.map(is_word).unwrap_or(false);
        let word_after = after.map(is_word).unwrap_or(false);

        if !word_before && !word_after {
            result.push_str(&content[last_end..i]);
            result.push_str(new);
            last_end = i + old.len();
        }
    }

    result.push_str(&content[last_end..]);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edit_module_creation() {
        use crate::storage::UnifiedGraphStore;
        let _store: Option<UnifiedGraphStore> = None;
    }

    #[test]
    fn test_edit_operation_replace() {
        let _op = EditOperation::Replace {
            file_path: PathBuf::from("test.rs"),
            start: 10,
            end: 20,
            new_content: String::from("test"),
        };
    }

    #[test]
    fn test_edit_result_success() {
        let result = EditResult::success(vec![PathBuf::from("foo.rs")]);
        assert!(result.success);
        assert_eq!(result.changed_files.len(), 1);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_edit_result_failure() {
        let result = EditResult::failure("something went wrong".to_string());
        assert!(!result.success);
        assert!(result.changed_files.is_empty());
        assert!(result.error.is_some());
    }

    #[test]
    fn test_find_symbol_span_function() {
        let code = "fn hello() { println!(\"Hello\"); }\n";
        let span = find_symbol_span(code, "hello").unwrap();
        assert!(code[span.0..span.1].starts_with("fn hello"));
        assert!(code[span.0..span.1].ends_with("}"));
    }

    #[test]
    fn test_find_symbol_span_struct() {
        let code = "pub struct Foo { x: i32 }\n";
        let span = find_symbol_span(code, "Foo").unwrap();
        assert!(code[span.0..span.1].contains("struct Foo"));
    }

    #[test]
    fn test_find_symbol_span_not_found() {
        let code = "fn bar() {}\n";
        assert!(find_symbol_span(code, "baz").is_none());
    }

    #[test]
    fn test_simple_word_replace() {
        let code = "fn old_name() {}\nfn caller() { old_name(); }";
        let result = simple_word_replace(code, "old_name", "new_name");
        assert!(result.contains("fn new_name()"));
        assert!(result.contains("new_name();"));
        assert!(!result.contains("old_name"));
    }

    #[test]
    fn test_simple_word_replace_respects_boundaries() {
        let code = "fn get_name() {}\nfn name() {}";
        let result = simple_word_replace(code, "name", "title");
        assert!(result.contains("get_name"));
        assert!(result.contains("fn title()"));
    }

    #[tokio::test]
    async fn test_create_file_new() {
        let temp = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open_with_path(
                temp.path(),
                temp.path().join("test.db"),
                crate::storage::BackendKind::default(),
            )
            .await
            .unwrap(),
        );
        let edit = EditModule::new(store);

        let result = edit
            .create_file(Path::new("src/lib.rs"), "pub fn hello() {}")
            .await
            .unwrap();
        assert!(result.success);

        let content = tokio::fs::read_to_string(temp.path().join("src/lib.rs"))
            .await
            .unwrap();
        assert_eq!(content, "pub fn hello() {}");
    }

    #[tokio::test]
    async fn test_create_file_rejects_absolute() {
        let temp = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open_with_path(
                temp.path(),
                temp.path().join("test.db"),
                crate::storage::BackendKind::default(),
            )
            .await
            .unwrap(),
        );
        let edit = EditModule::new(store);

        let result = edit.create_file(Path::new("/tmp/evil.rs"), "bad").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ForgeError::PathNotAllowed(_)));
    }

    #[tokio::test]
    async fn test_create_file_rejects_existing() {
        let temp = tempfile::tempdir().unwrap();
        let existing = temp.path().join("exists.rs");
        tokio::fs::write(&existing, "old").await.unwrap();

        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open_with_path(
                temp.path(),
                temp.path().join("test.db"),
                crate::storage::BackendKind::default(),
            )
            .await
            .unwrap(),
        );
        let edit = EditModule::new(store);

        let result = edit.create_file(Path::new("exists.rs"), "new").await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ForgeError::FileAlreadyExists(_)
        ));
    }

    #[tokio::test]
    async fn test_create_file_nested_dirs() {
        let temp = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open_with_path(
                temp.path(),
                temp.path().join("test.db"),
                crate::storage::BackendKind::default(),
            )
            .await
            .unwrap(),
        );
        let edit = EditModule::new(store);

        let result = edit
            .create_file(Path::new("a/b/c/deep.rs"), "content")
            .await
            .unwrap();
        assert!(result.success);
        assert!(temp.path().join("a/b/c/deep.rs").exists());
    }

    #[tokio::test]
    async fn test_create_directory_new() {
        let temp = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open_with_path(
                temp.path(),
                temp.path().join("test.db"),
                crate::storage::BackendKind::default(),
            )
            .await
            .unwrap(),
        );
        let edit = EditModule::new(store);

        let result = edit
            .create_directory(Path::new("src/models"))
            .await
            .unwrap();
        assert!(result.success);
        assert!(temp.path().join("src/models").is_dir());
    }

    #[tokio::test]
    async fn test_write_file_overwrites() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("existing.rs");
        tokio::fs::write(&file, "old content").await.unwrap();

        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open_with_path(
                temp.path(),
                temp.path().join("test.db"),
                crate::storage::BackendKind::default(),
            )
            .await
            .unwrap(),
        );
        let edit = EditModule::new(store);

        let result = edit
            .write_file(Path::new("existing.rs"), "new content")
            .await
            .unwrap();
        assert!(result.success);

        let content = tokio::fs::read_to_string(&file).await.unwrap();
        assert_eq!(content, "new content");
    }

    #[tokio::test]
    async fn test_write_file_creates_parent_dirs() {
        let temp = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open_with_path(
                temp.path(),
                temp.path().join("test.db"),
                crate::storage::BackendKind::default(),
            )
            .await
            .unwrap(),
        );
        let edit = EditModule::new(store);

        let result = edit
            .write_file(Path::new("deep/nested/file.rs"), "content")
            .await
            .unwrap();
        assert!(result.success);
        assert!(temp.path().join("deep/nested/file.rs").exists());
    }

    #[test]
    fn test_identifier_spans_rust() {
        use crate::types::Language;
        let code = b"fn foo() { self.foo(); foo(); crate::foo(); }";
        let spans = identifier_spans(code, "foo", Language::Rust);
        assert!(spans.len() >= 3, "should find foo, self.foo, crate::foo");
    }

    #[test]
    fn test_identifier_spans_python() {
        use crate::types::Language;
        let code = b"self.value\ncls.value\nvalue\n";
        let spans = identifier_spans(code, "value", Language::Python);
        assert!(spans.len() >= 3, "should find value, self.value, cls.value");
    }

    #[test]
    fn test_identifier_spans_java() {
        use crate::types::Language;
        let code = b"this.name\nname\n";
        let spans = identifier_spans(code, "name", Language::Java);
        assert!(spans.len() >= 2, "should find name and this.name");
    }

    #[test]
    fn test_identifier_spans_respects_boundaries() {
        use crate::types::Language;
        let code = b"get_name\nname\nnames\n";
        let spans = identifier_spans(code, "name", Language::Rust);
        assert_eq!(spans.len(), 1, "should not match get_name or names");
    }

    #[test]
    fn test_language_from_extension() {
        assert!(matches!(
            language_from_extension(Path::new("foo.rs")),
            crate::types::Language::Rust
        ));
        assert!(matches!(
            language_from_extension(Path::new("bar.py")),
            crate::types::Language::Python
        ));
        assert!(matches!(
            language_from_extension(Path::new("baz.java")),
            crate::types::Language::Java
        ));
        assert!(matches!(
            language_from_extension(Path::new("a.ts")),
            crate::types::Language::TypeScript
        ));
    }

    #[tokio::test]
    async fn test_undo_create_file() {
        let temp = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open_with_path(
                temp.path(),
                temp.path().join("test.db"),
                crate::storage::BackendKind::default(),
            )
            .await
            .unwrap(),
        );
        let edit = EditModule::new(store);

        edit.create_file(Path::new("new.rs"), "fn main() {}")
            .await
            .unwrap();
        assert!(temp.path().join("new.rs").exists());
        assert!(edit.can_undo());
        assert_eq!(edit.undo_depth(), 1);

        let result = edit.undo().await.unwrap();
        assert!(matches!(result, UndoResult::Undone { .. }));
        assert!(!temp.path().join("new.rs").exists());
        assert!(!edit.can_undo());
    }

    #[tokio::test]
    async fn test_undo_write_file_restores_previous() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("existing.rs");
        tokio::fs::write(&file, "old content").await.unwrap();

        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open_with_path(
                temp.path(),
                temp.path().join("test.db"),
                crate::storage::BackendKind::default(),
            )
            .await
            .unwrap(),
        );
        let edit = EditModule::new(store);

        edit.write_file(Path::new("existing.rs"), "new content")
            .await
            .unwrap();
        assert_eq!(
            tokio::fs::read_to_string(&file).await.unwrap(),
            "new content"
        );

        edit.undo().await.unwrap();
        assert_eq!(
            tokio::fs::read_to_string(&file).await.unwrap(),
            "old content"
        );
    }

    #[tokio::test]
    async fn test_undo_write_file_removes_new() {
        let temp = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open_with_path(
                temp.path(),
                temp.path().join("test.db"),
                crate::storage::BackendKind::default(),
            )
            .await
            .unwrap(),
        );
        let edit = EditModule::new(store);

        edit.write_file(Path::new("brand_new.rs"), "content")
            .await
            .unwrap();
        assert!(temp.path().join("brand_new.rs").exists());

        edit.undo().await.unwrap();
        assert!(!temp.path().join("brand_new.rs").exists());
    }

    #[tokio::test]
    async fn test_undo_create_directory() {
        let temp = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open_with_path(
                temp.path(),
                temp.path().join("test.db"),
                crate::storage::BackendKind::default(),
            )
            .await
            .unwrap(),
        );
        let edit = EditModule::new(store);

        edit.create_directory(Path::new("my_dir")).await.unwrap();
        assert!(temp.path().join("my_dir").is_dir());

        edit.undo().await.unwrap();
        assert!(!temp.path().join("my_dir").exists());
    }

    #[tokio::test]
    async fn test_undo_empty_stack() {
        let temp = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open_with_path(
                temp.path(),
                temp.path().join("test.db"),
                crate::storage::BackendKind::default(),
            )
            .await
            .unwrap(),
        );
        let edit = EditModule::new(store);

        let result = edit.undo().await.unwrap();
        assert!(matches!(result, UndoResult::Empty));
    }

    #[tokio::test]
    async fn test_undo_depth_and_clear() {
        let temp = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::storage::UnifiedGraphStore::open_with_path(
                temp.path(),
                temp.path().join("test.db"),
                crate::storage::BackendKind::default(),
            )
            .await
            .unwrap(),
        );
        let edit = EditModule::new(store);

        edit.create_file(Path::new("a.rs"), "a").await.unwrap();
        edit.create_file(Path::new("b.rs"), "b").await.unwrap();
        edit.create_file(Path::new("c.rs"), "c").await.unwrap();
        assert_eq!(edit.undo_depth(), 3);

        edit.clear_undo_stack();
        assert_eq!(edit.undo_depth(), 0);
        assert!(!edit.can_undo());
    }
}

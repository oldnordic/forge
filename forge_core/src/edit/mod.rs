//! Edit module - Span-safe code editing
//!
//! This module provides span-safe refactoring operations via Splice integration.
//! Symbol resolution is done through Magellan; actual edits through Splice.

use crate::error::{ForgeError, Result};
use std::path::{Path, PathBuf};

/// Result of an edit operation.
#[derive(Debug, Clone)]
pub struct EditResult {
    /// Whether the operation succeeded
    pub success: bool,
    /// Files that were changed
    pub changed_files: Vec<PathBuf>,
    /// Optional error message
    pub error: Option<String>,
}

impl EditResult {
    /// Creates a successful result.
    pub fn success(files: Vec<PathBuf>) -> Self {
        Self {
            success: true,
            changed_files: files,
            error: None,
        }
    }

    /// Creates a failed result.
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
}

impl EditModule {
    /// Create a new EditModule.
    pub fn new(store: std::sync::Arc<crate::storage::UnifiedGraphStore>) -> Self {
        Self { store }
    }

    /// Apply an edit operation using splice.
    pub async fn apply(&mut self, op: EditOperation) -> Result<()> {
        match op {
            EditOperation::Replace {
                file_path,
                start,
                end,
                new_content,
            } => {
                #[cfg(feature = "splice")]
                {
                    splice::patch::replace_span(&file_path, start, end, &new_content).map_err(
                        |e| ForgeError::DatabaseError(format!("Splice replace failed: {}", e)),
                    )?;
                    Ok(())
                }
                #[cfg(not(feature = "splice"))]
                {
                    let _ = (file_path, start, end, new_content);
                    Err(ForgeError::DatabaseError(
                        "splice feature not enabled".to_string(),
                    ))
                }
            }
        }
    }

    /// Patches a symbol with new content.
    ///
    /// Uses magellan to find the symbol's byte span, then splice to apply
    /// the replacement with validation. Requires graph.db to exist.
    pub async fn patch_symbol(&self, symbol: &str, replacement: &str) -> Result<EditResult> {
        let db_path = self.store.db_path.join("graph.db");
        if !db_path.exists() {
            return Err(ForgeError::DatabaseError(
                "graph.db not found; run forge.graph().index() first".to_string(),
            ));
        }
        self.patch_symbol_via_db(symbol, replacement, &db_path).await
    }

    /// Patch using magellan DB for precise symbol resolution.
    #[cfg(feature = "magellan")]
    async fn patch_symbol_via_db(
        &self,
        symbol: &str,
        replacement: &str,
        db_path: &Path,
    ) -> Result<EditResult> {
        let mut graph = magellan::CodeGraph::open(db_path)
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to open graph: {}", e)))?;

        let file_nodes = graph
            .all_file_nodes()
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to get file nodes: {}", e)))?;

        let mut changed_files = Vec::new();

        for (file_path, _file_node) in file_nodes {
            let symbols = graph
                .symbols_in_file(&file_path)
                .map_err(|e| ForgeError::DatabaseError(format!("Failed to get symbols: {}", e)))?;

            for sym in symbols {
                if sym.name.as_deref() == Some(symbol) {
                    let full_path = self.store.codebase_path.join(&file_path);

                    #[cfg(feature = "splice")]
                    {
                        match splice::patch::replace_span(
                            &full_path,
                            sym.byte_start,
                            sym.byte_end,
                            replacement,
                        ) {
                            Ok(_) => {
                                changed_files.push(PathBuf::from(&file_path));
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to patch {} in {}: {}",
                                    symbol,
                                    file_path,
                                    e
                                );
                            }
                        }
                    }

                    #[cfg(not(feature = "splice"))]
                    {
                        let _ = (full_path, file_path);
                        return Err(ForgeError::DatabaseError(
                            "splice feature not enabled".to_string(),
                        ));
                    }
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

    #[cfg(not(feature = "magellan"))]
    async fn patch_symbol_via_db(
        &self,
        _symbol: &str,
        _replacement: &str,
        _db_path: &Path,
    ) -> Result<EditResult> {
        Err(ForgeError::DatabaseError(
            "magellan feature not enabled".to_string(),
        ))
    }

    /// Patch by scanning files in the codebase directory recursively.
    async fn patch_symbol_via_files(&self, symbol: &str, replacement: &str) -> Result<EditResult> {
        let codebase = &self.store.codebase_path;
        let mut changed_files = Vec::new();
        let mut files = Vec::new();
        collect_files_recursive(codebase, &mut files).await;

        for path in files {
            let content = match tokio::fs::read_to_string(&path).await {
                Ok(c) => c,
                Err(_) => continue,
            };

            if let Some(span) = find_symbol_span(&content, symbol) {
                let mut modified = content.as_bytes()[..span.0].to_vec();
                modified.extend_from_slice(replacement.as_bytes());
                modified.extend_from_slice(&content.as_bytes()[span.1..]);

                tokio::fs::write(&path, modified).await.map_err(|e| {
                    ForgeError::DatabaseError(format!("Failed to write file: {}", e))
                })?;

                changed_files.push(path.strip_prefix(codebase).unwrap_or(&path).to_path_buf());
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

    /// Renames a symbol and updates all references.
    ///
    /// Uses magellan to find all references to the symbol, then splice
    /// to apply span-safe replacements. Falls back to word-boundary
    /// replacement when no database is available.
    pub async fn rename_symbol(&self, old_name: &str, new_name: &str) -> Result<EditResult> {
        let db_path = self.store.db_path.join("graph.db");

        if db_path.exists() {
            match self
                .rename_symbol_via_db(old_name, new_name, &db_path)
                .await
            {
                Ok(result) => Ok(result),
                Err(ForgeError::SymbolNotFound(_)) => {
                    self.rename_symbol_via_files(old_name, new_name).await
                }
                Err(e) => Err(e),
            }
        } else {
            self.rename_symbol_via_files(old_name, new_name).await
        }
    }

    /// Rename using magellan DB for precise reference resolution.
    #[cfg(feature = "magellan")]
    async fn rename_symbol_via_db(
        &self,
        old_name: &str,
        new_name: &str,
        db_path: &Path,
    ) -> Result<EditResult> {
        let mut graph = magellan::CodeGraph::open(db_path)
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to open graph: {}", e)))?;

        let mut all_refs: Vec<magellan::references::ReferenceFact> = Vec::new();
        let file_nodes = graph
            .all_file_nodes()
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to get file nodes: {}", e)))?;

        for (file_path, _file_node) in file_nodes {
            if let Ok(Some(symbol_id)) = graph.symbol_id_by_name(&file_path, old_name) {
                if let Ok(refs) = graph.references_to_symbol(symbol_id) {
                    all_refs.extend(refs);
                }
            }
            if let Ok(call_facts) = graph.callers_of_symbol(&file_path, old_name) {
                for fact in call_facts {
                    all_refs.push(magellan::references::ReferenceFact {
                        file_path: fact.file_path,
                        referenced_symbol: fact.callee,
                        byte_start: fact.byte_start,
                        byte_end: fact.byte_end,
                        start_line: fact.start_line,
                        start_col: fact.start_col,
                        end_line: fact.end_line,
                        end_col: fact.end_col,
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

        #[cfg(feature = "splice")]
        {
            let by_file = splice::graph::rename::group_references_by_file(&all_refs);
            for (file_path, _refs) in by_file {
                let full_path = self.store.codebase_path.join(&file_path);
                match splice::graph::rename::apply_replacements_in_file(
                    &full_path, old_name, new_name, &all_refs,
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
        }

        #[cfg(not(feature = "splice"))]
        {
            let by_file = splice::graph::rename::group_references_by_file(&all_refs);
            for (file_path, _refs) in by_file {
                let full_path = self.store.codebase_path.join(&file_path);
                let content = tokio::fs::read_to_string(&full_path).await.map_err(|e| {
                    ForgeError::DatabaseError(format!("Failed to read file: {}", e))
                })?;
                let modified = simple_word_replace(&content, old_name, new_name);
                if modified != content {
                    tokio::fs::write(&full_path, modified).await.map_err(|e| {
                        ForgeError::DatabaseError(format!("Failed to write file: {}", e))
                    })?;
                    changed_files.push(file_path.into());
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

    #[cfg(not(feature = "magellan"))]
    async fn rename_symbol_via_db(
        &self,
        _old_name: &str,
        _new_name: &str,
        _db_path: &Path,
    ) -> Result<EditResult> {
        Err(ForgeError::DatabaseError(
            "magellan feature not enabled".to_string(),
        ))
    }

    /// Rename by scanning files recursively and doing word-boundary replacement.
    async fn rename_symbol_via_files(&self, old_name: &str, new_name: &str) -> Result<EditResult> {
        let codebase = &self.store.codebase_path;
        let mut changed_files = Vec::new();
        let mut found_any = false;
        let mut files = Vec::new();
        collect_files_recursive(codebase, &mut files).await;

        for path in files {
            let content = match tokio::fs::read_to_string(&path).await {
                Ok(c) => c,
                Err(_) => continue,
            };

            if !content.contains(old_name) {
                continue;
            }

            if find_symbol_span(&content, old_name).is_some() {
                found_any = true;
            }

            let modified = simple_word_replace(&content, old_name, new_name);
            if modified != content {
                tokio::fs::write(&path, modified).await.map_err(|e| {
                    ForgeError::DatabaseError(format!("Failed to write file: {}", e))
                })?;
                changed_files.push(path.strip_prefix(codebase).unwrap_or(&path).to_path_buf());
            }
        }

        if !found_any {
            return Err(ForgeError::SymbolNotFound(format!(
                "Symbol '{}' not found",
                old_name
            )));
        }

        Ok(EditResult::success(changed_files))
    }
}

/// Recursively collect all files under `dir`, skipping build artifacts.
async fn collect_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if matches!(
                    name,
                    "target" | ".git" | ".forge" | ".magellan" | "node_modules"
                ) {
                    continue;
                }
            }
            Box::pin(collect_files_recursive(&path, files)).await;
        } else if path.is_file() {
            files.push(path);
        }
    }
}

/// Find the byte span of a symbol definition in source code.
///
/// Looks for patterns like `fn name`, `struct name`, `enum name`, etc.
/// Returns (start, end) of the full definition.
fn find_symbol_span(content: &str, symbol: &str) -> Option<(usize, usize)> {
    // Match common definition patterns
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
            // Find the end of the definition (matching braces or semicolon)
            let end = find_definition_end(content, pos);
            return Some((pos, end));
        }
    }

    None
}

/// Find the end byte offset of a definition starting at `start`.
fn find_definition_end(content: &str, start: usize) -> usize {
    let rest = &content[start..];

    // For brace-delimited definitions, count braces
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

    // For non-brace definitions (e.g., type aliases, consts), find semicolon
    if let Some(semi_pos) = rest.find(';') {
        return start + semi_pos + 1;
    }

    content.len()
}

/// Word-boundary replacement preserving non-word characters around matches.
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

/// An edit operation.
pub enum EditOperation {
    /// Replace a span with new content.
    Replace {
        file_path: PathBuf,
        start: usize,
        end: usize,
        new_content: String,
    },
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
        // "get_name" should NOT be changed (word boundary before _)
        assert!(result.contains("get_name"));
        assert!(result.contains("fn title()"));
    }
}

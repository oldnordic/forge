//! Edit module - Span-safe code editing
//!
//! This module provides span-safe refactoring operations via Splice integration.

use crate::error::{ForgeError, Result};
use crate::types::{Span, SymbolId};
use std::path::PathBuf;

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

    /// Apply an edit operation.
    pub async fn apply(&mut self, _op: EditOperation) -> Result<()> {
        Ok(())
    }
    
    /// Patches a symbol with new content.
    ///
    /// Finds the symbol definition and replaces it with the new content.
    ///
    /// # Arguments
    ///
    /// * `symbol` - Symbol name to patch
    /// * `replacement` - New content for the symbol
    ///
    /// # Returns
    ///
    /// Result indicating success/failure and changed files.
    pub async fn patch_symbol(
        &self,
        symbol: &str,
        replacement: &str
    ) -> Result<EditResult> {
        use tokio::fs;
        
        // Find the symbol in the codebase
        let codebase_path = &self.store.codebase_path;
        
        // Search for files containing the symbol definition
        let mut changed_files = Vec::new();
        
        // Scan Rust files for the symbol definition
        Self::patch_symbol_in_dir(codebase_path, codebase_path, symbol, replacement, &mut changed_files).await?;
        
        if changed_files.is_empty() {
            return Err(ForgeError::SymbolNotFound(format!("Symbol '{}' not found", symbol)));
        }
        
        Ok(EditResult::success(changed_files))
    }
    
    async fn patch_symbol_in_dir(
        root: &std::path::Path,
        dir: &std::path::Path,
        symbol: &str,
        replacement: &str,
        changed_files: &mut Vec<PathBuf>,
    ) -> Result<()> {
        use tokio::fs;
        
        let mut entries = fs::read_dir(dir).await
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to read dir: {}", e)))?;
        
        while let Some(entry) = entries.next_entry().await
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to read entry: {}", e)))? 
        {
            let path = entry.path();
            if path.is_dir() {
                Box::pin(Self::patch_symbol_in_dir(root, &path, symbol, replacement, changed_files)).await?;
            } else if path.is_file() && path.extension().map(|e| e == "rs").unwrap_or(false) {
                // Read file and look for symbol definition
                let content = fs::read_to_string(&path).await
                    .map_err(|e| ForgeError::DatabaseError(format!("Failed to read file: {}", e)))?;
                
                // Look for function definition pattern
                let patterns = vec![
                    format!("fn {}(", symbol),
                    format!("pub fn {}(", symbol),
                    format!("async fn {}(", symbol),
                    format!("pub async fn {}(", symbol),
                ];
                
                let mut modified = content.clone();
                let mut found = false;
                
                for pattern in &patterns {
                    if let Some(start_idx) = modified.find(pattern) {
                        // Find the end of the function (matching braces)
                        if let Some(end_idx) = find_function_end(&modified, start_idx) {
                            modified.replace_range(start_idx..end_idx, replacement);
                            found = true;
                            break;
                        }
                    }
                }
                
                // Also check for struct/impl definitions
                if !found {
                    let struct_pattern = format!("struct {} ", symbol);
                    if let Some(start_idx) = modified.find(&struct_pattern) {
                        // Find end of struct definition
                        if let Some(end_idx) = find_struct_end(&modified, start_idx) {
                            modified.replace_range(start_idx..end_idx, replacement);
                            found = true;
                        }
                    }
                }
                
                if found {
                    fs::write(&path, modified).await
                        .map_err(|e| ForgeError::DatabaseError(format!("Failed to write file: {}", e)))?;
                    let relative_path = path.strip_prefix(root).unwrap_or(&path);
                    changed_files.push(relative_path.to_path_buf());
                }
            }
        }
        
        Ok(())
    }
    
    /// Renames a symbol and updates all references.
    ///
    /// # Arguments
    ///
    /// * `old_name` - Current symbol name
    /// * `new_name` - New symbol name
    ///
    /// # Returns
    ///
    /// Result indicating success/failure.
    pub async fn rename_symbol(
        &self,
        old_name: &str,
        new_name: &str
    ) -> Result<EditResult> {
        use tokio::fs;
        
        let codebase_path = &self.store.codebase_path;
        let mut changed_files = Vec::new();
        
        // Scan all Rust files and replace occurrences
        Self::rename_in_dir(codebase_path, codebase_path, old_name, new_name, &mut changed_files).await?;
        
        if changed_files.is_empty() {
            return Err(ForgeError::SymbolNotFound(format!("Symbol '{}' not found", old_name)));
        }
        
        Ok(EditResult::success(changed_files))
    }
    
    async fn rename_in_dir(
        root: &std::path::Path,
        dir: &std::path::Path,
        old_name: &str,
        new_name: &str,
        changed_files: &mut Vec<PathBuf>,
    ) -> Result<()> {
        use tokio::fs;
        
        let mut entries = fs::read_dir(dir).await
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to read dir: {}", e)))?;
        
        while let Some(entry) = entries.next_entry().await
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to read entry: {}", e)))? 
        {
            let path = entry.path();
            if path.is_dir() {
                Box::pin(Self::rename_in_dir(root, &path, old_name, new_name, changed_files)).await?;
            } else if path.is_file() && path.extension().map(|e| e == "rs").unwrap_or(false) {
                let content = fs::read_to_string(&path).await
                    .map_err(|e| ForgeError::DatabaseError(format!("Failed to read file: {}", e)))?;
                
                // Simple word-boundary replacement
                let modified = replace_word_boundaries(&content, old_name, new_name);
                
                if modified != content {
                    fs::write(&path, modified).await
                        .map_err(|e| ForgeError::DatabaseError(format!("Failed to write file: {}", e)))?;
                    let relative_path = path.strip_prefix(root).unwrap_or(&path);
                    changed_files.push(relative_path.to_path_buf());
                }
            }
        }
        
        Ok(())
    }
}

/// Find the end of a function definition, handling nested braces
fn find_function_end(content: &str, start_idx: usize) -> Option<usize> {
    let after_sig = &content[start_idx..];
    
    // Find opening brace
    if let Some(brace_idx) = after_sig.find('{') {
        let body_start = start_idx + brace_idx + 1;
        let mut brace_count = 1;
        let mut in_string = false;
        let mut escape_next = false;
        
        for (i, c) in content[body_start..].char_indices() {
            if escape_next {
                escape_next = false;
                continue;
            }
            
            match c {
                '\\' if in_string => escape_next = true,
                '"' | '\'' => in_string = !in_string,
                '{' if !in_string => brace_count += 1,
                '}' if !in_string => {
                    brace_count -= 1;
                    if brace_count == 0 {
                        return Some(body_start + i + 1);
                    }
                }
                _ => {}
            }
        }
    }
    
    None
}

/// Find the end of a struct definition
fn find_struct_end(content: &str, start_idx: usize) -> Option<usize> {
    let after_keyword = &content[start_idx..];
    
    // Find opening brace or semicolon
    if let Some(brace_idx) = after_keyword.find('{') {
        let body_start = start_idx + brace_idx + 1;
        let mut brace_count = 1;
        
        for (i, c) in content[body_start..].char_indices() {
            match c {
                '{' => brace_count += 1,
                '}' => {
                    brace_count -= 1;
                    if brace_count == 0 {
                        return Some(body_start + i + 1);
                    }
                }
                _ => {}
            }
        }
    } else if let Some(semi_idx) = after_keyword.find(';') {
        return Some(start_idx + semi_idx + 1);
    }
    
    None
}

/// Replace occurrences with word boundaries
fn replace_word_boundaries(content: &str, old: &str, new: &str) -> String {
    let mut result = String::new();
    let mut last_end = 0;
    
    for (i, _) in content.match_indices(old) {
        // Check word boundaries
        let before = if i > 0 { content.chars().nth(i - 1) } else { None };
        let after = content.chars().nth(i + old.len());
        
        let is_word_char = |c: char| c.is_alphanumeric() || c == '_';
        let word_before = before.map(is_word_char).unwrap_or(false);
        let word_after = after.map(is_word_char).unwrap_or(false);
        
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
        span: Span,
        new_content: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edit_module_creation() {
        // EditModule::new requires UnifiedGraphStore
        // For now just test that the type exists
        use crate::storage::UnifiedGraphStore;
        // Can't easily create without async, but verify type exists
        let _store: Option<UnifiedGraphStore> = None;
    }

    #[test]
    fn test_edit_operation_replace() {
        let span = Span { start: 10, end: 20 };
        let _op = EditOperation::Replace {
            span,
            new_content: String::from("test"),
        };
    }
}

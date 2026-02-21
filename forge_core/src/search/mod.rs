//! Search module - Semantic code search via llmgrep
//!
//! This module provides semantic code search by integrating with llmgrep,
//! which queries magellan databases for symbols, references, and calls.

use std::sync::Arc;
use crate::storage::UnifiedGraphStore;
use crate::error::{ForgeError, Result as ForgeResult};
use crate::types::{Symbol, SymbolKind, Language, Location};

/// Search module for semantic code queries.
pub struct SearchModule {
    store: Arc<UnifiedGraphStore>,
}

impl SearchModule {
    /// Create a new SearchModule.
    pub fn new(store: Arc<UnifiedGraphStore>) -> Self {
        Self { store }
    }

    /// Indexes the codebase for search.
    ///
    /// This is a no-op for the current implementation which scans files directly.
    /// In a future version with embedding-based search, this would build the index.
    pub async fn index(&self) -> ForgeResult<()> {
        // Current implementation scans files directly, no indexing needed
        Ok(())
    }

    /// Pattern-based search using regex (async).
    ///
    /// Scans source files for patterns like "fn \w+\(" and returns matching symbols.
    pub async fn pattern_search(&self, pattern: &str) -> ForgeResult<Vec<Symbol>> {
        use regex::Regex;
        
        
        // Compile the regex pattern
        let regex = Regex::new(pattern)
            .map_err(|e| ForgeError::DatabaseError(format!("Invalid regex pattern: {}", e)))?;
        
        let mut results = Vec::new();
        
        // Scan source files recursively
        Self::search_files_recursive(
            &self.store.codebase_path,
            &self.store.codebase_path,
            &regex,
            &mut results,
        ).await?;
        
        Ok(results)
    }
    
    /// Recursively search files for pattern matches
    async fn search_files_recursive(
        root: &std::path::Path,
        dir: &std::path::Path,
        regex: &regex::Regex,
        results: &mut Vec<Symbol>,
    ) -> ForgeResult<()> {
        use tokio::fs;
        
        let mut entries = fs::read_dir(dir).await
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to read dir: {}", e)))?;
        
        while let Some(entry) = entries.next_entry().await
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to read entry: {}", e)))? 
        {
            let path = entry.path();
            if path.is_dir() {
                // Recurse into subdirectories
                Box::pin(Self::search_files_recursive(root, &path, regex, results)).await?;
            } else if path.is_file() && path.extension().map(|e| e == "rs").unwrap_or(false) {
                // Read and search Rust files
                if let Ok(content) = fs::read_to_string(&path).await {
                    for (line_num, line) in content.lines().enumerate() {
                        if regex.is_match(line) {
                            // Extract symbol name from the matched line
                            let symbol_name = extract_symbol_from_line(line);
                            let relative_path = path.strip_prefix(root).unwrap_or(&path);
                            
                            results.push(Symbol {
                                id: crate::types::SymbolId(0),
                                name: symbol_name.clone(),
                                fully_qualified_name: symbol_name,
                                kind: SymbolKind::Function, // Assume function for fn patterns
                                language: Language::Rust,
                                location: Location {
                                    file_path: relative_path.to_path_buf(),
                                    byte_start: 0,
                                    byte_end: line.len() as u32,
                                    line_number: line_num + 1,
                                },
                                parent_id: None,
                                metadata: serde_json::Value::Null,
                            });
                        }
                    }
                }
            }
        }
        
        Ok(())
    }

    /// Pattern-based search (alias for `pattern_search`).
    pub async fn pattern(&self, pattern: &str) -> ForgeResult<Vec<Symbol>> {
        self.pattern_search(pattern).await
    }

    /// Semantic search using natural language (async).
    ///
    /// Note: True semantic search would require embedding generation.
    /// This implementation uses keyword matching on symbol names, with
    /// substring matching for partial word matches.
    pub async fn semantic_search(&self, query: &str) -> ForgeResult<Vec<Symbol>> {
        // Extract keywords from the query
        let keywords: Vec<&str> = query
            .split_whitespace()
            .filter(|w| w.len() >= 3) // Consider words 3+ chars
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
            .filter(|w| !w.is_empty())
            .collect();
        
        if keywords.is_empty() {
            return Ok(Vec::new());
        }
        
        // First try exact pattern search
        let mut all_results = Vec::new();
        for keyword in &keywords {
            let matches = self.pattern_search(keyword).await?;
            all_results.extend(matches);
        }
        
        // Also scan files for keywords that might match as substrings
        // This handles cases like "addition" matching "add"
        self.scan_for_substring_matches(&keywords, &mut all_results).await?;
        
        // Remove duplicates (by name)
        let mut seen = std::collections::HashSet::new();
        all_results.retain(|s| seen.insert(s.name.clone()));
        
        Ok(all_results)
    }
    
    /// Scan files for symbols that contain keyword substrings
    async fn scan_for_substring_matches(
        &self,
        keywords: &[&str],
        results: &mut Vec<Symbol>,
    ) -> ForgeResult<()> {
        use tokio::fs;
        
        let codebase_path = &self.store.codebase_path;
        let mut entries = fs::read_dir(codebase_path).await
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to read dir: {}", e)))?;
        
        while let Some(entry) = entries.next_entry().await
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to read entry: {}", e)))? 
        {
            let path = entry.path();
            if path.is_dir() {
                // Recurse (simplified - in production use walkdir)
                if let Ok(sub_entries) = fs::read_dir(&path).await {
                    let mut sub_entries = sub_entries;
                    while let Ok(Some(sub_entry)) = sub_entries.next_entry().await {
                        let sub_path = sub_entry.path();
                        if sub_path.is_file() && sub_path.extension().map(|e| e == "rs").unwrap_or(false) {
                            Self::check_file_for_submatches(&sub_path, keywords, results, codebase_path).await?;
                        }
                    }
                }
            } else if path.is_file() && path.extension().map(|e| e == "rs").unwrap_or(false) {
                Self::check_file_for_submatches(&path, keywords, results, codebase_path).await?;
            }
        }
        
        Ok(())
    }
    
    async fn check_file_for_submatches(
        path: &std::path::Path,
        keywords: &[&str],
        results: &mut Vec<Symbol>,
        root: &std::path::Path,
    ) -> ForgeResult<()> {
        use tokio::fs;
        
        if let Ok(content) = fs::read_to_string(path).await {
            for (line_num, line) in content.lines().enumerate() {
                // Look for function definitions
                if line.contains("fn ") {
                    let fn_name = extract_symbol_from_line(line);
                    // Check if any keyword is a substring of this function name
                    // or if function name is a substring of any keyword
                    for keyword in keywords {
                        if fn_name.contains(keyword) || keyword.contains(&fn_name) {
                            if !fn_name.is_empty() && fn_name != "fn" {
                                let relative_path = path.strip_prefix(root).unwrap_or(path);
                                results.push(Symbol {
                                    id: crate::types::SymbolId(0),
                                    name: fn_name.clone(),
                                    fully_qualified_name: fn_name,
                                    kind: SymbolKind::Function,
                                    language: Language::Rust,
                                    location: Location {
                                        file_path: relative_path.to_path_buf(),
                                        byte_start: 0,
                                        byte_end: line.len() as u32,
                                        line_number: line_num + 1,
                                    },
                                    parent_id: None,
                                    metadata: serde_json::Value::Null,
                                });
                            }
                            break;
                        }
                    }
                }
                
                // Also look for struct definitions (for "calculator" -> "Calculator")
                if line.contains("struct ") {
                    let struct_name = extract_struct_from_line(line);
                    for keyword in keywords {
                        let keyword_lower = keyword.to_lowercase();
                        let struct_lower = struct_name.to_lowercase();
                        if struct_lower.contains(&keyword_lower) || keyword_lower.contains(&struct_lower) {
                            if !struct_name.is_empty() {
                                let relative_path = path.strip_prefix(root).unwrap_or(path);
                                results.push(Symbol {
                                    id: crate::types::SymbolId(0),
                                    name: struct_name.clone(),
                                    fully_qualified_name: struct_name,
                                    kind: SymbolKind::Struct,
                                    language: Language::Rust,
                                    location: Location {
                                        file_path: relative_path.to_path_buf(),
                                        byte_start: 0,
                                        byte_end: line.len() as u32,
                                        line_number: line_num + 1,
                                    },
                                    parent_id: None,
                                    metadata: serde_json::Value::Null,
                                });
                            }
                            break;
                        }
                    }
                }
            }
        }
        
        Ok(())
    }

    /// Semantic search (alias for `semantic_search`).
    pub async fn semantic(&self, query: &str) -> ForgeResult<Vec<Symbol>> {
        self.semantic_search(query).await
    }

    /// Find a specific symbol by name (async).
    pub async fn symbol_by_name(&self, name: &str) -> ForgeResult<Option<Symbol>> {
        let symbols = self.pattern_search(name).await?;
        // Return first exact match or None
        Ok(symbols.into_iter().find(|s| s.name == name))
    }

    /// Find all symbols of a specific kind (async).
    pub async fn symbols_by_kind(&self, kind: SymbolKind) -> ForgeResult<Vec<Symbol>> {
        // Query all symbols and filter by kind
        let all_symbols = self.store.get_all_symbols().await
            .map_err(|e| ForgeError::DatabaseError(format!("Kind search failed: {}", e)))?;

        let filtered: Vec<Symbol> = all_symbols
            .into_iter()
            .filter(|s| s.kind == kind)
            .collect();

        Ok(filtered)
    }
}

/// Map magellan SymbolKind to forge SymbolKind
#[cfg(feature = "magellan")]
#[expect(dead_code)] // Helper for magellan integration
fn map_magellan_kind(kind: &magellan::SymbolKind) -> SymbolKind {
    use magellan::SymbolKind as MagellanKind;
    
    match kind {
        MagellanKind::Function => SymbolKind::Function,
        MagellanKind::Method => SymbolKind::Method,
        MagellanKind::Class => SymbolKind::Struct,
        MagellanKind::Interface => SymbolKind::Trait,
        MagellanKind::Enum => SymbolKind::Enum,
        MagellanKind::Module => SymbolKind::Module,
        MagellanKind::TypeAlias => SymbolKind::TypeAlias,
        MagellanKind::Union => SymbolKind::Enum,
        MagellanKind::Namespace => SymbolKind::Module,
        MagellanKind::Unknown => SymbolKind::Function,
    }
}

/// Extract function name from a source line
/// e.g., "pub fn add(a: i32) -> i32 {" -> "add"
fn extract_symbol_from_line(line: &str) -> String {
    let line = line.trim();
    
    // Try to extract function name
    if let Some(fn_pos) = line.find("fn ") {
        let after_fn = &line[fn_pos + 3..];
        // Find the end of the identifier (whitespace or ()
        if let Some(end_pos) = after_fn.find(|c: char| c.is_whitespace() || c == '(') {
            return after_fn[..end_pos].trim().to_string();
        }
    }
    
    // Default: return first word
    line.split_whitespace().next().unwrap_or("").to_string()
}

/// Extract struct name from a source line
/// e.g., "pub struct Calculator {" -> "Calculator"
fn extract_struct_from_line(line: &str) -> String {
    let line = line.trim();
    
    if let Some(struct_pos) = line.find("struct ") {
        let after_struct = &line[struct_pos + 7..];
        if let Some(end_pos) = after_struct.find(|c: char| c.is_whitespace() || c == '{' || c == ';' || c == '(') {
            return after_struct[..end_pos].trim().to_string();
        }
    }
    
    // Default: return first word
    line.split_whitespace().next().unwrap_or("").to_string()
}

/// Simple glob pattern matching (supports * wildcard)
#[expect(dead_code)] // Helper for pattern matching
fn glob_match(pattern: &str, text: &str) -> bool {
    if !pattern.contains('*') {
        return pattern == text;
    }
    
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.is_empty() {
        return true;
    }
    
    let mut text_remaining = text;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        
        if i == 0 && !pattern.starts_with('*') {
            // First part must match at start
            if !text_remaining.starts_with(part) {
                return false;
            }
            text_remaining = &text_remaining[part.len()..];
        } else if i == parts.len() - 1 && !pattern.ends_with('*') {
            // Last part must match at end
            if !text_remaining.ends_with(part) {
                return false;
            }
        } else {
            // Middle part can match anywhere
            if let Some(pos) = text_remaining.find(part) {
                text_remaining = &text_remaining[pos + part.len()..];
            } else {
                return false;
            }
        }
    }
    
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::BackendKind;

    #[tokio::test]
    async fn test_search_module_creation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Arc::new(UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite).await.unwrap());
        let _search = SearchModule::new(store.clone());
    }

    #[tokio::test]
    async fn test_pattern_search_empty() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Arc::new(UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite).await.unwrap());
        let search = SearchModule::new(store);

        let results = search.pattern_search("nonexistent").await.unwrap();
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_symbol_by_name_not_found() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Arc::new(UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite).await.unwrap());
        let search = SearchModule::new(store);

        let result = search.symbol_by_name("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_symbols_by_kind() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Arc::new(UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite).await.unwrap());
        let search = SearchModule::new(store);

        let functions = search.symbols_by_kind(SymbolKind::Function).await.unwrap();
        // Empty since no symbols inserted yet
        assert!(functions.is_empty());
    }
}

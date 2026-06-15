//! Search module - Semantic code search via llmgrep
//!
//! This module provides semantic code search by delegating to `llmgrep::forge`
//! convenience functions. When llmgrep is disabled, falls back to regex-based
//! file scanning.

use crate::error::{ForgeError, Result as ForgeResult};
use crate::storage::UnifiedGraphStore;
use crate::types::{Language, Location, Symbol, SymbolId, SymbolKind};
use std::path::PathBuf;
use std::sync::Arc;

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
    /// llmgrep reads magellan's DB directly, so this is a no-op.
    /// The graph module's `index()` populates the shared DB.
    pub async fn index(&self) -> ForgeResult<()> {
        Ok(())
    }

    /// Pattern-based search using regex.
    ///
    /// With llmgrep: delegates to `llmgrep::forge::search_symbols_regex`.
    /// Without: scans source files recursively with regex.
    pub async fn pattern_search(&self, pattern: &str) -> ForgeResult<Vec<Symbol>> {
        let db_path = self.store.db_path.clone();
        if db_path.exists() {
            if let Ok(results) = self.search_via_llmgrep(pattern, true).await {
                return Ok(results);
            }
        }

        self.pattern_search_via_files(pattern).await
    }

    /// Pattern-based search (alias for `pattern_search`).
    pub async fn pattern(&self, pattern: &str) -> ForgeResult<Vec<Symbol>> {
        self.pattern_search(pattern).await
    }

    /// Semantic search using natural language.
    ///
    /// With llmgrep: delegates to `llmgrep::forge::search_symbols`.
    /// Without: splits query into keywords and scans files.
    pub async fn semantic_search(&self, query: &str) -> ForgeResult<Vec<Symbol>> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        let db_path = self.store.db_path.clone();
        if db_path.exists() {
            if let Ok(results) = self.search_via_llmgrep(query, false).await {
                return Ok(results);
            }
        }

        self.semantic_search_via_files(query).await
    }

    /// Semantic search (alias for `semantic_search`).
    pub async fn semantic(&self, query: &str) -> ForgeResult<Vec<Symbol>> {
        self.semantic_search(query).await
    }

    /// Find a specific symbol by name.
    pub async fn symbol_by_name(&self, name: &str) -> ForgeResult<Option<Symbol>> {
        let symbols = self.pattern_search(name).await?;
        Ok(symbols.into_iter().find(|s| s.name == Arc::from(name)))
    }

    /// Find all symbols of a specific kind.
    pub async fn symbols_by_kind(&self, kind: SymbolKind) -> ForgeResult<Vec<Symbol>> {
        let all_symbols = self
            .store
            .get_all_symbols()
            .await
            .map_err(|e| ForgeError::DatabaseError(format!("Kind search failed: {}", e)))?;

        Ok(all_symbols.into_iter().filter(|s| s.kind == kind).collect())
    }

    /// Find all references to a symbol.
    pub async fn references(&self, symbol_name: &str, limit: usize) -> ForgeResult<Vec<Symbol>> {
        let db_path = self.store.db_path.clone();
        if !db_path.exists() {
            return Ok(Vec::new());
        }
        llmgrep::forge::search_references(symbol_name, &db_path, limit)
            .map(|refs| {
                refs.into_iter()
                    .map(|r| Symbol {
                        id: SymbolId(0),
                        name: Arc::from(r.referenced_symbol.clone()),
                        fully_qualified_name: Arc::from(r.referenced_symbol),
                        kind: SymbolKind::Function,
                        language: Language::Unknown("unknown".to_string()),
                        location: Location {
                            file_path: PathBuf::from(&r.span.file_path),
                            byte_start: r.span.byte_start as u32,
                            byte_end: r.span.byte_end as u32,
                            line_number: r.span.start_line as usize,
                        },
                        parent_id: None,
                        metadata: serde_json::Value::Null,
                    })
                    .collect()
            })
            .map_err(|e| ForgeError::DatabaseError(format!("Reference search failed: {}", e)))
    }

    /// Find all calls involving a symbol.
    pub async fn calls(&self, symbol_name: &str, limit: usize) -> ForgeResult<Vec<Symbol>> {
        let db_path = self.store.db_path.clone();
        if !db_path.exists() {
            return Ok(Vec::new());
        }
        llmgrep::forge::search_calls(symbol_name, &db_path, limit)
            .map(|calls| {
                calls
                    .into_iter()
                    .map(|c| Symbol {
                        id: SymbolId(0),
                        name: Arc::from(c.caller.clone()),
                        fully_qualified_name: Arc::from(c.caller.clone()),
                        kind: SymbolKind::Function,
                        language: Language::Unknown("unknown".to_string()),
                        location: Location {
                            file_path: PathBuf::from(&c.span.file_path),
                            byte_start: c.span.byte_start as u32,
                            byte_end: c.span.byte_end as u32,
                            line_number: c.span.start_line as usize,
                        },
                        parent_id: None,
                        metadata: serde_json::Value::Null,
                    })
                    .collect()
            })
            .map_err(|e| ForgeError::DatabaseError(format!("Call search failed: {}", e)))
    }

    /// Lookup a symbol by fully-qualified name.
    pub async fn lookup(&self, fqn: &str) -> ForgeResult<Option<Symbol>> {
        let db_path = self.store.db_path.clone();
        if !db_path.exists() {
            return Ok(None);
        }
        llmgrep::forge::lookup_symbol(fqn, &db_path)
            .map(|m| Some(llmgrep_match_to_symbol(m)))
            .map_err(|e| ForgeError::DatabaseError(format!("Lookup failed: {}", e)))
    }

    // -- llmgrep-backed search --

    async fn search_via_llmgrep(&self, query: &str, use_regex: bool) -> ForgeResult<Vec<Symbol>> {
        let db_path = self.store.db_path.clone();

        let result = if use_regex {
            llmgrep::forge::search_symbols_regex(query, &db_path, 50)
        } else {
            llmgrep::forge::search_symbols(query, &db_path, 50)
        };

        result
            .map(|matches| matches.into_iter().map(llmgrep_match_to_symbol).collect())
            .map_err(|e| ForgeError::DatabaseError(format!("llmgrep search failed: {}", e)))
    }

    // -- File-based fallback search --

    async fn pattern_search_via_files(&self, pattern: &str) -> ForgeResult<Vec<Symbol>> {
        use regex::Regex;

        let regex = Regex::new(pattern)
            .map_err(|e| ForgeError::DatabaseError(format!("Invalid regex pattern: {}", e)))?;

        let mut results = Vec::new();
        let mut files = Vec::new();
        collect_source_files(&self.store.codebase_path, &mut files).await;

        for path in files {
            if let Ok(content) = tokio::fs::read_to_string(&path).await {
                for (line_num, line) in content.lines().enumerate() {
                    if regex.is_match(line) {
                        let symbol_name = extract_symbol_from_line(line);
                        let relative_path = path
                            .strip_prefix(&self.store.codebase_path)
                            .unwrap_or(&path);
                        results.push(Symbol {
                            id: SymbolId(0),
                            name: Arc::from(symbol_name.clone()),
                            fully_qualified_name: Arc::from(symbol_name),
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
                }
            }
        }

        Ok(results)
    }

    async fn semantic_search_via_files(&self, query: &str) -> ForgeResult<Vec<Symbol>> {
        let keywords: Vec<&str> = query
            .split_whitespace()
            .filter(|w| w.len() >= 3)
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
            .filter(|w| !w.is_empty())
            .collect();

        if keywords.is_empty() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();
        let mut files = Vec::new();
        collect_source_files(&self.store.codebase_path, &mut files).await;

        for path in files {
            let Ok(content) = tokio::fs::read_to_string(&path).await else {
                continue;
            };
            for (line_num, line) in content.lines().enumerate() {
                let name = extract_symbol_from_line(line);
                if name.is_empty() || name == "fn" {
                    continue;
                }
                let name_lower = name.to_lowercase();
                let matches_keyword = keywords.iter().any(|kw| {
                    let kw_lower = kw.to_lowercase();
                    name_lower.contains(&kw_lower) || kw_lower.contains(&name_lower)
                });
                if matches_keyword {
                    let relative_path = path
                        .strip_prefix(&self.store.codebase_path)
                        .unwrap_or(&path);
                    results.push(Symbol {
                        id: SymbolId(0),
                        name: Arc::from(name.clone()),
                        fully_qualified_name: Arc::from(name.clone()),
                        kind: if line.contains("struct ") {
                            SymbolKind::Struct
                        } else {
                            SymbolKind::Function
                        },
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

        let mut seen = std::collections::HashSet::new();
        results.retain(|s| seen.insert(s.name.clone()));

        Ok(results)
    }
}

async fn collect_source_files(dir: &std::path::Path, files: &mut Vec<PathBuf>) {
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
            Box::pin(collect_source_files(&path, files)).await;
        } else if path.is_file()
            && path
                .extension()
                .map(|e| {
                    matches!(
                        e.to_str(),
                        Some("rs" | "py" | "ts" | "js" | "go" | "java" | "c" | "cpp")
                    )
                })
                .unwrap_or(false)
        {
            files.push(path);
        }
    }
}

fn llmgrep_match_to_symbol(m: llmgrep::output::SymbolMatch) -> Symbol {
    let kind = map_llmgrep_kind(&m.kind);
    let language = m
        .language
        .as_deref()
        .map(map_llmgrep_language)
        .unwrap_or(Language::Unknown("unknown".to_string()));
    let fqn: Arc<str> = Arc::from(m.fqn.clone().unwrap_or_else(|| m.name.clone()));

    Symbol {
        id: SymbolId(0),
        name: Arc::from(m.name),
        fully_qualified_name: fqn,
        kind,
        language,
        location: Location {
            file_path: PathBuf::from(&m.span.file_path),
            byte_start: m.span.byte_start as u32,
            byte_end: m.span.byte_end as u32,
            line_number: m.span.start_line as usize,
        },
        parent_id: None,
        metadata: serde_json::Value::Null,
    }
}

fn map_llmgrep_kind(kind: &str) -> SymbolKind {
    match kind {
        "function_item" | "function" => SymbolKind::Function,
        "method_item" | "method" | "impl_item" => SymbolKind::Method,
        "struct_item" | "struct" | "class" => SymbolKind::Struct,
        "trait_item" | "trait" | "interface" => SymbolKind::Trait,
        "enum_item" | "enum" => SymbolKind::Enum,
        "mod_item" | "module" | "namespace" => SymbolKind::Module,
        "type_item" | "type_alias" => SymbolKind::TypeAlias,
        "const_item" | "constant" => SymbolKind::Constant,
        "field" | "property" => SymbolKind::Field,
        _ => SymbolKind::Function,
    }
}

fn map_llmgrep_language(lang: &str) -> Language {
    match lang {
        "rust" => Language::Rust,
        "python" => Language::Python,
        "c" => Language::C,
        "cpp" | "c++" => Language::Cpp,
        "java" => Language::Java,
        "javascript" | "js" => Language::JavaScript,
        "typescript" | "ts" => Language::TypeScript,
        "go" => Language::Go,
        _ => Language::Unknown(lang.to_string()),
    }
}

fn extract_symbol_from_line(line: &str) -> String {
    let line = line.trim();

    if let Some(fn_pos) = line.find("fn ") {
        let after_fn = &line[fn_pos + 3..];
        if let Some(end_pos) = after_fn.find(|c: char| c.is_whitespace() || c == '(') {
            return after_fn[..end_pos].trim().to_string();
        }
    }

    line.split_whitespace().next().unwrap_or("").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::BackendKind;

    #[tokio::test]
    async fn test_search_module_creation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await
                .unwrap(),
        );
        let _search = SearchModule::new(Arc::clone(&store));
    }

    #[tokio::test]
    async fn test_pattern_search_empty() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await
                .unwrap(),
        );
        let search = SearchModule::new(store);

        let results = search.pattern_search("nonexistent").await.unwrap();
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_symbol_by_name_not_found() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await
                .unwrap(),
        );
        let search = SearchModule::new(store);

        let result = search.symbol_by_name("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_symbols_by_kind() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
                .await
                .unwrap(),
        );
        let search = SearchModule::new(store);

        let functions = search.symbols_by_kind(SymbolKind::Function).await.unwrap();
        assert!(functions.is_empty());
    }

    #[test]
    fn test_extract_symbol_from_line() {
        assert_eq!(
            extract_symbol_from_line("pub fn add(a: i32) -> i32 {"),
            "add"
        );
        assert_eq!(extract_symbol_from_line("fn hello() {"), "hello");
    }
}

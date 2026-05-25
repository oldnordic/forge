//! Search module - Semantic code search via llmgrep
//!
//! This module provides semantic code search by integrating with llmgrep,
//! which queries magellan databases for symbols, references, and calls.
//! When the llmgrep feature is disabled, falls back to regex-based file scanning.

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
    /// With llmgrep: queries the magellan DB with regex matching.
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
    /// With llmgrep: queries the magellan DB with fuzzy/substring matching.
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

    // -- llmgrep-backed search --

    async fn search_via_llmgrep(&self, query: &str, use_regex: bool) -> ForgeResult<Vec<Symbol>> {
        use llmgrep::query::SearchOptions;

        let db_path = self.store.db_path.clone();
        let backend = llmgrep::Backend::detect_and_open(&db_path).map_err(|e| {
            ForgeError::DatabaseError(format!("Failed to open llmgrep backend: {}", e))
        })?;

        let options = SearchOptions {
            db_path: &db_path,
            query,
            path_filter: None,
            kind_filter: None,
            language_filter: None,
            limit: 50,
            use_regex,
            candidates: 100,
            context: llmgrep::query::ContextOptions::default(),
            snippet: llmgrep::query::SnippetOptions::default(),
            fqn: llmgrep::query::FqnOptions {
                fqn: true,
                ..Default::default()
            },
            include_score: false,
            sort_by: llmgrep::SortMode::default(),
            metrics: llmgrep::query::MetricsOptions::default(),
            ast: llmgrep::query::AstOptions::new(),
            depth: llmgrep::query::DepthOptions::default(),
            algorithm: llmgrep::AlgorithmOptions::default(),
            symbol_id: None,
            fqn_pattern: None,
            exact_fqn: None,
            coverage_filter: None,
        };

        let (response, _truncated, _fts_used) = backend
            .search_symbols(options)
            .map_err(|e| ForgeError::DatabaseError(format!("llmgrep search failed: {}", e)))?;

        Ok(response
            .results
            .into_iter()
            .map(llmgrep_match_to_symbol)
            .collect())
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

/// Recursively collect source files, skipping build artifacts.
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

/// Extract function name from a source line
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

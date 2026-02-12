//! Storage abstraction layer.
//!
//! This module provides a unified interface to the SQLiteGraph backend.

use std::path::Path;
use std::sync::Arc;
use crate::error::{ForgeError, Result};
use crate::types::{Symbol, SymbolId, Reference, SymbolKind, ReferenceKind, Language};

/// Unified graph store for all ForgeKit operations.
///
/// This wraps the SQLiteGraph backend and provides a high-level API
/// for querying symbols, references, and CFG data.
#[derive(Clone)]
pub struct UnifiedGraphStore {
    /// Path to the codebase
    pub codebase_path: std::path::PathBuf,
    /// Path to the database file
    pub db_path: std::path::PathBuf,
    /// Internal graph backend
    #[cfg(feature = "sqlite")]
    graph: Option<Arc<sqlitegraph::SqliteGraph>>,
}

impl std::fmt::Debug for UnifiedGraphStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnifiedGraphStore")
            .field("codebase_path", &self.codebase_path)
            .field("db_path", &self.db_path)
            .field("graph", &self.graph.as_ref().map(|_| "<SqliteGraph>"))
            .finish()
    }
}

impl UnifiedGraphStore {
    /// Opens a graph store at the given path.
    ///
    /// # Arguments
    ///
    /// * `codebase_path` - Path to the codebase directory
    ///
    /// # Returns
    ///
    /// A `UnifiedGraphStore` instance or an error if initialization fails
    pub async fn open(codebase_path: impl AsRef<Path>) -> Result<Self> {
        let codebase = codebase_path.as_ref();
        let db_path = codebase.join(".forge").join("graph.db");

        // Create parent directory if it doesn't exist
        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent).await
                .map_err(|e| ForgeError::DatabaseError(
                    format!("Failed to create database directory: {}", e)
                ))?;
        }

        #[cfg(feature = "sqlite")]
        let graph = {
            // Try to open existing database, or create new one
            match sqlitegraph::SqliteGraph::open(&db_path) {
                Ok(g) => Some(Arc::new(g)),
                Err(e) => {
                    // If database doesn't exist yet, that's okay
                    // We'll create it when indexing happens
                    eprintln!("Warning: Could not open database: {}", e);
                    None
                }
            }
        };

        #[cfg(not(feature = "sqlite"))]
        let graph = None;

        Ok(UnifiedGraphStore {
            codebase_path: codebase.to_path_buf(),
            db_path,
            graph,
        })
    }

    /// Opens a graph store with a custom database path.
    ///
    /// # Arguments
    ///
    /// * `codebase_path` - Path to the codebase directory
    /// * `db_path` - Custom path for the database file
    pub async fn open_with_path(codebase_path: impl AsRef<Path>, db_path: impl AsRef<Path>) -> Result<Self> {
        let codebase = codebase_path.as_ref();
        let db = db_path.as_ref();

        // Create parent directory if it doesn't exist
        if let Some(parent) = db.parent() {
            tokio::fs::create_dir_all(parent).await
                .map_err(|e| ForgeError::DatabaseError(
                    format!("Failed to create database directory: {}", e)
                ))?;
        }

        #[cfg(feature = "sqlite")]
        let graph = {
            match sqlitegraph::SqliteGraph::open(db) {
                Ok(g) => Some(Arc::new(g)),
                Err(e) => {
                    eprintln!("Warning: Could not open database: {}", e);
                    None
                }
            }
        };

        #[cfg(not(feature = "sqlite"))]
        let graph = None;

        Ok(UnifiedGraphStore {
            codebase_path: codebase.to_path_buf(),
            db_path: db.to_path_buf(),
            graph,
        })
    }

    /// Returns the path to the database file.
    #[inline]
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Returns true if the database is connected.
    pub fn is_connected(&self) -> bool {
        self.graph.is_some()
    }

    /// Creates an in-memory graph store for testing.
    ///
    /// # Returns
    ///
    /// A `UnifiedGraphStore` instance backed by an in-memory database.
    pub async fn memory() -> Result<Self> {
        #[cfg(feature = "sqlite")]
        let graph = Some(Arc::new(sqlitegraph::SqliteGraph::open_in_memory_with_config(
            &sqlitegraph::SqliteConfig::default(),
        ).map_err(|e| ForgeError::DatabaseError(e.to_string()))?));

        #[cfg(not(feature = "sqlite"))]
        let graph = None;

        Ok(UnifiedGraphStore {
            codebase_path: std::path::PathBuf::from("/memory"),
            db_path: std::path::PathBuf::from(":memory:"),
            graph,
        })
    }

    /// Query symbols by name from the database.
    ///
    /// # Arguments
    ///
    /// * `name` - The symbol name to search for
    ///
    /// # Returns
    ///
    /// A vector of matching symbols
    pub async fn query_symbols(&self, name: &str) -> Result<Vec<Symbol>> {
        let graph = self.graph.as_ref()
            .ok_or_else(|| ForgeError::BackendNotAvailable(
                "Database not connected".to_string()
            ))?;

        // Query symbols table using raw SQL through the introspection API
        self.query_symbols_impl(graph, name).await
    }

    #[cfg(feature = "sqlite")]
    async fn query_symbols_impl(&self, graph: &sqlitegraph::SqliteGraph, name: &str) -> Result<Vec<Symbol>> {
        // Use the introspection API to get underlying connection
        let introspection = graph.introspect()
            .map_err(|e| ForgeError::DatabaseError(format!("Introspection failed: {}", e)))?;

        // For now, return empty results if we can't query
        // Full implementation will use the proper SQLiteGraph API
        let _ = (introspection, name);
        Ok(Vec::new())
    }

    /// Query references for a specific symbol.
    ///
    /// # Arguments
    ///
    /// * `symbol_id` - The symbol ID to query references for
    ///
    /// # Returns
    ///
    /// A vector of references to/from the symbol
    pub async fn query_references(&self, _symbol_id: SymbolId) -> Result<Vec<Reference>> {
        // Placeholder - will be implemented with proper SQLiteGraph API
        Ok(Vec::new())
    }

    /// Checks if a symbol exists in the graph.
    pub async fn symbol_exists(&self, id: SymbolId) -> Result<bool> {
        let graph = self.graph.as_ref()
            .ok_or_else(|| ForgeError::BackendNotAvailable(
                "Database not connected".to_string()
            ))?;

        #[cfg(feature = "sqlite")]
        {
            // Try to get introspection to check if database is valid
            let _introspection = graph.introspect()
                .map_err(|e| ForgeError::DatabaseError(format!("Introspection failed: {}", e)))?;
            let _ = id;
            // For now, return false since we haven't implemented symbol lookup
            Ok(false)
        }

        #[cfg(not(feature = "sqlite"))]
        {
            let _ = (graph, id);
            Ok(false)
        }
    }

    /// Gets a symbol by ID.
    pub async fn get_symbol(&self, id: SymbolId) -> Result<Symbol> {
        let graph = self.graph.as_ref()
            .ok_or_else(|| ForgeError::BackendNotAvailable(
                "Database not connected".to_string()
            ))?;

        #[cfg(feature = "sqlite")]
        {
            let _introspection = graph.introspect()
                .map_err(|e| ForgeError::DatabaseError(format!("Introspection failed: {}", e)))?;
            let _ = id;
            // For now, return not found since we haven't implemented symbol lookup
            Err(ForgeError::SymbolNotFound(format!("{}", id)))
        }

        #[cfg(not(feature = "sqlite"))]
        {
            let _ = (graph, id);
            Err(ForgeError::SymbolNotFound(format!("{}", id)))
        }
    }
}

/// Parse a symbol kind from string.
fn parse_symbol_kind(s: &str) -> SymbolKind {
    match s {
        "Function" => SymbolKind::Function,
        "Method" => SymbolKind::Method,
        "Struct" => SymbolKind::Struct,
        "Enum" => SymbolKind::Enum,
        "Trait" => SymbolKind::Trait,
        "Impl" => SymbolKind::Impl,
        "Module" => SymbolKind::Module,
        "TypeAlias" => SymbolKind::TypeAlias,
        "Constant" => SymbolKind::Constant,
        "Static" => SymbolKind::Static,
        "Parameter" => SymbolKind::Parameter,
        "LocalVariable" => SymbolKind::LocalVariable,
        "Field" => SymbolKind::Field,
        "Macro" => SymbolKind::Macro,
        "Use" => SymbolKind::Use,
        _ => SymbolKind::Function, // Default fallback
    }
}

/// Parse a language from string.
fn parse_language(s: &str) -> Language {
    match s {
        "Rust" => Language::Rust,
        "Python" => Language::Python,
        "C" => Language::C,
        "Cpp" => Language::Cpp,
        "Java" => Language::Java,
        "JavaScript" => Language::JavaScript,
        "TypeScript" => Language::TypeScript,
        "Go" => Language::Go,
        other => Language::Unknown(other.to_string()),
    }
}

/// Parse a reference kind from string.
fn parse_reference_kind(s: &str) -> ReferenceKind {
    match s {
        "Call" => ReferenceKind::Call,
        "Use" => ReferenceKind::Use,
        "TypeReference" => ReferenceKind::TypeReference,
        "Inherit" => ReferenceKind::Inherit,
        "Implementation" => ReferenceKind::Implementation,
        "Override" => ReferenceKind::Override,
        _ => ReferenceKind::Use, // Default fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_unified_graph_store() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = UnifiedGraphStore::open(temp_dir.path()).await.unwrap();

        assert!(store.db_path().starts_with(temp_dir.path()));
        // Graph may or may not be connected depending on if database exists
    }

    #[tokio::test]
    async fn test_open_with_custom_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let custom_db = temp_dir.path().join("custom").join("db.sqlite");

        let store = UnifiedGraphStore::open_with_path(temp_dir.path(), &custom_db).await.unwrap();

        assert_eq!(store.db_path(), custom_db);
    }

    #[tokio::test]
    async fn test_query_symbols_empty() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = UnifiedGraphStore::open(temp_dir.path()).await.unwrap();

        let symbols = store.query_symbols("main").await.unwrap();
        assert_eq!(symbols.len(), 0);
    }

    #[test]
    fn test_parse_symbol_kind() {
        assert!(matches!(parse_symbol_kind("Function"), SymbolKind::Function));
        assert!(matches!(parse_symbol_kind("Struct"), SymbolKind::Struct));
        assert!(matches!(parse_symbol_kind("Unknown"), SymbolKind::Function)); // fallback
    }

    #[test]
    fn test_parse_language() {
        assert!(matches!(parse_language("Rust"), Language::Rust));
        assert!(matches!(parse_language("Python"), Language::Python));
        assert!(matches!(parse_language("UnknownLang"), Language::Unknown(_)));
    }

    #[test]
    fn test_parse_reference_kind() {
        assert!(matches!(parse_reference_kind("Call"), ReferenceKind::Call));
        assert!(matches!(parse_reference_kind("Use"), ReferenceKind::Use));
        assert!(matches!(parse_reference_kind("Unknown"), ReferenceKind::Use)); // fallback
    }
}

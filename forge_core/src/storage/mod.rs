//! Storage abstraction layer using sqlitegraph V3 backend.
//!
//! This module provides graph-based storage for ForgeKit using sqlitegraph's
//! native V3 backend for high performance and scalability.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use parking_lot::RwLock;

use crate::error::{ForgeError, Result};
use crate::types::{Symbol, SymbolId, Reference, SymbolKind, ReferenceKind, Language, Location};

/// Re-export sqlitegraph types needed for advanced usage
pub use sqlitegraph::backend::{GraphBackend, NodeSpec, EdgeSpec};
pub use sqlitegraph::graph::GraphEntity;

/// Unified graph store using sqlitegraph V3 backend.
///
/// This provides high-performance graph storage for symbols and references
/// with full ACID transactions and incremental indexing support.
pub struct UnifiedGraphStore {
    /// Path to codebase
    pub codebase_path: PathBuf,
    /// Path to database file
    pub db_path: PathBuf,
    /// Internal sqlitegraph backend (V3)
    backend: Arc<RwLock<Option<sqlitegraph::backend::native::v3::V3Backend>>>,
    /// Temp directory holder for in-memory databases (keeps the dir alive)
    _temp_dir: Option<Arc<tempfile::TempDir>>,
}

impl Clone for UnifiedGraphStore {
    fn clone(&self) -> Self {
        Self {
            codebase_path: self.codebase_path.clone(),
            db_path: self.db_path.clone(),
            backend: Arc::clone(&self.backend),
            _temp_dir: self._temp_dir.as_ref().map(Arc::clone),
        }
    }
}

impl std::fmt::Debug for UnifiedGraphStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnifiedGraphStore")
            .field("codebase_path", &self.codebase_path)
            .field("db_path", &self.db_path)
            .field("connected", &self.is_connected())
            .finish()
    }
}

impl UnifiedGraphStore {
    /// Opens a graph store at given path.
    ///
    /// # Arguments
    ///
    /// * `codebase_path` - Path to codebase directory
    ///
    /// # Returns
    ///
    /// A `UnifiedGraphStore` instance or an error if initialization fails
    pub async fn open(codebase_path: impl AsRef<Path>) -> Result<Self> {
        let codebase = codebase_path.as_ref();
        let db_path = codebase.join(".forge").join("graph.v3");

        // Create parent directory if it doesn't exist
        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent).await
                .map_err(|e| ForgeError::DatabaseError(
                    format!("Failed to create database directory: {}", e)
                ))?;
        }

        // Initialize V3 backend if database doesn't exist
        let backend = if !db_path.exists() {
            let backend = sqlitegraph::backend::native::v3::V3Backend::create(&db_path)
                .map_err(|e| ForgeError::DatabaseError(
                    format!("Failed to create V3 database: {}", e)
                ))?;
            Some(backend)
        } else {
            let backend = sqlitegraph::backend::native::v3::V3Backend::open(&db_path)
                .map_err(|e| ForgeError::DatabaseError(
                    format!("Failed to open V3 database: {}", e)
                ))?;
            Some(backend)
        };

        Ok(UnifiedGraphStore {
            codebase_path: codebase.to_path_buf(),
            db_path,
            backend: Arc::new(RwLock::new(backend)),
            _temp_dir: None,
        })
    }

    /// Opens a graph store with a custom database path.
    ///
    /// # Arguments
    ///
    /// * `codebase_path` - Path to codebase directory
    /// * `db_path` - Custom path for database file
    pub async fn open_with_path(
        codebase_path: impl AsRef<Path>, 
        db_path: impl AsRef<Path>
    ) -> Result<Self> {
        let codebase = codebase_path.as_ref();
        let db = db_path.as_ref();

        // Create parent directory if it doesn't exist
        if let Some(parent) = db.parent() {
            tokio::fs::create_dir_all(parent).await
                .map_err(|e| ForgeError::DatabaseError(
                    format!("Failed to create database directory: {}", e)
                ))?;
        }

        // Initialize V3 backend
        let backend = if !db.exists() {
            let backend = sqlitegraph::backend::native::v3::V3Backend::create(db)
                .map_err(|e| ForgeError::DatabaseError(
                    format!("Failed to create V3 database: {}", e)
                ))?;
            Some(backend)
        } else {
            let backend = sqlitegraph::backend::native::v3::V3Backend::open(db)
                .map_err(|e| ForgeError::DatabaseError(
                    format!("Failed to open V3 database: {}", e)
                ))?;
            Some(backend)
        };

        Ok(UnifiedGraphStore {
            codebase_path: codebase.to_path_buf(),
            db_path: db.to_path_buf(),
            backend: Arc::new(RwLock::new(backend)),
            _temp_dir: None,
        })
    }

    /// Creates an in-memory graph store for testing.
    ///
    /// # Returns
    ///
    /// A `UnifiedGraphStore` instance backed by an in-memory database.
    pub async fn memory() -> Result<Self> {
        let temp_dir = tempfile::tempdir()
            .map_err(|e| ForgeError::DatabaseError(
                format!("Failed to create temp directory: {}", e)
            ))?;
        
        let db_path = temp_dir.path().join("memory.v3");
        
        let backend = sqlitegraph::backend::native::v3::V3Backend::create(&db_path)
            .map_err(|e| ForgeError::DatabaseError(
                format!("Failed to create V3 database: {}", e)
            ))?;

        Ok(UnifiedGraphStore {
            codebase_path: temp_dir.path().to_path_buf(),
            db_path,
            backend: Arc::new(RwLock::new(Some(backend))),
            _temp_dir: Some(Arc::new(temp_dir)),
        })
    }

    /// Returns path to database file.
    #[inline]
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Returns true if database file exists.
    pub fn is_connected(&self) -> bool {
        self.db_path.exists()
    }

    /// Get a reference to the backend (if initialized).
    fn with_backend<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&sqlitegraph::backend::native::v3::V3Backend) -> Result<T>,
    {
        let backend_guard = self.backend.read();
        match backend_guard.as_ref() {
            Some(backend) => f(backend),
            None => Err(ForgeError::DatabaseError(
                "Database not initialized".to_string()
            )),
        }
    }

    /// Insert a symbol into the graph.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The symbol to insert
    ///
    /// # Returns
    ///
    /// The assigned symbol ID
    pub async fn insert_symbol(&self, symbol: &Symbol) -> Result<SymbolId> {
        self.with_backend(|backend| {
            let node = NodeSpec {
                kind: format!("{:?}", symbol.kind),
                name: symbol.name.clone(),
                file_path: Some(symbol.location.file_path.to_string_lossy().to_string()),
                data: serde_json::json!({
                    "fully_qualified_name": symbol.fully_qualified_name,
                    "language": format!("{:?}", symbol.language),
                    "byte_start": symbol.location.byte_start,
                    "byte_end": symbol.location.byte_end,
                    "line_number": symbol.location.line_number,
                    "parent_id": symbol.parent_id.map(|id| id.0),
                    "metadata": symbol.metadata,
                }),
            };

            let id = backend.insert_node(node)
                .map_err(|e| ForgeError::DatabaseError(
                    format!("Failed to insert symbol: {}", e)
                ))?;

            Ok(SymbolId(id))
        })
    }

    /// Insert a reference between symbols.
    ///
    /// # Arguments
    ///
    /// * `reference` - The reference to insert
    pub async fn insert_reference(&self, reference: &Reference) -> Result<()> {
        self.with_backend(|backend| {
            let edge = EdgeSpec {
                from: reference.from.0,
                to: reference.to.0,
                edge_type: format!("{:?}", reference.kind),
                data: serde_json::json!({
                    "file_path": reference.location.file_path.to_string_lossy().to_string(),
                    "byte_start": reference.location.byte_start,
                    "byte_end": reference.location.byte_end,
                    "line_number": reference.location.line_number,
                }),
            };

            backend.insert_edge(edge)
                .map_err(|e| ForgeError::DatabaseError(
                    format!("Failed to insert reference: {}", e)
                ))?;

            Ok(())
        })
    }

    /// Query symbols by name pattern.
    ///
    /// # Arguments
    ///
    /// * `name` - Name pattern to search for
    ///
    /// # Returns
    ///
    /// List of matching symbols
    pub async fn query_symbols(&self, name: &str) -> Result<Vec<Symbol>> {
        self.with_backend(|backend| {
            let snapshot = sqlitegraph::SnapshotId::current();
            let ids = backend.entity_ids()
                .map_err(|e| ForgeError::DatabaseError(
                    format!("Failed to get entity IDs: {}", e)
                ))?;

            let mut symbols = Vec::new();
            for id in ids {
                if let Ok(entity) = backend.get_node(snapshot, id) {
                    if entity.name.contains(name) {
                        if let Some(symbol) = Self::entity_to_symbol(&entity) {
                            symbols.push(symbol);
                        }
                    }
                }
            }

            Ok(symbols)
        })
    }

    /// Get a symbol by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The symbol ID
    ///
    /// # Returns
    ///
    /// The symbol or an error if not found
    pub async fn get_symbol(&self, id: SymbolId) -> Result<Symbol> {
        self.with_backend(|backend| {
            let snapshot = sqlitegraph::SnapshotId::current();
            let entity = backend.get_node(snapshot, id.0)
                .map_err(|e| ForgeError::SymbolNotFound(
                    format!("Symbol {}: {}", id.0, e)
                ))?;

            Self::entity_to_symbol(&entity)
                .ok_or_else(|| ForgeError::SymbolNotFound(
                    format!("Invalid symbol data for ID {}", id.0)
                ))
        })
    }

    /// Check if a symbol exists.
    ///
    /// # Arguments
    ///
    /// * `id` - The symbol ID to check
    pub async fn symbol_exists(&self, id: SymbolId) -> Result<bool> {
        self.with_backend(|backend| {
            let snapshot = sqlitegraph::SnapshotId::current();
            match backend.get_node(snapshot, id.0) {
                Ok(_) => Ok(true),
                Err(_) => Ok(false),
            }
        })
    }

    /// Query references for a specific symbol.
    ///
    /// # Arguments
    ///
    /// * `symbol_id` - The symbol to find references for
    ///
    /// # Returns
    ///
    /// List of references where this symbol is the target
    pub async fn query_references(&self, symbol_id: SymbolId) -> Result<Vec<Reference>> {
        self.with_backend(|backend| {
            let snapshot = sqlitegraph::SnapshotId::current();
            
            // Query incoming edges (references TO this symbol)
            let query = sqlitegraph::backend::NeighborQuery {
                direction: sqlitegraph::backend::BackendDirection::Incoming,
                edge_type: None,
            };
            
            let from_ids = backend.neighbors(snapshot, symbol_id.0, query)
                .map_err(|e| ForgeError::DatabaseError(
                    format!("Failed to query references: {}", e)
                ))?;

            let mut references = Vec::new();
            for from_id in from_ids {
                // Create a basic reference - in a full implementation we'd
                // retrieve edge properties too
                references.push(Reference {
                    from: SymbolId(from_id),
                    to: symbol_id,
                    kind: ReferenceKind::Use, // Default, should be from edge data
                    location: Location {
                        file_path: PathBuf::from("unknown"),
                        byte_start: 0,
                        byte_end: 0,
                        line_number: 0,
                    },
                });
            }

            Ok(references)
        })
    }

    /// Get all symbols in the graph.
    pub async fn get_all_symbols(&self) -> Result<Vec<Symbol>> {
        self.with_backend(|backend| {
            let snapshot = sqlitegraph::SnapshotId::current();
            let ids = backend.entity_ids()
                .map_err(|e| ForgeError::DatabaseError(
                    format!("Failed to get entity IDs: {}", e)
                ))?;

            let mut symbols = Vec::new();
            for id in ids {
                if let Ok(entity) = backend.get_node(snapshot, id) {
                    if let Some(symbol) = Self::entity_to_symbol(&entity) {
                        symbols.push(symbol);
                    }
                }
            }

            Ok(symbols)
        })
    }

    /// Get the count of symbols in the graph.
    pub async fn symbol_count(&self) -> Result<usize> {
        self.with_backend(|backend| {
            let header = backend.header();
            Ok(header.node_count as usize)
        })
    }

    /// Convert a GraphEntity to a Symbol.
    fn entity_to_symbol(entity: &GraphEntity) -> Option<Symbol> {
        let data = &entity.data;
        
        Some(Symbol {
            id: SymbolId(entity.id),
            name: entity.name.clone(),
            fully_qualified_name: data
                .get("fully_qualified_name")
                .and_then(|v| v.as_str())
                .unwrap_or(&entity.name)
                .to_string(),
            kind: parse_symbol_kind(&entity.kind),
            language: parse_language(
                data.get("language")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown")
            ),
            location: Location {
                file_path: PathBuf::from(
                    data.get("file_path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                ),
                byte_start: data
                    .get("byte_start")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
                byte_end: data
                    .get("byte_end")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
                line_number: data
                    .get("line_number")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize,
            },
            parent_id: data
                .get("parent_id")
                .and_then(|v| v.as_i64())
                .map(SymbolId),
            metadata: data
                .get("metadata")
                .cloned()
                .unwrap_or_else(|| serde_json::Value::Null),
        })
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
        assert!(store.is_connected());
    }

    #[tokio::test]
    async fn test_open_with_custom_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let custom_db = temp_dir.path().join("custom").join("graph.v3");

        let store = UnifiedGraphStore::open_with_path(temp_dir.path(), &custom_db).await.unwrap();

        assert_eq!(store.db_path(), custom_db);
        assert!(store.is_connected());
    }

    #[tokio::test]
    async fn test_insert_and_get_symbol() {
        let store = UnifiedGraphStore::memory().await.unwrap();

        let symbol = Symbol {
            id: SymbolId(0), // Will be assigned
            name: "test_function".to_string(),
            fully_qualified_name: "crate::test_function".to_string(),
            kind: SymbolKind::Function,
            language: Language::Rust,
            location: Location {
                file_path: PathBuf::from("src/lib.rs"),
                byte_start: 0,
                byte_end: 100,
                line_number: 10,
            },
            parent_id: None,
            metadata: serde_json::json!({"doc": "Test function"}),
        };

        let id = store.insert_symbol(&symbol).await.unwrap();
        assert!(id.0 > 0);

        // Retrieve the symbol
        let retrieved = store.get_symbol(id).await.unwrap();
        assert_eq!(retrieved.name, "test_function");
        assert_eq!(retrieved.fully_qualified_name, "crate::test_function");
        assert!(matches!(retrieved.kind, SymbolKind::Function));
    }

    #[tokio::test]
    async fn test_query_symbols() {
        let store = UnifiedGraphStore::memory().await.unwrap();

        // Insert test symbols
        let symbol1 = Symbol {
            id: SymbolId(0),
            name: "main_function".to_string(),
            fully_qualified_name: "main".to_string(),
            kind: SymbolKind::Function,
            language: Language::Rust,
            location: Location {
                file_path: PathBuf::from("src/main.rs"),
                byte_start: 0,
                byte_end: 50,
                line_number: 1,
            },
            parent_id: None,
            metadata: serde_json::Value::Null,
        };

        let symbol2 = Symbol {
            id: SymbolId(0),
            name: "helper_function".to_string(),
            fully_qualified_name: "helper".to_string(),
            kind: SymbolKind::Function,
            language: Language::Rust,
            location: Location {
                file_path: PathBuf::from("src/lib.rs"),
                byte_start: 0,
                byte_end: 50,
                line_number: 5,
            },
            parent_id: None,
            metadata: serde_json::Value::Null,
        };

        store.insert_symbol(&symbol1).await.unwrap();
        store.insert_symbol(&symbol2).await.unwrap();

        // Query for "main"
        let results = store.query_symbols("main").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "main_function");

        // Query for "function"
        let results = store.query_symbols("function").await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_insert_reference() {
        let store = UnifiedGraphStore::memory().await.unwrap();

        // Insert two symbols
        let symbol1 = Symbol {
            id: SymbolId(0),
            name: "caller".to_string(),
            fully_qualified_name: "caller".to_string(),
            kind: SymbolKind::Function,
            language: Language::Rust,
            location: Location {
                file_path: PathBuf::from("src/lib.rs"),
                byte_start: 0,
                byte_end: 50,
                line_number: 1,
            },
            parent_id: None,
            metadata: serde_json::Value::Null,
        };

        let symbol2 = Symbol {
            id: SymbolId(0),
            name: "callee".to_string(),
            fully_qualified_name: "callee".to_string(),
            kind: SymbolKind::Function,
            language: Language::Rust,
            location: Location {
                file_path: PathBuf::from("src/lib.rs"),
                byte_start: 100,
                byte_end: 150,
                line_number: 10,
            },
            parent_id: None,
            metadata: serde_json::Value::Null,
        };

        let id1 = store.insert_symbol(&symbol1).await.unwrap();
        let id2 = store.insert_symbol(&symbol2).await.unwrap();

        // Insert reference
        let reference = Reference {
            from: id1,
            to: id2,
            kind: ReferenceKind::Call,
            location: Location {
                file_path: PathBuf::from("src/lib.rs"),
                byte_start: 25,
                byte_end: 35,
                line_number: 2,
            },
        };

        store.insert_reference(&reference).await.unwrap();

        // Query references
        let refs = store.query_references(id2).await.unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].from.0, id1.0);
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

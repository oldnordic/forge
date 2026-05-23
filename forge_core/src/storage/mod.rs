//! Storage abstraction layer supporting dual backends.
//!
//! This module provides graph-based storage for ForgeKit with support for both
//! SQLite and Native V3 backends. Users choose the backend based on their needs.
//!
//! # Backend Selection
//!
//! | Feature | SQLite Backend | Native V3 Backend |
//! |---------|----------------|-------------------|
//! | ACID Transactions | ✅ Full | ✅ WAL-based |
//! | Raw SQL Access | ✅ Yes | ❌ No |
//! | Dependencies | libsqlite3 | Pure Rust |
//! | Startup Time | Fast | Faster |
//! | Tool Compatibility | magellan, llmgrep, mirage, splice (current) | Updated tools |
//!
//! # Examples
//!
//! ```rust,no_run
//! use forge_core::storage::{UnifiedGraphStore, BackendKind};
//!
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! // Use SQLite backend (default, stable)
//! let store = UnifiedGraphStore::open("./codebase", BackendKind::SQLite).await?;
//!
//! // Or use Native V3 backend (updated tools required)
//! let store = UnifiedGraphStore::open("./codebase", BackendKind::NativeV3).await?;
//! # Ok(())
//! # }
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::error::{ForgeError, Result};
use crate::types::{Language, Location, Reference, ReferenceKind, Symbol, SymbolId, SymbolKind};

// Re-export sqlitegraph types for advanced usage
pub use sqlitegraph::backend::{EdgeSpec, NodeSpec};
pub use sqlitegraph::config::{open_graph, BackendKind as SqliteGraphBackendKind, GraphConfig};
pub use sqlitegraph::graph::{GraphEntity, SqliteGraph};

/// Resolves the default magellan database path for a project root.
///
/// Returns `~/.magellan/<stem>.db` where `<stem>` is the last component
/// of `project_root`. Falls back to `~/.magellan/graph.db` if the stem
/// cannot be determined.
pub fn default_db_path(project_root: &std::path::Path) -> std::path::PathBuf {
    let stem = project_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("graph");
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    std::path::PathBuf::from(home)
        .join(".magellan")
        .join(format!("{}.db", stem))
}

/// Backend kind selection for UnifiedGraphStore.
///
/// Users choose which backend to use based on their requirements.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BackendKind {
    /// SQLite backend - stable, mature, works with current tools
    #[default]
    SQLite,
    /// Native V3 backend - high performance, pure Rust, updated tools required
    NativeV3,
}

impl std::fmt::Display for BackendKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SQLite => write!(f, "SQLite"),
            Self::NativeV3 => write!(f, "NativeV3"),
        }
    }
}

impl BackendKind {
    /// Converts to sqlitegraph's BackendKind.
    #[cfg(test)] // Only used in tests currently
    fn to_sqlitegraph_kind(self) -> SqliteGraphBackendKind {
        match self {
            Self::SQLite => SqliteGraphBackendKind::SQLite,
            Self::NativeV3 => SqliteGraphBackendKind::Native,
        }
    }

    /// Returns the default file extension for this backend.
    pub fn file_extension(&self) -> &str {
        match self {
            Self::SQLite => "db",
            Self::NativeV3 => "v3",
        }
    }

    /// Returns the default database filename for this backend.
    pub fn default_filename(&self) -> &str {
        match self {
            Self::SQLite => "graph.db",
            Self::NativeV3 => "graph.v3",
        }
    }
}

/// Unified graph store supporting dual backends.
///
/// This provides graph storage for symbols and references with the user's
/// choice of SQLite or Native V3 backend. Both backends expose the same
/// functionality through a unified API.
pub struct UnifiedGraphStore {
    /// Path to codebase
    pub codebase_path: PathBuf,
    /// Path to database file
    pub db_path: PathBuf,
    /// Active backend kind
    pub backend_kind: BackendKind,
    /// Reference storage for Native V3 backend (enables cross-file references)
    references: std::sync::Mutex<Vec<StoredReference>>,
}

/// Internal reference storage for Native V3 backend
#[derive(Clone, Debug)]
struct StoredReference {
    to_symbol: String,
    kind: ReferenceKind,
    file_path: PathBuf,
    line_number: usize,
}

impl Clone for UnifiedGraphStore {
    fn clone(&self) -> Self {
        Self {
            codebase_path: self.codebase_path.clone(),
            db_path: self.db_path.clone(),
            backend_kind: self.backend_kind,
            references: std::sync::Mutex::new(self.references.lock().unwrap().clone()),
        }
    }
}

impl std::fmt::Debug for UnifiedGraphStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnifiedGraphStore")
            .field("codebase_path", &self.codebase_path)
            .field("db_path", &self.db_path)
            .field("backend_kind", &self.backend_kind)
            .field("connected", &self.is_connected())
            .finish()
    }
}

impl UnifiedGraphStore {
    /// Opens a graph store with the specified backend.
    ///
    /// # Arguments
    ///
    /// * `codebase_path` - Path to codebase directory
    /// * `backend_kind` - Which backend to use (SQLite or NativeV3)
    ///
    /// # Returns
    ///
    /// A `UnifiedGraphStore` instance or an error if initialization fails
    pub async fn open(codebase_path: impl AsRef<Path>, backend_kind: BackendKind) -> Result<Self> {
        let codebase = codebase_path.as_ref();
        if !codebase.exists() {
            return Err(ForgeError::DatabaseError(format!(
                "Codebase path does not exist: {}",
                codebase.display()
            )));
        }
        let db_path = default_db_path(codebase);

        // Create parent directory if it doesn't exist
        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                ForgeError::DatabaseError(format!("Failed to create database directory: {}", e))
            })?;
        }

        // NativeV3 uses its own file so it doesn't overwrite the magellan SQLite DB.
        // db_path always stays at ~/.magellan/<stem>.db (SQLite, for magellan).
        let sqlitegraph_path = match backend_kind {
            BackendKind::SQLite => db_path.clone(),
            BackendKind::NativeV3 => {
                let stem = codebase
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("graph");
                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
                std::path::PathBuf::from(home)
                    .join(".magellan")
                    .join(format!("{}.v3", stem))
            }
        };
        let config = match backend_kind {
            BackendKind::SQLite => GraphConfig::sqlite(),
            BackendKind::NativeV3 => GraphConfig::native(),
        };

        let _graph = open_graph(&sqlitegraph_path, &config)
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to open database: {}", e)))?;

        // For NativeV3, also initialise the SQLite magellan DB at db_path so
        // is_connected() and graph() operations (which always use db_path) work.
        if matches!(backend_kind, BackendKind::NativeV3) {
            let _ = open_graph(&db_path, &GraphConfig::sqlite()).map_err(|e| {
                ForgeError::DatabaseError(format!("Failed to init magellan SQLite DB: {}", e))
            })?;
        }

        Ok(UnifiedGraphStore {
            codebase_path: codebase.to_path_buf(),
            db_path,
            backend_kind,
            references: std::sync::Mutex::new(Vec::new()),
        })
    }

    /// Opens a graph store with a custom database path.
    ///
    /// # Arguments
    ///
    /// * `codebase_path` - Path to codebase directory
    /// * `db_path` - Custom path for database file
    /// * `backend_kind` - Which backend to use
    pub async fn open_with_path(
        codebase_path: impl AsRef<Path>,
        db_path: impl AsRef<Path>,
        backend_kind: BackendKind,
    ) -> Result<Self> {
        let codebase = codebase_path.as_ref();
        let db = db_path.as_ref();

        // Create parent directory if it doesn't exist
        if let Some(parent) = db.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                ForgeError::DatabaseError(format!("Failed to create database directory: {}", e))
            })?;
        }

        // Open the graph (this validates the database works)
        let config = match backend_kind {
            BackendKind::SQLite => GraphConfig::sqlite(),
            BackendKind::NativeV3 => GraphConfig::native(),
        };

        let _graph = open_graph(db, &config)
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to open database: {}", e)))?;

        Ok(UnifiedGraphStore {
            codebase_path: codebase.to_path_buf(),
            db_path: db.to_path_buf(),
            backend_kind,
            references: std::sync::Mutex::new(Vec::new()),
        })
    }

    /// Creates an in-memory store for testing.
    #[cfg(test)]
    pub async fn memory() -> Result<Self> {
        use tempfile::tempdir;

        let temp = tempdir().map_err(|e| {
            ForgeError::DatabaseError(format!("Failed to create temp directory: {}", e))
        })?;

        Self::open(temp.path(), BackendKind::SQLite).await
    }

    /// Returns the backend kind currently in use.
    #[inline]
    pub fn backend_kind(&self) -> BackendKind {
        self.backend_kind
    }

    /// Returns the path to the database file.
    #[inline]
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Returns true if the database file exists.
    pub fn is_connected(&self) -> bool {
        self.db_path.exists()
    }

    /// Returns true if the graph database has no indexed entities.
    ///
    /// Used by `Forge::open()` to decide whether to auto-trigger indexing.
    /// Opens the sqlitegraph backend and checks `entity_ids()` count.
    pub fn needs_indexing(&self) -> bool {
        if !self.is_connected() {
            return true;
        }
        let config = match self.backend_kind {
            BackendKind::SQLite => GraphConfig::sqlite(),
            BackendKind::NativeV3 => GraphConfig::native(),
        };
        match open_graph(&self.db_path, &config) {
            Ok(backend) => match backend.entity_ids() {
                Ok(ids) => ids.is_empty(),
                Err(_) => true,
            },
            Err(_) => true,
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
        let config = match self.backend_kind {
            BackendKind::SQLite => GraphConfig::sqlite(),
            BackendKind::NativeV3 => GraphConfig::native(),
        };
        let backend = open_graph(&self.db_path, &config)
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to open graph: {}", e)))?;

        let kind = match symbol.kind {
            SymbolKind::Function | SymbolKind::Method => "fn",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::Trait => "trait",
            SymbolKind::Impl => "impl",
            SymbolKind::Module => "module",
            SymbolKind::TypeAlias => "type",
            SymbolKind::Constant | SymbolKind::Static => "const",
            SymbolKind::Parameter | SymbolKind::LocalVariable | SymbolKind::Field => "variable",
            SymbolKind::Macro => "macro",
            SymbolKind::Use => "use",
        };

        let node = NodeSpec {
            kind: kind.to_string(),
            name: symbol.name.to_string(),
            file_path: Some(symbol.location.file_path.to_string_lossy().into_owned()),
            data: symbol.metadata.clone(),
        };

        let id = backend
            .insert_node(node)
            .map_err(|e| ForgeError::DatabaseError(format!("Insert node failed: {}", e)))?;

        Ok(SymbolId(id))
    }

    /// Insert a reference between symbols.
    ///
    /// # Arguments
    ///
    /// * `reference` - The reference to insert
    pub async fn insert_reference(&self, reference: &Reference) -> Result<()> {
        // For Native V3 backend, store references in memory to enable cross-file references
        // This is a capability that SQLite backend (via magellan) doesn't support
        if self.backend_kind == BackendKind::NativeV3 {
            let mut refs = self.references.lock().unwrap();

            // Try to resolve symbol names from the reference
            // In a full implementation, we'd look up symbol names from IDs
            let to_symbol = format!("sym_{}", reference.to.0);

            refs.push(StoredReference {
                to_symbol,
                kind: reference.kind,
                file_path: reference.location.file_path.clone(),
                line_number: reference.location.line_number,
            });
        }
        Ok(())
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
        let conn = rusqlite::Connection::open(&self.db_path)
            .map_err(|e| ForgeError::DatabaseError(format!("Open db failed: {}", e)))?;

        let pattern = format!("%{}%", name);
        let mut stmt = conn
            .prepare(
                "SELECT id, kind, name, file_path FROM graph_entities WHERE name LIKE ?1 LIMIT 50",
            )
            .map_err(|e| ForgeError::DatabaseError(format!("Prepare failed: {}", e)))?;

        let symbols = stmt
            .query_map(rusqlite::params![pattern], |row| {
                let id: i64 = row.get(0)?;
                let sym_name: String = row.get(2)?;
                let file_path: Option<String> = row.get(3)?;
                Ok((id, sym_name, file_path))
            })
            .map_err(|e| ForgeError::DatabaseError(format!("Query failed: {}", e)))?
            .flatten()
            .map(|(id, sym_name, file_path)| Symbol {
                id: SymbolId(id),
                name: Arc::from(sym_name.as_str()),
                fully_qualified_name: Arc::from(sym_name.as_str()),
                kind: SymbolKind::Function,
                language: Language::Rust,
                location: Location {
                    file_path: file_path
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from("")),
                    byte_start: 0,
                    byte_end: 0,
                    line_number: 0,
                },
                parent_id: None,
                metadata: serde_json::Value::Null,
            })
            .collect();

        Ok(symbols)
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
    pub async fn get_symbol(&self, _id: SymbolId) -> Result<Symbol> {
        Err(ForgeError::SymbolNotFound("Not implemented".to_string()))
    }

    /// Check if a symbol exists.
    ///
    /// # Arguments
    ///
    /// * `id` - The symbol ID to check
    pub async fn symbol_exists(&self, id: SymbolId) -> Result<bool> {
        let conn = rusqlite::Connection::open(&self.db_path)
            .map_err(|e| ForgeError::DatabaseError(format!("Open db failed: {}", e)))?;
        let exists: i64 = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM graph_entities WHERE id = ?1)",
                rusqlite::params![id.0],
                |row| row.get(0),
            )
            .map_err(|e| ForgeError::DatabaseError(format!("Query failed: {}", e)))?;
        Ok(exists > 0)
    }

    /// Query references for a specific symbol.
    ///
    /// # Arguments
    ///
    /// * `symbol_id` - The symbol to find references for
    ///
    /// # Returns
    ///
    /// List of references where this symbol is the target.
    /// For Native V3 backend, this includes cross-file references.
    pub async fn query_references(&self, symbol_id: SymbolId) -> Result<Vec<Reference>> {
        // For Native V3 backend, use in-memory stored references
        // In a full implementation, this would query magellan's side tables
        if self.backend_kind == BackendKind::NativeV3 {
            let refs = self.references.lock().unwrap();
            let target_symbol = format!("sym_{}", symbol_id.0);

            let mut result = Vec::new();
            for stored in refs.iter() {
                if stored.to_symbol == target_symbol {
                    result.push(Reference {
                        from: SymbolId(0),
                        to: symbol_id,
                        from_name: None,
                        to_name: None,
                        kind: stored.kind,
                        location: Location {
                            file_path: stored.file_path.clone(),
                            byte_start: 0,
                            byte_end: 0,
                            line_number: stored.line_number,
                        },
                    });
                }
            }
            return Ok(result);
        }

        // For SQLite backend, return empty (cross-file references not supported by magellan SQLite)
        Ok(Vec::new())
    }

    /// Get all symbols in the graph.
    pub async fn get_all_symbols(&self) -> Result<Vec<Symbol>> {
        let conn = rusqlite::Connection::open(&self.db_path)
            .map_err(|e| ForgeError::DatabaseError(format!("Open db failed: {}", e)))?;
        let mut stmt = conn
            .prepare("SELECT id, kind, name, file_path FROM graph_entities LIMIT 1000")
            .map_err(|e| ForgeError::DatabaseError(format!("Prepare failed: {}", e)))?;
        let symbols = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let sym_name: String = row.get(2)?;
                let file_path: Option<String> = row.get(3)?;
                Ok((id, sym_name, file_path))
            })
            .map_err(|e| ForgeError::DatabaseError(format!("Query failed: {}", e)))?
            .flatten()
            .map(|(id, sym_name, file_path)| Symbol {
                id: SymbolId(id),
                name: Arc::from(sym_name.as_str()),
                fully_qualified_name: Arc::from(sym_name.as_str()),
                kind: SymbolKind::Function,
                language: Language::Rust,
                location: Location {
                    file_path: file_path
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from("")),
                    byte_start: 0,
                    byte_end: 0,
                    line_number: 0,
                },
                parent_id: None,
                metadata: serde_json::Value::Null,
            })
            .collect();
        Ok(symbols)
    }

    /// Get count of symbols in the graph.
    pub async fn symbol_count(&self) -> Result<usize> {
        let conn = rusqlite::Connection::open(&self.db_path)
            .map_err(|e| ForgeError::DatabaseError(format!("Open db failed: {}", e)))?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM graph_entities", [], |row| row.get(0))
            .map_err(|e| ForgeError::DatabaseError(format!("Query failed: {}", e)))?;
        Ok(count as usize)
    }

    /// Scans and indexes cross-file references for Native V3 backend.
    ///
    /// This is a capability that Native V3 enables over SQLite.
    /// It uses magellan's native cross-file reference indexing.
    ///
    /// Note: With the updated magellan, cross-file references are automatically
    /// indexed during the normal `index_references` call. This method is kept
    /// for API compatibility but delegates to magellan.
    pub async fn index_cross_file_references(&self) -> Result<usize> {
        if self.backend_kind != BackendKind::NativeV3 {
            return Ok(0); // Only supported on Native V3
        }

        // For now, use the legacy implementation that scans files
        // In a full implementation, this would use magellan's side tables
        self.legacy_index_cross_file_references().await
    }

    /// Legacy implementation using in-memory storage
    async fn legacy_index_cross_file_references(&self) -> Result<usize> {
        use regex::Regex;
        use tokio::fs;

        // First pass: collect all symbol definitions
        let mut symbols: std::collections::HashMap<String, (PathBuf, usize)> =
            std::collections::HashMap::new();
        self.collect_symbols_recursive(&self.codebase_path, &mut symbols)
            .await?;

        // Second pass: find all references
        let reference_pattern = Regex::new(r"\b([a-zA-Z_][a-zA-Z0-9_]*)\s*\(").unwrap();

        {
            let mut refs = self.references.lock().unwrap();
            refs.clear();
        }

        let mut found_refs: Vec<StoredReference> = Vec::new();

        for (symbol_name, (_file_path, _)) in &symbols {
            // Scan all files for references to this symbol
            for (target_file, _) in symbols.values() {
                if let Ok(content) = fs::read_to_string(target_file).await {
                    for (line_num, line) in content.lines().enumerate() {
                        // Skip lines that are function definitions
                        if line.contains("fn ") || line.contains("struct ") {
                            continue;
                        }

                        // Check for calls/references to this symbol
                        for cap in reference_pattern.captures_iter(line) {
                            if let Some(matched) = cap.get(1) {
                                if matched.as_str() == symbol_name {
                                    found_refs.push(StoredReference {
                                        to_symbol: format!("sym_{}", symbol_name),
                                        kind: ReferenceKind::Call,
                                        file_path: target_file.clone(),
                                        line_number: line_num + 1,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        let ref_count = found_refs.len();
        self.references.lock().unwrap().extend(found_refs);

        Ok(ref_count)
    }

    async fn collect_symbols_recursive(
        &self,
        dir: &Path,
        symbols: &mut std::collections::HashMap<String, (PathBuf, usize)>,
    ) -> Result<()> {
        use tokio::fs;

        let mut entries = fs::read_dir(dir)
            .await
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to read dir: {}", e)))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to read entry: {}", e)))?
        {
            let path = entry.path();
            if path.is_dir() {
                Box::pin(self.collect_symbols_recursive(&path, symbols)).await?;
            } else if path.is_file() && path.extension().map(|e| e == "rs").unwrap_or(false) {
                if let Ok(content) = fs::read_to_string(&path).await {
                    for (line_num, line) in content.lines().enumerate() {
                        // Extract function definitions
                        if let Some(fn_pos) = line.find("fn ") {
                            let after_fn = &line[fn_pos + 3..];
                            if let Some(end_pos) =
                                after_fn.find(|c: char| c.is_whitespace() || c == '(')
                            {
                                let name = after_fn[..end_pos].trim().to_string();
                                if !name.is_empty() {
                                    symbols.insert(name, (path.clone(), line_num + 1));
                                }
                            }
                        }
                        // Extract struct definitions
                        if let Some(struct_pos) = line.find("struct ") {
                            let after_struct = &line[struct_pos + 7..];
                            if let Some(end_pos) = after_struct
                                .find(|c: char| c.is_whitespace() || c == '{' || c == ';')
                            {
                                let name = after_struct[..end_pos].trim().to_string();
                                if !name.is_empty() {
                                    symbols.insert(name, (path.clone(), line_num + 1));
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Query references by symbol name (for Native V3 backend).
    /// This enables cross-file references that magellan doesn't support.
    pub async fn query_references_for_symbol(&self, symbol_name: &str) -> Result<Vec<Reference>> {
        if self.backend_kind != BackendKind::NativeV3 {
            return Ok(Vec::new());
        }

        let refs = self.references.lock().unwrap();
        let mut result = Vec::new();

        for stored in refs.iter() {
            if stored.to_symbol == format!("sym_{}", symbol_name)
                || stored.to_symbol.contains(symbol_name)
            {
                result.push(Reference {
                    from: SymbolId(0),
                    to: SymbolId(0),
                    from_name: None,
                    to_name: None,
                    kind: stored.kind,
                    location: Location {
                        file_path: stored.file_path.clone(),
                        byte_start: 0,
                        byte_end: 0,
                        line_number: stored.line_number,
                    },
                });
            }
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test that BackendKind::default() returns SQLite
    #[test]
    fn test_backend_kind_default() {
        assert_eq!(BackendKind::default(), BackendKind::SQLite);
    }

    // Test that to_sqlitegraph_kind() converts correctly
    #[test]
    fn test_backend_kind_to_sqlitegraph() {
        assert_eq!(
            BackendKind::SQLite.to_sqlitegraph_kind(),
            SqliteGraphBackendKind::SQLite
        );
        assert_eq!(
            BackendKind::NativeV3.to_sqlitegraph_kind(),
            SqliteGraphBackendKind::Native
        );
    }

    // Test that file_extension() returns correct values
    #[test]
    fn test_backend_kind_file_extension() {
        assert_eq!(BackendKind::SQLite.file_extension(), "db");
        assert_eq!(BackendKind::NativeV3.file_extension(), "v3");
    }

    // Test that default_filename() returns correct values
    #[test]
    fn test_backend_kind_default_filename() {
        assert_eq!(BackendKind::SQLite.default_filename(), "graph.db");
        assert_eq!(BackendKind::NativeV3.default_filename(), "graph.v3");
    }

    // Test that BackendKind Display implementation works
    #[test]
    fn test_backend_kind_display() {
        assert_eq!(BackendKind::SQLite.to_string(), "SQLite");
        assert_eq!(BackendKind::NativeV3.to_string(), "NativeV3");
    }

    // Test that opening a SQLite store creates database file under ~/.magellan/
    #[tokio::test]
    async fn test_open_sqlite_creates_database() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
            .await
            .unwrap();

        assert_eq!(store.backend_kind(), BackendKind::SQLite);
        assert!(store.db_path().to_string_lossy().contains(".magellan"));
        assert!(store.db_path().extension().is_some_and(|e| e == "db"));
        assert!(store.is_connected());
    }

    // Test that opening a Native V3 store creates database file under ~/.magellan/
    #[tokio::test]
    async fn test_open_native_v3_creates_database() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = UnifiedGraphStore::open(temp_dir.path(), BackendKind::NativeV3)
            .await
            .unwrap();

        assert_eq!(store.backend_kind(), BackendKind::NativeV3);
        assert!(store.db_path().to_string_lossy().contains(".magellan"));
        assert!(store.db_path().extension().is_some_and(|e| e == "db"));
        assert!(store.is_connected());
    }

    // Test that opening with custom path works
    #[tokio::test]
    async fn test_open_with_custom_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let custom_db = temp_dir.path().join("custom").join("graph.db");

        let store =
            UnifiedGraphStore::open_with_path(temp_dir.path(), &custom_db, BackendKind::SQLite)
                .await
                .unwrap();

        assert_eq!(store.db_path(), custom_db);
        assert!(store.is_connected());
    }

    // Test inserting a symbol returns a valid ID (placeholder)
    #[tokio::test]
    async fn test_insert_symbol_returns_id() {
        let store = UnifiedGraphStore::memory().await.unwrap();

        let symbol = Symbol {
            id: SymbolId(0),
            name: Arc::from("test_function"),
            fully_qualified_name: Arc::from("crate::test_function"),
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
    }

    // Test query_symbols returns empty for non-existent pattern
    #[tokio::test]
    async fn test_query_symbols_empty() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
            .await
            .unwrap();

        // Query for non-existent pattern
        let results = store.query_symbols("nonexistent_xyz").await.unwrap();
        assert!(results.is_empty());
    }

    // Test insert_reference succeeds (placeholder)
    #[tokio::test]
    async fn test_insert_reference_placeholder() {
        let store = UnifiedGraphStore::memory().await.unwrap();

        let reference = Reference {
            from: SymbolId(1),
            to: SymbolId(2),
            from_name: None,
            to_name: None,
            kind: ReferenceKind::Call,
            location: Location {
                file_path: PathBuf::from("src/lib.rs"),
                byte_start: 25,
                byte_end: 35,
                line_number: 2,
            },
        };

        // Should succeed even though it's a placeholder
        store.insert_reference(&reference).await.unwrap();
    }

    // Test symbol_exists returns false for an unknown ID
    #[tokio::test]
    async fn test_symbol_exists_unknown_id() {
        let (store, _dir) = isolated_store().await;
        assert!(!store.symbol_exists(SymbolId(99999)).await.unwrap());
    }

    // Test get_all_symbols returns empty on fresh DB
    #[tokio::test]
    async fn test_get_all_symbols_empty_db() {
        let (store, _dir) = isolated_store().await;
        let symbols = store.get_all_symbols().await.unwrap();
        assert!(symbols.is_empty());
    }

    // Test get_all_symbols returns inserted symbols
    #[tokio::test]
    async fn test_get_all_symbols_returns_inserted() {
        let (store, _dir) = isolated_store().await;
        store
            .insert_symbol(&make_symbol("alpha_get_all"))
            .await
            .unwrap();
        store
            .insert_symbol(&make_symbol("beta_get_all"))
            .await
            .unwrap();
        let symbols = store.get_all_symbols().await.unwrap();
        assert_eq!(
            symbols.len(),
            2,
            "get_all_symbols should return all inserted symbols"
        );
    }

    // Test symbol_count returns 0 on fresh DB
    #[tokio::test]
    async fn test_symbol_count_empty_db() {
        let (store, _dir) = isolated_store().await;
        assert_eq!(store.symbol_count().await.unwrap(), 0);
    }

    // Test symbol_count returns correct count after inserts
    #[tokio::test]
    async fn test_symbol_count_after_inserts() {
        let (store, _dir) = isolated_store().await;
        store
            .insert_symbol(&make_symbol("count_sym_a"))
            .await
            .unwrap();
        store
            .insert_symbol(&make_symbol("count_sym_b"))
            .await
            .unwrap();
        store
            .insert_symbol(&make_symbol("count_sym_c"))
            .await
            .unwrap();
        assert_eq!(
            store.symbol_count().await.unwrap(),
            3,
            "symbol_count should equal insert count"
        );
    }

    // Test Clone implementation
    #[test]
    fn test_unified_graph_store_clone() {
        let store = UnifiedGraphStore {
            codebase_path: PathBuf::from("/test"),
            db_path: PathBuf::from("/test/graph.db"),
            backend_kind: BackendKind::SQLite,
            references: std::sync::Mutex::new(Vec::new()),
        };

        let cloned = store.clone();

        assert_eq!(cloned.codebase_path, PathBuf::from("/test"));
        assert_eq!(cloned.db_path, PathBuf::from("/test/graph.db"));
        assert_eq!(cloned.backend_kind, BackendKind::SQLite);
    }

    // Test Debug implementation
    #[test]
    fn test_unified_graph_store_debug() {
        let store = UnifiedGraphStore {
            codebase_path: PathBuf::from("/test"),
            db_path: PathBuf::from("/test/graph.db"),
            backend_kind: BackendKind::SQLite,
            references: std::sync::Mutex::new(Vec::new()),
        };

        let debug_str = format!("{:?}", store);
        assert!(debug_str.contains("UnifiedGraphStore"));
        assert!(debug_str.contains("codebase_path: \"/test\""));
        assert!(debug_str.contains("db_path: \"/test/graph.db\""));
        assert!(debug_str.contains("backend_kind: SQLite"));
    }

    #[test]
    fn test_default_db_path_uses_home_dot_magellan() {
        let project = std::path::Path::new("/home/user/Projects/my-cool-project");
        let db = default_db_path(project);
        assert!(db.to_string_lossy().contains(".magellan"));
        assert!(db.to_string_lossy().ends_with("my-cool-project.db"));
    }

    #[test]
    fn test_default_db_path_fallback_stem() {
        let project = std::path::Path::new("/");
        let db = default_db_path(project);
        assert!(db.to_string_lossy().ends_with(".magellan/graph.db"));
    }

    fn make_symbol(name: &str) -> Symbol {
        Symbol {
            id: SymbolId(0),
            name: Arc::from(name),
            fully_qualified_name: Arc::from(name),
            kind: SymbolKind::Function,
            language: Language::Rust,
            location: Location {
                file_path: PathBuf::from("src/lib.rs"),
                byte_start: 0,
                byte_end: 10,
                line_number: 1,
            },
            parent_id: None,
            metadata: serde_json::Value::Null,
        }
    }

    async fn isolated_store() -> (UnifiedGraphStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let store = UnifiedGraphStore::open_with_path(dir.path(), &db_path, BackendKind::SQLite)
            .await
            .unwrap();
        (store, dir)
    }

    #[tokio::test]
    async fn test_insert_symbol_unique_ids() {
        let (store, _dir) = isolated_store().await;
        let id1 = store.insert_symbol(&make_symbol("alpha_fn")).await.unwrap();
        let id2 = store.insert_symbol(&make_symbol("beta_fn")).await.unwrap();
        assert_ne!(id1, id2, "each insert should return a unique ID");
    }

    #[tokio::test]
    async fn test_symbol_exists_after_insert() {
        let (store, _dir) = isolated_store().await;
        let id = store.insert_symbol(&make_symbol("check_fn")).await.unwrap();
        assert!(
            store.symbol_exists(id).await.unwrap(),
            "symbol should exist after insert"
        );
    }

    #[tokio::test]
    async fn test_query_symbols_finds_inserted() {
        let (store, _dir) = isolated_store().await;
        store
            .insert_symbol(&make_symbol("my_unique_query_target"))
            .await
            .unwrap();
        let results = store.query_symbols("my_unique_query_target").await.unwrap();
        assert!(!results.is_empty(), "query should find the inserted symbol");
    }
}

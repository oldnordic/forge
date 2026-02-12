//! Storage abstraction layer.
//!
//! This module provides a unified interface to the SQLiteGraph backend.
//! Currently a placeholder - full integration planned for v0.2.

use std::path::Path;
use crate::error::{ForgeError, Result};
use crate::types::SymbolId;

/// Unified graph store for all ForgeKit operations.
///
/// This is currently a placeholder implementation.
/// Full SQLiteGraph integration is planned for v0.2.
#[derive(Clone)]
pub struct UnifiedGraphStore {
    /// Path to the codebase
    pub codebase_path: std::path::PathBuf,
    /// Path to the database file
    pub db_path: std::path::PathBuf,
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

        Ok(UnifiedGraphStore {
            codebase_path: codebase.to_path_buf(),
            db_path,
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

        Ok(UnifiedGraphStore {
            codebase_path: codebase.to_path_buf(),
            db_path: db.to_path_buf(),
        })
    }

    /// Returns the path to the database file.
    #[inline]
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Checks if a symbol exists in the graph (placeholder).
    pub async fn symbol_exists(&self, _id: SymbolId) -> Result<bool> {
        // TODO: Implement via SQLiteGraph in v0.2
        Ok(false)
    }

    /// Gets a symbol by ID (placeholder).
    pub async fn get_symbol(&self, id: SymbolId) -> Result<crate::types::Symbol> {
        // TODO: Implement via SQLiteGraph in v0.2
        Err(ForgeError::SymbolNotFound(format!("{}", id)))
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
        assert!(store.symbol_exists(SymbolId(999)).await.unwrap() == false);
    }
}

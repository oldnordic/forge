use std::path::{Path, PathBuf};

use sqlitegraph::config::{open_graph, GraphConfig};

use crate::error::{ForgeError, Result};
use crate::types::ReferenceKind;

use super::{default_db_path, BackendKind};

#[derive(Clone, Debug)]
pub(super) struct StoredReference {
    pub(super) to_symbol: String,
    pub(super) kind: ReferenceKind,
    pub(super) file_path: PathBuf,
    pub(super) line_number: usize,
}

pub struct UnifiedGraphStore {
    pub codebase_path: PathBuf,
    pub db_path: PathBuf,
    pub backend_kind: BackendKind,
    pub(super) references: std::sync::Mutex<Vec<StoredReference>>,
}

impl Clone for UnifiedGraphStore {
    fn clone(&self) -> Self {
        Self {
            codebase_path: self.codebase_path.clone(),
            db_path: self.db_path.clone(),
            backend_kind: self.backend_kind,
            references: std::sync::Mutex::new(
                self.references
                    .lock()
                    .expect("invariant: references mutex not poisoned")
                    .clone(),
            ),
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
    pub async fn open(codebase_path: impl AsRef<Path>, backend_kind: BackendKind) -> Result<Self> {
        let codebase = codebase_path.as_ref();
        if !codebase.exists() {
            return Err(ForgeError::DatabaseError(format!(
                "Codebase path does not exist: {}",
                codebase.display()
            )));
        }
        let db_path = default_db_path(codebase);

        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                ForgeError::DatabaseError(format!("Failed to create database directory: {}", e))
            })?;
        }

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

    pub async fn open_with_path(
        codebase_path: impl AsRef<Path>,
        db_path: impl AsRef<Path>,
        backend_kind: BackendKind,
    ) -> Result<Self> {
        let codebase = codebase_path.as_ref();
        let db = db_path.as_ref();

        if let Some(parent) = db.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                ForgeError::DatabaseError(format!("Failed to create database directory: {}", e))
            })?;
        }

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

    #[cfg(test)]
    pub async fn memory() -> Result<Self> {
        use tempfile::tempdir;

        let temp = tempdir().map_err(|e| {
            ForgeError::DatabaseError(format!("Failed to create temp directory: {}", e))
        })?;

        Self::open(temp.path(), BackendKind::SQLite).await
    }

    #[inline]
    pub fn backend_kind(&self) -> BackendKind {
        self.backend_kind
    }

    #[inline]
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn is_connected(&self) -> bool {
        self.db_path.exists()
    }

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
}

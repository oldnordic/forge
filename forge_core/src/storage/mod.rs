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

mod ops;
mod store;
#[cfg(test)]
mod tests;

pub use sqlitegraph::backend::{EdgeSpec, NodeSpec};
pub use sqlitegraph::config::{open_graph, BackendKind as SqliteGraphBackendKind, GraphConfig};
pub use sqlitegraph::graph::{GraphEntity, SqliteGraph};

pub use store::UnifiedGraphStore;

use std::path::Path;

pub fn default_db_path(project_root: &Path) -> std::path::PathBuf {
    let stem = project_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("graph");
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    std::path::PathBuf::from(home)
        .join(".magellan")
        .join(format!("{}.db", stem))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BackendKind {
    #[default]
    SQLite,
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
    #[cfg(test)]
    fn to_sqlitegraph_kind(self) -> SqliteGraphBackendKind {
        match self {
            Self::SQLite => SqliteGraphBackendKind::SQLite,
            Self::NativeV3 => SqliteGraphBackendKind::Native,
        }
    }

    pub fn file_extension(&self) -> &str {
        match self {
            Self::SQLite => "db",
            Self::NativeV3 => "v3",
        }
    }

    pub fn default_filename(&self) -> &str {
        match self {
            Self::SQLite => "graph.db",
            Self::NativeV3 => "graph.v3",
        }
    }
}
